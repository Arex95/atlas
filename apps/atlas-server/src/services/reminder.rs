use crate::dtos::reminder::{CreateReminderRequest, ReminderResponse, UpdateReminderRequest};
use crate::repositories::reminder as repo;
use sqlx::SqlitePool;

pub async fn list(
    pool: &SqlitePool,
    project_id: Option<&str>,
    status: Option<&str>,
) -> sqlx::Result<Vec<ReminderResponse>> {
    repo::find_all(pool, project_id, status)
        .await
        .map(|rows| rows.into_iter().map(ReminderResponse::from).collect())
}

pub async fn create(pool: &SqlitePool, payload: &CreateReminderRequest) -> sqlx::Result<ReminderResponse> {
    let id = ulid::Ulid::new().to_string();
    let description = payload.description.clone().unwrap_or_default();
    let kind = payload.kind.as_str();

    repo::create(
        pool,
        &id,
        payload.project_id.as_deref(),
        None,
        &payload.title,
        &description,
        &payload.due_at,
        kind,
    )
    .await
    .map(ReminderResponse::from)
}

pub async fn update(
    pool: &SqlitePool,
    id: &str,
    payload: &UpdateReminderRequest,
    existing: crate::models::Reminder,
) -> sqlx::Result<ReminderResponse> {
    let title = payload.title.clone().unwrap_or(existing.title);
    let description = payload.description.clone().unwrap_or(existing.description);
    let due_at = payload.due_at.clone().unwrap_or(existing.due_at);
    let status = payload.status.clone().unwrap_or(existing.status);

    repo::update(pool, id, &title, &description, &due_at, &status)
        .await
        .map(ReminderResponse::from)
}

pub async fn delete(pool: &SqlitePool, id: &str) -> sqlx::Result<bool> {
    repo::delete(pool, id).await
}
