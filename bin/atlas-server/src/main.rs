//! Atlas server binary — entry point.
//!
//! Composes feature crates and runs the long-lived HTTP server.
//! Business logic belongs to the feature crates it composes,
//! never here.

mod config;

use config::{
    DEFAULT_DB_PATH, ENV_DB_PATH, ENV_TRACKER_WEBHOOK_SECRET, StartupError,
    read_github_oauth_config, read_gitlab_oauth_config, read_listen_addr, read_mcp_token,
    read_mirror_config, read_server_url, read_tracker_config, read_workspace_root,
};

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;

use atlas_afg::api::AfgRuntime;
use atlas_auth::api::AuthStore;
use atlas_mcp::api::{McpState, McpToken};
use atlas_messaging::api::MessageStore;
use atlas_sessions::api::SessionStore;
use atlas_terminal::api::PtyPool;
use atlas_tracker::api::{
    Composition, MirrorConfig, SqlitePool, build_composition, run_migrations,
};
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use serde_json::json;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode};
use tokio::signal;

#[tokio::main]
async fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("atlas-server: {err}");
            ExitCode::from(1)
        }
    }
}

#[derive(Clone)]
struct HealthState {
    pool: SqlitePool,
}

async fn run() -> Result<(), StartupError> {
    let db_path =
        PathBuf::from(std::env::var(ENV_DB_PATH).unwrap_or_else(|_| DEFAULT_DB_PATH.to_owned()));
    ensure_parent_dir(&db_path)?;
    let pool = open_pool(&db_path).await?;
    run_migrations(&pool).await.map_err(StartupError::Migrate)?;
    atlas_messaging::api::run_migrations(&pool)
        .await
        .map_err(StartupError::Migrate)?;
    let messages = MessageStore::new(pool.clone());
    atlas_sessions::api::run_migrations(&pool)
        .await
        .map_err(StartupError::Migrate)?;
    let sessions = SessionStore::new(pool.clone());
    let workspace_root = read_workspace_root()?;
    // Read here rather than beside the other configuration below: a
    // spawned terminal is told this address, so the pool needs it at
    // construction.
    let listen_addr = read_listen_addr()?;
    let server_url = read_server_url(listen_addr);
    let terminals = Arc::new(PtyPool::new(sessions.clone(), workspace_root, server_url));
    atlas_afg::api::run_migrations(&pool)
        .await
        .map_err(StartupError::Migrate)?;
    // Hoisted rather than constructed inline: the live-view router
    // subscribes to the same store the runtime writes through, so
    // both halves must share one instance (the hub lives inside it).
    let afg_store = atlas_afg::api::AfgStore::new(pool.clone());
    let afg = AfgRuntime::new(afg_store.clone(), sessions.clone(), messages.clone());
    atlas_auth::api::run_migrations(&pool)
        .await
        .map_err(StartupError::Migrate)?;
    let memory = migrate_memory(&pool).await?;
    let notes = migrate_notes(&pool).await?;
    let graph = migrate_graph(&pool).await?;
    let auth_store = AuthStore::new(pool.clone());
    let sync_router = sync_router(&auth_store, &sessions, &memory, &notes);
    let afg_router = atlas_afg::api::router(afg_store, auth_store.clone(), sessions.clone());
    let sync_supervisor = Arc::new(atlas_sync::api::SyncSupervisor::new(
        sessions.clone(),
        memory.clone(),
        notes.clone(),
    ));

    let mcp_token = read_mcp_token()?;
    let gitlab_oauth_config = read_gitlab_oauth_config()?;
    let github_oauth_config = read_github_oauth_config()?;

    let Tracker {
        runtime,
        webhook: tracker_webhook,
        syncer_handle,
        kind: tracker_kind,
        mirror_summary,
    } = compose_tracker(pool.clone())?;

    let mcp_state = McpState::new(
        runtime.clone(),
        messages,
        sessions,
        terminals,
        afg,
        sync_supervisor,
        memory,
        notes,
        graph.clone(),
        Arc::new(atlas_graph::api::GraphWatcher::new(graph)),
        McpToken::new(mcp_token),
    );
    let app = Router::new()
        .route("/health", get(health))
        .with_state(HealthState { pool })
        .nest("/api/mcp", atlas_mcp::api::router(mcp_state))
        .nest(
            "/api/auth",
            atlas_auth::api::router(auth_store, gitlab_oauth_config, github_oauth_config),
        )
        .nest("/api/sync", sync_router)
        .nest("/api/afg", afg_router)
        .nest("/api/webhooks/tracker", tracker_webhook)
        .layer(tower_http::trace::TraceLayer::new_for_http());

    let listener = tokio::net::TcpListener::bind(listen_addr)
        .await
        .map_err(|source| StartupError::Bind {
            addr: listen_addr,
            source,
        })?;

    tracing::info!(
        version = env!("CARGO_PKG_VERSION"),
        addr = %listen_addr,
        tracker = ?tracker_kind,
        mirror = mirror_summary,
        mcp = "on",
        "atlas-server listening",
    );

    let serve = axum::serve(listener, app).with_graceful_shutdown(shutdown_signal());
    if let Err(e) = serve.await {
        tracing::error!(error = %e, "atlas-server exited with error");
    }

    if let Some(handle) = syncer_handle {
        handle.abort();
        let _ = handle.await;
    }

    tracing::info!("atlas-server stopped");
    // Force ownership move so the runtime isn't dropped before the
    // server is done responding to in-flight requests.
    drop(runtime);
    Ok(())
}

