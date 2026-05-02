use crate::constants::errors;
use crate::handlers::{err, err_internal, ok};
use crate::services::memory as memory_svc;
use crate::socket_events;
use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde::Deserialize;
use sqlx::SqlitePool;
use tracing::{error, info};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetMemoryRequest {
    pub project_id: String,
    pub key: String,
    pub value: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryQuery {
    pub project_id: String,
}

pub async fn set_memory(
    State(pool): State<SqlitePool>,
    Json(payload): Json<SetMemoryRequest>,
) -> impl IntoResponse {
    match memory_svc::set(&pool, &payload.project_id, &payload.key, &payload.value).await {
        Ok(()) => ok(()).into_response(),
        Err(e) => err_internal("orchestrator", e).into_response(),
    }
}

pub async fn get_memory(
    State(pool): State<SqlitePool>,
    Path(key): Path<String>,
    Query(query): Query<MemoryQuery>,
) -> impl IntoResponse {
    match memory_svc::get(&pool, &query.project_id, &key).await {
        Ok(Some(value)) => ok(serde_json::json!({ "key": key, "value": value })).into_response(),
        Ok(None) => err(StatusCode::NOT_FOUND, errors::KEY_NOT_FOUND).into_response(),
        Err(e) => err_internal("orchestrator", e).into_response(),
    }
}

pub async fn list_memory(
    State(pool): State<SqlitePool>,
    Query(query): Query<MemoryQuery>,
) -> impl IntoResponse {
    match memory_svc::list(&pool, &query.project_id).await {
        Ok(rows) => ok(rows).into_response(),
        Err(e) => err_internal("orchestrator", e).into_response(),
    }
}

pub async fn delete_memory(
    State(pool): State<SqlitePool>,
    Path(key): Path<String>,
    Query(query): Query<MemoryQuery>,
) -> impl IntoResponse {
    match memory_svc::delete(&pool, &query.project_id, &key).await {
        Ok(true) => ok(()).into_response(),
        Ok(false) => err(StatusCode::NOT_FOUND, errors::KEY_NOT_FOUND).into_response(),
        Err(e) => err_internal("orchestrator", e).into_response(),
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatRequest {
    pub from_session_id: String,
    pub to_session_id: String,
    pub message: String,
    pub is_agent: Option<bool>,
}

pub async fn chat(
    axum::Extension(io): axum::Extension<socketioxide::SocketIo>,
    Json(payload): Json<ChatRequest>,
) -> impl IntoResponse {
    info!(
        "[ORCH] Attempting to send chat to room {}: {}",
        payload.to_session_id, payload.message
    );

    let res = io
        .to(payload.to_session_id.clone())
        .emit(
            socket_events::SESSION_RECEIVE_MESSAGE,
            &serde_json::json!({
                "fromId": payload.from_session_id,
                "content": payload.message,
                "isAgent": payload.is_agent.unwrap_or(false)
            }),
        )
        .await;

    match res {
        Ok(_) => info!(
            "[ORCH] Socket emission successful for room {}",
            payload.to_session_id
        ),
        Err(e) => error!("[ORCH] Socket emission failed: {}", e),
    }

    ok(()).into_response()
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotifyRequest {
    pub session_id: String,
    pub project_id: Option<String>,
    pub message: String,
    pub r#type: String,
}

pub async fn notify(
    State(pool): State<SqlitePool>,
    axum::Extension(io): axum::Extension<socketioxide::SocketIo>,
    Json(payload): Json<NotifyRequest>,
) -> impl IntoResponse {
    info!(
        "[ORCH] Notification from {}: {}",
        payload.session_id, payload.message
    );

    let id = ulid::Ulid::new().to_string();
    let _ = crate::repositories::notification::create(
        &pool,
        &id,
        payload.project_id.as_deref(),
        Some(&payload.session_id),
        None,
        &payload.message,
        &payload.r#type,
    )
    .await;

    let _ = io
        .emit(
            socket_events::NOTIFICATION_NEW,
            &serde_json::json!({
                "id": &id,
                "sessionId": payload.session_id,
                "projectId": payload.project_id,
                "message": payload.message,
                "type": payload.r#type
            }),
        )
        .await;

    ok(()).into_response()
}

