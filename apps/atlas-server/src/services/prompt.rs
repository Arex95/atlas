use crate::dtos::prompt::{CreatePromptRequest, PromptResponse, UpdatePromptRequest};
use crate::repositories::prompt as repo;
use sqlx::SqlitePool;

pub async fn list(
    pool: &SqlitePool,
    project_id: Option<&str>,
    session_id: Option<&str>,
    category: Option<&str>,
) -> sqlx::Result<Vec<PromptResponse>> {
    let rows = repo::find_all(pool, project_id, session_id).await?;
    let mut result: Vec<PromptResponse> = rows.into_iter().map(PromptResponse::from).collect();
    if let Some(cat) = category {
        result.retain(|p| p.category == cat);
    }
    Ok(result)
}

pub async fn get(pool: &SqlitePool, id: &str) -> sqlx::Result<Option<PromptResponse>> {
    repo::find_by_id(pool, id)
        .await
        .map(|opt| opt.map(PromptResponse::from))
}

pub async fn create(pool: &SqlitePool, payload: &CreatePromptRequest) -> sqlx::Result<PromptResponse> {
    let id = ulid::Ulid::new().to_string();
    repo::create(
        pool,
        &id,
        payload.project_id.as_deref(),
        payload.session_id.as_deref(),
        &payload.title,
        &payload.content,
        &payload.category,
    )
    .await
    .map(PromptResponse::from)
}

pub async fn update(
    pool: &SqlitePool,
    id: &str,
    payload: &UpdatePromptRequest,
    existing: crate::models::Prompt,
) -> sqlx::Result<PromptResponse> {
    let title = payload.title.clone().unwrap_or(existing.title);
    let content = payload.content.clone().unwrap_or(existing.content);
    let category = payload.category.clone().unwrap_or(existing.category);

    repo::update(pool, id, &title, &content, &category)
        .await
        .map(PromptResponse::from)
}

pub async fn delete(pool: &SqlitePool, id: &str) -> sqlx::Result<bool> {
    repo::delete(pool, id).await
}