/// The `/api/sync` router, assembled from the stores it replicates.
///
/// Extracted for the same reason as the `migrate_*` helpers: every
/// Type 2 data kind added to replication lengthens this call, and
/// `run` is composition rather than a place to grow an argument list.
fn sync_router(
    auth: &AuthStore,
    sessions: &SessionStore,
    memory: &atlas_memory::api::MemoryStore,
    notes: &atlas_notes::api::NoteStore,
) -> axum::Router {
    atlas_sync::api::router(
        auth.clone(),
        sessions.clone(),
        memory.clone(),
        notes.clone(),
    )
}

/// Kept out of `run` so that adding a feature crate is one line there
/// rather than four, and the composition stays readable as it grows.
async fn migrate_memory(pool: &SqlitePool) -> Result<atlas_memory::api::MemoryStore, StartupError> {
    atlas_memory::api::run_migrations(pool)
        .await
        .map_err(StartupError::Migrate)?;
    Ok(atlas_memory::api::MemoryStore::new(pool.clone()))
}

async fn migrate_notes(pool: &SqlitePool) -> Result<atlas_notes::api::NoteStore, StartupError> {
    atlas_notes::api::run_migrations(pool)
        .await
        .map_err(StartupError::Migrate)?;
    Ok(atlas_notes::api::NoteStore::new(pool.clone()))
}

async fn migrate_graph(pool: &SqlitePool) -> Result<atlas_graph::api::GraphStore, StartupError> {
    atlas_graph::api::run_migrations(pool)
        .await
        .map_err(StartupError::Migrate)?;
    Ok(atlas_graph::api::GraphStore::new(pool.clone()))
}

/// Everything the tracker contributes to the running server.
///
/// Assembled here rather than inline so `run` stays readable: reading
/// three environment blocks, building the composition and starting the
/// mirror loop is one concern, and it is the only part of startup that
/// can be absent entirely.
struct Tracker {
    runtime: atlas_tracker::api::TrackerRuntime,
    webhook: axum::Router,
    syncer_handle: Option<tokio::task::JoinHandle<()>>,
    kind: atlas_tracker::api::TrackerKind,
    mirror_summary: String,
}

