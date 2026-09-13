//! Integration tests for `SyncSupervisor` (`auto` mode) —
//! real bound servers, a real background `tokio` task, no mocked
//! timers. Intervals are kept short (1-2s) to stay fast while still
//! proving the loop actually fires on its own.

use std::time::Duration;

use atlas_auth::api::{AuthStore, SqlitePool as AuthPool, run_migrations as run_auth_migrations};
use atlas_sessions::api::{
    NewSession, SessionStore, SqlitePool as SessionsPool, run_migrations as run_sessions_migrations,
};
use atlas_sync::api::{SyncConfig, SyncMode, SyncSupervisor, router};

async fn fresh_note_store() -> atlas_notes::api::NoteStore {
    let pool = atlas_notes::api::SqlitePool::connect("sqlite::memory:")
        .await
        .unwrap();
    atlas_notes::api::run_migrations(&pool).await.unwrap();
    atlas_notes::api::NoteStore::new(pool)
}

/// A memory store for the sync engine to carry alongside sessions.
/// Its own in-memory database: memory rows share no tables with
/// sessions, so nothing here needs the same pool.
async fn fresh_memory_store() -> atlas_memory::api::MemoryStore {
    let pool = atlas_memory::api::SqlitePool::connect("sqlite::memory:")
        .await
        .unwrap();
    atlas_memory::api::run_migrations(&pool).await.unwrap();
    atlas_memory::api::MemoryStore::new(pool)
}

struct RemoteServer {
    base_url: String,
    auth: AuthStore,
    sessions: SessionStore,
}

