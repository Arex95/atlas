use crate::dtos::session::OrchestratorMessageResponse;
use crate::handlers::{err_internal, ok, ok_list};
use crate::socket_events;
use crate::terminal::TerminalManager;
use axum::Extension;
use axum::{
    extract::{Json, Path},
    response::IntoResponse,
};
use serde::Deserialize;
use socketioxide::SocketIo;
use sqlx::SqlitePool;
use std::sync::Arc;
use tracing::{error, info};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendMessageRequest {
    pub to_session_id: String,
    pub content: serde_json::Value,
}

pub async fn send_cross_session_message(
    Extension(io): Extension<SocketIo>,
    Extension(term_mgr): Extension<Arc<TerminalManager>>,
    Path(from_id): Path<String>,
    Json(payload): Json<SendMessageRequest>,
) -> impl IntoResponse {
    let target_room = payload.to_session_id.clone();

    info!(
        "[ORCHESTRATOR] Routing message from {} to session room {}",
        from_id, target_room
    );

    let msg_id = ulid::Ulid::new().to_string();
    let now = chrono::Utc::now().to_rfc3339();

    let _ = io
        .within("/")
        .to(target_room.clone())
        .emit(
            socket_events::SESSION_RECEIVE_MESSAGE,
            &serde_json::json!({
                "id": msg_id,
                "fromId": from_id,
                "content": payload.content,
                "timestamp": now,
                "isAgent": true
            }),
        )
        .await;

    let content_str = match &payload.content {
        serde_json::Value::String(s) => s.clone(),
        v => v.to_string(),
    };

    if let Err(e) = term_mgr
        .inject_message(&target_room, &from_id, &content_str)
        .await
    {
        error!("[ORCHESTRATOR] PTY Injection failed: {}", e);
    }

    ok(()).into_response()
}

pub async fn get_session_messages(
    axum::extract::State(pool): axum::extract::State<SqlitePool>,
    Path(session_id): Path<String>,
) -> impl IntoResponse {
    let messages = sqlx::query_as::<_, OrchestratorMessageResponse>(
        "SELECT id, from_id, content, timestamp FROM messages WHERE session_id = ? ORDER BY timestamp DESC LIMIT 50"
    )
    .bind(session_id)
    .fetch_all(&pool)
    .await;

    match messages {
        Ok(msgs) => ok_list(msgs).into_response(),
        Err(e) => err_internal("orchestrator", e).into_response(),
    }
}
