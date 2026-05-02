use crate::dtos::task::{CreateTaskRequest, TaskResponse, UpdateTaskRequest};
use crate::repositories::task as repo;
use sqlx::SqlitePool;

pub async fn list(
    pool: &SqlitePool,
    project_id: Option<&str>,
    status: Option<&str>,
    parent_id: Option<&str>,
) -> sqlx::Result<Vec<TaskResponse>> {
    repo::find_all(pool, project_id, status, parent_id)
        .await
        .map(|rows| rows.into_iter().map(TaskResponse::from).collect())
}

pub async fn create(pool: &SqlitePool, body: &CreateTaskRequest) -> sqlx::Result<TaskResponse> {
    let id = ulid::Ulid::new().to_string();
    let description = body.description.as_deref().unwrap_or("");
    let status = body.status.as_deref().unwrap_or("todo");
    let priority = body.priority.as_deref().unwrap_or("medium");
    let tags_json = serde_json::to_string(&body.tags.clone().unwrap_or_default())
        .unwrap_or_else(|_| "[]".to_string());

    repo::create(
        pool,
        &id,
        &body.project_id,
        &body.title,
        description,
        status,
        priority,
        body.due_date.as_deref(),
        body.assigned_to.as_deref(),
        &tags_json,
        body.parent_id.as_deref(),
    )
    .await
    .map(TaskResponse::from)
}

pub async fn update(
    pool: &SqlitePool,
    id: &str,
    body: &UpdateTaskRequest,
) -> sqlx::Result<Option<TaskResponse>> {
    let tags_json = body
        .tags
        .as_ref()
        .map(|t| serde_json::to_string(t).unwrap_or_else(|_| "[]".to_string()));

    repo::update(
        pool,
        id,
        body.title.as_deref(),
        body.description.as_deref(),
        body.status.as_deref(),
        body.priority.as_deref(),
        body.due_date.as_deref(),
        body.assigned_to.as_deref(),
        tags_json.as_deref(),
        body.parent_id.as_deref(),
    )
    .await
    .map(|opt| opt.map(TaskResponse::from))
}

pub async fn delete(pool: &SqlitePool, id: &str) -> sqlx::Result<bool> {
    repo::delete(pool, id).await
}