async fn spawn_remote() -> RemoteServer {
    let pool = AuthPool::connect("sqlite::memory:").await.unwrap();
    run_auth_migrations(&pool).await.unwrap();
    run_sessions_migrations(&pool).await.unwrap();
    let auth = AuthStore::new(pool.clone());
    let sessions = SessionStore::new(pool);

    let app = router(
        auth.clone(),
        sessions.clone(),
        fresh_memory_store().await,
        fresh_note_store().await,
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    RemoteServer {
        base_url: format!("http://{addr}/"),
        auth,
        sessions,
    }
}

async fn fresh_local_store() -> SessionStore {
    let pool = SessionsPool::connect("sqlite::memory:").await.unwrap();
    run_sessions_migrations(&pool).await.unwrap();
    SessionStore::new(pool)
}

fn sample(owner_id: &str, relative_path: &str) -> NewSession {
    NewSession {
        project: "your-org/your-project".to_owned(),
        owner_id: owner_id.to_owned(),
        remote_url: "git@github.com:you/your-project.git".to_owned(),
        branch: "main".to_owned(),
        relative_path: relative_path.to_owned(),
        agent_kind: None,
        title: None,
        resume_command: None,
    }
}

/// Poll `store.list_since(owner_id, None)` until it's non-empty or
/// the deadline passes. Returns the row count seen (0 on timeout).
async fn wait_for_session(store: &SessionStore, owner_id: &str, deadline: Duration) -> usize {
    let end = tokio::time::Instant::now() + deadline;
    loop {
        let rows = store.list_since(owner_id, None).await.unwrap();
        if !rows.is_empty() || tokio::time::Instant::now() >= end {
            return rows.len();
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

#[tokio::test]
async fn auto_mode_converges_both_sides_without_a_manual_trigger() {
    let remote = spawn_remote().await;
    let issued = remote
        .auth
        .register("dev@example.com", "correct horse battery", "Dev")
        .await
        .unwrap();
    let owner_id = issued.user.id.0.clone();

    let local = fresh_local_store().await;
    local
        .create(sample(&owner_id, "laptop-workspace"))
        .await
        .unwrap();

    let supervisor = SyncSupervisor::new(
        local.clone(),
        fresh_memory_store().await,
        fresh_note_store().await,
    );
    supervisor
        .set_auto(SyncConfig {
            remote_url: remote.base_url.clone(),
            bearer_token: issued.token.clone(),
            owner_id: owner_id.clone(),
            interval_secs: 1,
        })
        .await
        .unwrap();

    // No manual sync.sessions_now call anywhere in this test — the
    // loop is what has to move this session across.
    let count = wait_for_session(&remote.sessions, &owner_id, Duration::from_secs(5)).await;
    assert_eq!(count, 1);

    let status = supervisor.status().await;
    assert_eq!(status.mode, SyncMode::Auto);
    assert!(status.last_report.is_some());
}

#[tokio::test]
async fn set_focus_stops_the_loop() {
    let remote = spawn_remote().await;
    let issued = remote
        .auth
        .register("dev@example.com", "correct horse battery", "Dev")
        .await
        .unwrap();
    let owner_id = issued.user.id.0.clone();
    let local = fresh_local_store().await;

    let supervisor = SyncSupervisor::new(
        local.clone(),
        fresh_memory_store().await,
        fresh_note_store().await,
    );
    supervisor
        .set_auto(SyncConfig {
            remote_url: remote.base_url.clone(),
            bearer_token: issued.token.clone(),
            owner_id: owner_id.clone(),
            interval_secs: 1,
        })
        .await
        .unwrap();

    // Let at least one tick happen so we know the loop was live.
    tokio::time::sleep(Duration::from_millis(1500)).await;
    assert_eq!(supervisor.status().await.mode, SyncMode::Auto);

    supervisor.set_focus().await;
    assert_eq!(supervisor.status().await.mode, SyncMode::Focus);

    // A session created after switching back to focus must NOT
    // appear on the remote — nothing should be ticking anymore.
    local
        .create(sample(&owner_id, "should-not-sync"))
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_secs(2)).await;
    let on_remote = remote.sessions.list_since(&owner_id, None).await.unwrap();
    assert!(on_remote.is_empty());
}

#[tokio::test]
async fn set_auto_twice_replaces_the_running_loop_instead_of_leaking_one() {
    let remote = spawn_remote().await;
    let issued = remote
        .auth
        .register("dev@example.com", "correct horse battery", "Dev")
        .await
        .unwrap();
    let owner_id = issued.user.id.0.clone();
    let local = fresh_local_store().await;

    let supervisor = SyncSupervisor::new(
        local.clone(),
        fresh_memory_store().await,
        fresh_note_store().await,
    );
    supervisor
        .set_auto(SyncConfig {
            remote_url: remote.base_url.clone(),
            bearer_token: issued.token.clone(),
            owner_id: owner_id.clone(),
            interval_secs: 1,
        })
        .await
        .unwrap();

    // Reconfigure with a bearer token the remote will reject. If the
    // first (valid-token) loop were still running alongside this
    // one — i.e. leaked, not replaced — it would still push new
    // sessions through successfully.
    supervisor
        .set_auto(SyncConfig {
            remote_url: remote.base_url.clone(),
            bearer_token: "revoked-or-never-issued".to_owned(),
            owner_id: owner_id.clone(),
            interval_secs: 1,
        })
        .await
        .unwrap();

    local
        .create(sample(&owner_id, "should-not-sync"))
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_secs(3)).await;

    let on_remote = remote.sessions.list_since(&owner_id, None).await.unwrap();
    assert!(
        on_remote.is_empty(),
        "a session synced through despite the active loop holding a bad token — \
         the first (valid) loop was not actually stopped"
    );

    let status = supervisor.status().await;
    assert!(status.last_error.is_some());
}

#[tokio::test]
async fn set_auto_with_a_bad_url_fails_immediately() {
    let local = fresh_local_store().await;
    let supervisor =
        SyncSupervisor::new(local, fresh_memory_store().await, fresh_note_store().await);
    let err = supervisor
        .set_auto(SyncConfig {
            remote_url: "not a url".to_owned(),
            bearer_token: "irrelevant".to_owned(),
            owner_id: "owner-a".to_owned(),
            interval_secs: 1,
        })
        .await
        .unwrap_err();
    assert!(matches!(err, atlas_sync::api::SyncError::BadUrl(_)));
}
