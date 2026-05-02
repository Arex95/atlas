use crate::dtos::notification::{CreateNotificationRequest, NotificationResponse};
use crate::models::Notification;
use crate::repositories::notification as repo;
use sqlx::SqlitePool;

pub async fn list(
    pool: &SqlitePool,
    project_id: Option<&str>,
    status: Option<&str>,
) -> sqlx::Result<Vec<NotificationResponse>> {
    repo::find_all(pool, project_id, status)
        .await
        .map(|rows| rows.into_iter().map(NotificationResponse::from).collect())
}

pub async fn create(
    pool: &SqlitePool,
    payload: &CreateNotificationRequest,
) -> sqlx::Result<Notification> {
    let id = ulid::Ulid::new().to_string();
    repo::create(
        pool,
        &id,
        payload.project_id.as_deref(),
        payload.session_id.as_deref(),
        payload.title.as_deref(),
        &payload.message,
        &payload.kind,
    )
    .await
}

pub async fn mark_all_read(pool: &SqlitePool) -> sqlx::Result<()> {
    repo::mark_all_read(pool).await
}

pub async fn delete(pool: &SqlitePool, id: &str) -> sqlx::Result<bool> {
    repo::delete(pool, id).await
}
