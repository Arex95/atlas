use crate::handlers::{ok};
use crate::socket_events;
use axum::{
    Json,
    extract::{Extension, State},
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use socketioxide::SocketIo;
use sqlx::SqlitePool;
use ulid::Ulid;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebhookTriggerRequest {
    pub event: String,
    pub project_id: Option<String>,
    pub message: Option<String>,
    pub session_id: Option<String>,
    pub data: Option<serde_json::Value>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebhookTriggerResponse {
    pub event_id: String,
    pub event: String,
}

pub async fn trigger_webhook(
    State(pool): State<SqlitePool>,
    Extension(io): Extension<SocketIo>,
    Json(payload): Json<WebhookTriggerRequest>,
) -> impl IntoResponse {
    let event_id = Ulid::new().to_string();
    let message = payload
        .message
        .as_deref()
        .unwrap_or(&payload.event)
        .to_string();

    let _ = sqlx::query(
        "INSERT INTO notifications (id, project_id, title, message, type) VALUES (?, ?, ?, ?, 'info')",
    )
    .bind(&event_id)
    .bind(&payload.project_id)
    .bind(format!("Webhook: {}", payload.event))
    .bind(&message)
    .execute(&pool)
    .await;

    let socket_payload = serde_json::json!({
        "id": &event_id,
        "title": format!("Webhook: {}", payload.event),
        "message": &message,
        "type": "info",
        "projectId": payload.project_id,
        "data": payload.data,
    });

    if let Some(ref sid) = payload.session_id {
        let _ = io
            .within("/")
            .to(sid.clone())
            .emit(socket_events::NOTIFICATION_NEW, &socket_payload)
            .await;
    } else {
        let _ = io.emit(socket_events::NOTIFICATION_NEW, &socket_payload).await;
    }

    ok(WebhookTriggerResponse {
        event_id,
        event: payload.event,
    })
    .into_response()
}