fn compose_tracker(pool: SqlitePool) -> Result<Tracker, StartupError> {
    let tracker_config = read_tracker_config()?;
    let mirror_config = read_mirror_config()?;
    let kind = tracker_config.kind.clone();

    let Composition {
        runtime,
        syncer,
        webhook,
    } = build_composition(
        &tracker_config,
        &mirror_config,
        pool,
        std::env::var(ENV_TRACKER_WEBHOOK_SECRET).ok(),
    )?;

    Ok(Tracker {
        runtime,
        webhook,
        syncer_handle: syncer.map(atlas_tracker::api::MirrorSyncer::spawn),
        kind,
        mirror_summary: describe_mirror(&mirror_config),
    })
}

/// One line for the startup log. Pulled out of `run` because it is
/// presentation rather than composition, and `run` is at its length
/// budget.
fn describe_mirror(config: &MirrorConfig) -> String {
    if config.projects.is_empty() {
        "off".to_owned()
    } else {
        format!(
            "{n} projects, interval {secs}s",
            n = config.projects.len(),
            secs = config.interval.as_secs(),
        )
    }
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };
    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => tracing::info!("received SIGINT, shutting down"),
        () = terminate => tracing::info!("received SIGTERM, shutting down"),
    }
}

async fn health(State(state): State<HealthState>) -> Response {
    match sqlx::query_scalar::<_, i32>("SELECT 1")
        .fetch_one(&state.pool)
        .await
    {
        Ok(_) => (
            StatusCode::OK,
            Json(json!({ "status": "ok", "version": env!("CARGO_PKG_VERSION") })),
        )
            .into_response(),
        Err(e) => {
            tracing::warn!(error = %e, "health: db check failed");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({ "status": "degraded", "reason": "db unavailable" })),
            )
                .into_response()
        }
    }
}

fn ensure_parent_dir(db_path: &Path) -> Result<(), StartupError> {
    if let Some(parent) = db_path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent).map_err(|source| StartupError::DbDir {
            path: parent.to_path_buf(),
            source,
        })?;
        // The directory too: a world-executable directory lets someone
        // reach the `-wal` file even once the database itself is 0600.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700));
        }
    }
    Ok(())
}

async fn open_pool(db_path: &Path) -> Result<SqlitePool, StartupError> {
    let options = SqliteConnectOptions::new()
        .filename(db_path)
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal);
    let pool = SqlitePool::connect_with(options)
        .await
        .map_err(StartupError::DbOpen)?;
    restrict_to_owner(db_path);
    Ok(pool)
}

/// Make the database readable only by the user running the server.
///
/// `SQLite` creates it with the process umask, which on most systems
/// means world-readable — so on a machine with more than one account,
/// everybody's notes, memory and messages were readable by anyone. The
/// file holds no credential that works if stolen (passwords and tokens
/// are stored hashed), but it holds a developer's private notes, and
/// those are private because they are theirs.
///
/// WAL mode writes two siblings alongside it; both get the same
/// treatment, since a reader of `-wal` reads recent writes.
///
/// Applied after opening rather than before, because the file does not
/// exist until `SQLite` creates it. A failure here is logged and not
/// fatal: refusing to start over a permission bit would be worse than
/// running with the umask's answer, and the log says which it is.
#[cfg(unix)]
fn restrict_to_owner(db_path: &Path) {
    use std::os::unix::fs::PermissionsExt;

    for suffix in ["", "-wal", "-shm"] {
        let path = if suffix.is_empty() {
            db_path.to_path_buf()
        } else {
            let mut name = db_path.as_os_str().to_owned();
            name.push(suffix);
            PathBuf::from(name)
        };
        if !path.exists() {
            continue;
        }
        if let Err(e) = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)) {
            tracing::warn!(path = %path.display(), error = %e, "could not restrict database permissions");
        }
    }
}

#[cfg(not(unix))]
fn restrict_to_owner(_db_path: &Path) {}
