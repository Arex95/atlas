use socketioxide::SocketIo;
use socketioxide::extract::{Data, SocketRef};
use sqlx::SqlitePool;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tower::ServiceBuilder;
use tower::BoxError;
use tower_http::{
    cors::CorsLayer,
    request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer},
    trace::TraceLayer,
};
use tracing::{error, info, warn};

mod constants;
mod db;
mod dtos;
mod git;
mod handlers;
mod indexer;
mod mcp;
mod middleware;
mod models;
mod prompts;
mod repositories;
mod routes;
mod services;
mod socket_events;
mod terminal;

pub use terminal::TerminalManager;

async fn handle_session_spawn(
    socket: SocketRef,
    data: serde_json::Value,
    pool: SqlitePool,
    term_mgr: Arc<TerminalManager>,
) {
    info!("[WS] Received session:spawn from {}: {:?}", socket.id, data);
    let session_id = match data.get("sessionId").and_then(|v| v.as_str()) {
        Some(id) => id.to_string(),
        None => return,
    };

    let session =
        sqlx::query_as::<_, crate::models::AiSession>("SELECT * FROM ai_sessions WHERE id = ?")
            .bind(&session_id)
            .fetch_optional(&pool)
            .await
            .unwrap_or_default();

    let s = match session {
        Some(s) => s,
        None => return,
    };

    let sid = s.id.clone();
    let dir = s.working_directory.clone();
    let sid_clone = sid.clone();

    let res = term_mgr
        .get_or_create_session(sid.clone(), s.project_id.clone().unwrap_or_default(), dir)
        .await;

    match res {
        Ok((mut rx, scrollback)) => {
            info!(
                "[WS] PTY linked for {}. Sending scrollback ({} chars).",
                sid_clone,
                scrollback.len()
            );

            if !scrollback.is_empty() {
                let _ = socket.emit(
                    socket_events::TERMINAL_OUTPUT,
                    &serde_json::json!({
                        "sessionId": sid_clone,
                        "output": scrollback
                    }),
                );
            }

            let socket_emit = socket.clone();
            let sid_emit = sid.clone();
            tokio::spawn(async move {
                while let Ok(output_str) = rx.recv().await {
                    let _ = socket_emit.emit(
                        socket_events::TERMINAL_OUTPUT,
                        &serde_json::json!({
                            "sessionId": sid_emit,
                            "output": output_str
                        }),
                    );
                }
            });
        }
        Err(e) => {
            error!("[WS] Failed to get/create session: {}", e);
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    info!("Starting Atlas Orchestrator (Rust)...");

    let _ = dotenvy::dotenv();

    let database_url = std::env::var(constants::env::DATABASE_URL).unwrap_or_else(|_| {
        let cwd = std::env::current_dir().expect("cwd unavailable");
        let db_path = cwd.join("atlas-data/atlas.db");
        if let Some(parent) = db_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        format!("sqlite://{}", db_path.display())
    });
    info!("DATABASE_URL = {}", database_url);

    match std::env::var(constants::env::ATLAS_API_TOKEN) {
        Ok(t) if !t.is_empty() => info!("REST API auth: ENABLED (ATLAS_API_TOKEN set)"),
        _ => warn!(
            "REST API auth: DISABLED (ATLAS_API_TOKEN unset). \
             Any local process can read/write all Atlas data. \
             Set ATLAS_API_TOKEN in .env for any non-loopback deployment."
        ),
    }

    match std::env::var(constants::env::ATLAS_MCP_TOKEN) {
        Ok(t) if !t.is_empty() => info!("MCP auth: ENABLED (ATLAS_MCP_TOKEN set)"),
        _ => warn!(
            "MCP auth: DISABLED (ATLAS_MCP_TOKEN unset). \
             Anyone reaching /api/mcp can list/read your projects. \
             Set ATLAS_MCP_TOKEN in .env for any non-loopback deployment."
        ),
    }

    let pool = db::init_db(&database_url).await?;

    let reminder_pool = pool.clone();
    let (reminder_io_tx, mut reminder_io_rx) = tokio::sync::mpsc::channel::<serde_json::Value>(64);
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
        loop {
            interval.tick().await;
            let now = chrono::Utc::now();
            let now_str = now.to_rfc3339();
            #[derive(sqlx::FromRow)]
            struct DueReminder {
                id: String,
                title: String,
                project_id: Option<String>,
            }
            let due = sqlx::query_as::<_, DueReminder>(
                "SELECT id, title, project_id FROM reminders
                 WHERE status = 'pending' AND due_at <= ?
                 AND (last_notified_at IS NULL OR last_notified_at < datetime(due_at, '-1 minute'))",
            )
            .bind(&now_str)
            .fetch_all(&reminder_pool)
            .await
            .unwrap_or_default();

            for r in due {
                let notif_id = ulid::Ulid::new().to_string();
                let _ = sqlx::query(
                    "INSERT INTO notifications (id, project_id, title, message, type) VALUES (?, ?, ?, ?, 'reminder')",
                )
                .bind(&notif_id)
                .bind(&r.project_id)
                .bind(&r.title)
                .bind(format!("Reminder due: {}", r.title))
                .execute(&reminder_pool)
                .await;

                let _ = sqlx::query(
                    "UPDATE reminders SET last_notified_at = ? WHERE id = ?",
                )
                .bind(&now_str)
                .bind(&r.id)
                .execute(&reminder_pool)
                .await;

                let _ = reminder_io_tx
                    .send(serde_json::json!({
                        "id": notif_id,
                        "title": r.title,
                        "message": format!("Reminder due: {}", r.title),
                        "type": "reminder",
                        "projectId": r.project_id
                    }))
                    .await;
            }
        }
    });

    let term_mgr = Arc::new(TerminalManager::new(pool.clone()));

    let (layer, io) = SocketIo::builder().build_layer();

    let io_reminders = io.clone();
    tokio::spawn(async move {
        while let Some(payload) = reminder_io_rx.recv().await {
            let _ = io_reminders
                .emit(socket_events::NOTIFICATION_NEW, &payload)
                .await;
        }
    });

    let io_events = io.clone();
    let mut events_rx = term_mgr.events_tx.subscribe();
    tokio::spawn(async move {
        while let Ok(event) = events_rx.recv().await {
            match event {
                terminal::TerminalEvent::PathUpdated {
                    session_id,
                    new_path,
                } => {
                    info!(
                        "[WS] Emitting session:updated for {} -> {}",
                        session_id, new_path
                    );
                    let _ = io_events.within("/").to(session_id.clone()).emit(
                        socket_events::SESSION_UPDATED,
                        &serde_json::json!({ "sessionId": session_id, "workingDirectory": new_path })
                    ).await;
                }
            }
        }
    });

    let pool_ws_root = pool.clone();
    let term_mgr_ws_root = term_mgr.clone();
    let io_handle = io.clone();

    io.ns("/", move |socket: SocketRef| async move {
        let pool_ws = pool_ws_root.clone();
        let term_mgr_ws = term_mgr_ws_root.clone();
        let io_msg = io_handle.clone();

        socket.on(socket_events::SUBSCRIBE_SESSION, move |socket: SocketRef, Data::<serde_json::Value>(data)| async move {
            let session_id = data.as_str().map(|s| s.to_string())
                .or_else(|| data.get("sessionId").and_then(|v| v.as_str()).map(|s| s.to_string()))
                .unwrap_or_default();

            if !session_id.is_empty() {
                info!("[WS] Socket {} joining session room: {}", socket.id, session_id);
                socket.join(session_id);
            } else {
                warn!("[WS] Received invalid subscribe:session payload: {:?}", data);
            }
        });

        let tm_resize = term_mgr_ws.clone();
        socket.on(socket_events::TERMINAL_RESIZE, move |_socket: SocketRef, Data::<crate::dtos::session::TerminalResize>(data)| async move {
            tm_resize.resize_session(&data.session_id, data.rows, data.cols).await;
        });

        let tm_input = term_mgr_ws.clone();
        socket.on(socket_events::TERMINAL_INPUT, move |socket: SocketRef, Data::<serde_json::Value>(data)| async move {
            let session_id = data.get("sessionId").and_then(|v| v.as_str()).unwrap_or_default();
            let input = data.get("data").and_then(|v| v.as_str()).unwrap_or_default();

            if input.is_empty() { return; }

            match tm_input.write_input(session_id, input).await {
                Ok((Some(cmd), _)) => {
                    let _ = socket.emit(socket_events::TERMINAL_SECURITY_ALERT, &serde_json::json!({
                        "sessionId": session_id,
                        "command": cmd
                    }));
                },
                Ok((None, _)) => {},
                Err(e) => error!("[WS] Write error: {}", e),
            }
        });

        let tm_force = term_mgr_ws.clone();
        socket.on(socket_events::TERMINAL_FORCE_WRITE, move |_: SocketRef, Data::<serde_json::Value>(data)| async move {
            let session_id = data.get("sessionId").and_then(|v| v.as_str()).unwrap_or_default();
            let input = data.get("data").and_then(|v| v.as_str()).unwrap_or_default();
            tm_force.force_write_input(session_id, input).await;
        });

        let io_msg_inner = io_msg.clone();
        socket.on(socket_events::SESSION_MESSAGE, move |_socket: SocketRef, Data::<serde_json::Value>(data)| async move {
            if let (Some(from_id), Some(to_id), Some(content)) = (
                data.get("fromId").and_then(|v| v.as_str()),
                data.get("toId").and_then(|v| v.as_str()),
                data.get("content")
            ) {
                let _ = io_msg_inner.within("/").to(to_id.to_string()).emit(
                    socket_events::SESSION_RECEIVE_MESSAGE,
                    &serde_json::json!({ "fromId": from_id, "content": content, "isAgent": true })
                ).await;
            }
        });

        let p_spawn = pool_ws.clone();
        let tm_spawn = term_mgr_ws.clone();
        socket.on(socket_events::SESSION_SPAWN, move |socket: SocketRef, Data(data): Data<serde_json::Value>| async move {
            handle_session_spawn(socket, data, p_spawn, tm_spawn).await;
        });
    });

    let cors_origin = std::env::var(constants::env::WEB_ORIGIN)
        .unwrap_or_else(|_| constants::defaults::WEB_ORIGIN.to_string());
    let cors_origin_header = cors_origin
        .parse::<axum::http::HeaderValue>()
        .map_err(|e| format!("Invalid WEB_ORIGIN '{}': {}", cors_origin, e))?;
    let cors = CorsLayer::new()
        .allow_origin(cors_origin_header)
        .allow_methods([
            axum::http::Method::GET,
            axum::http::Method::POST,
            axum::http::Method::PATCH,
            axum::http::Method::OPTIONS,
            axum::http::Method::PUT,
            axum::http::Method::DELETE,
        ])
        .allow_headers([
            axum::http::header::CONTENT_TYPE,
            axum::http::header::AUTHORIZATION,
            axum::http::header::ACCEPT,
        ])
        .allow_credentials(true);

    let app = routes::router()
        .layer(axum::Extension(io.clone()))
        .layer(axum::Extension(term_mgr.clone()))
        .layer(layer)
        .layer(
            ServiceBuilder::new()
                .layer(axum::error_handling::HandleErrorLayer::new(
                    |e: BoxError| async move {
                        if e.is::<tower::timeout::error::Elapsed>() {
                            axum::http::StatusCode::REQUEST_TIMEOUT
                        } else {
                            axum::http::StatusCode::INTERNAL_SERVER_ERROR
                        }
                    },
                ))
                .layer(tower::timeout::TimeoutLayer::new(Duration::from_secs(30)))
                .layer(SetRequestIdLayer::x_request_id(MakeRequestUuid))
                .layer(TraceLayer::new_for_http())
                .layer(PropagateRequestIdLayer::x_request_id())
                .layer(cors),
        )
        .with_state(pool);

    let port = std::env::var(constants::env::PORT)
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(constants::defaults::PORT);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    info!("Server listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
