use crate::constants::errors;
use crate::dtos::notification::{CreateNotificationRequest, NotificationResponse};
use crate::handlers::{err, err_internal, ok, ok_list, validate};
use crate::services::notification as svc;
use crate::socket_events;
use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde::Deserialize;
use socketioxide::SocketIo;
use sqlx::SqlitePool;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotificationQuery {
    pub project_id: Option<String>,
    pub status: Option<String>,
}

pub async fn list_notifications(
    State(pool): State<SqlitePool>,
    Query(q): Query<NotificationQuery>,
) -> impl IntoResponse {
    match svc::list(&pool, q.project_id.as_deref(), q.status.as_deref()).await {
        Ok(notifs) => ok_list(notifs).into_response(),
        Err(e) => err_internal("notification", e).into_response(),
    }
}

pub async fn create_notification(
    State(pool): State<SqlitePool>,
    axum::Extension(io): axum::Extension<SocketIo>,
    Json(payload): Json<CreateNotificationRequest>,
) -> impl IntoResponse {
    if let Err(e) = validate(&payload) {
        return e.into_response();
    }

    match svc::create(&pool, &payload).await {
        Ok(n) => {
            let response = NotificationResponse::from(n);
            let _ = io
                .emit(
                    socket_events::NOTIFICATION_NEW,
                    &serde_json::json!({
                        "id": &response.id,
                        "title": &response.title,
                        "message": &response.message,
                        "type": &response.kind,
                        "projectId": &response.project_id,
                        "createdAt": &response.created_at
                    }),
                )
                .await;
            ok(response).into_response()
        }
        Err(e) => err_internal("notification", e).into_response(),
    }
}

pub async fn mark_all_read(State(pool): State<SqlitePool>) -> impl IntoResponse {
    match svc::mark_all_read(&pool).await {
        Ok(()) => ok(()).into_response(),
        Err(e) => err_internal("notification", e).into_response(),
    }
}

pub async fn delete_notification(
    State(pool): State<SqlitePool>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match svc::delete(&pool, &id).await {
        Ok(true) => ok(()).into_response(),
        Ok(false) => err(StatusCode::NOT_FOUND, errors::NOTIFICATION_NOT_FOUND).into_response(),
        Err(e) => err_internal("notification", e).into_response(),
    }
}
