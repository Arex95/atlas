use crate::dtos::document::{CreateDocumentRequest, DocumentResponse, UpdateDocumentRequest};
use crate::repositories::document as repo;
use sqlx::SqlitePool;

pub async fn list(
    pool: &SqlitePool,
    project_id: &str,
    kind: Option<&str>,
) -> sqlx::Result<Vec<DocumentResponse>> {
    repo::find_all(pool, project_id, kind)
        .await
        .map(|rows| rows.into_iter().map(DocumentResponse::from).collect())
}

pub async fn get(pool: &SqlitePool, id: &str) -> sqlx::Result<Option<DocumentResponse>> {
    repo::find_by_id(pool, id)
        .await
        .map(|opt| opt.map(DocumentResponse::from))
}

pub async fn create(pool: &SqlitePool, payload: &CreateDocumentRequest) -> sqlx::Result<DocumentResponse> {
    let id = ulid::Ulid::new().to_string();
    let tags = serde_json::to_string(&payload.tags.clone().unwrap_or_default()).unwrap_or_default();
    let content = payload.content.clone().unwrap_or_default();

    repo::create(pool, &id, &payload.project_id, &payload.title, &content, &payload.kind, &tags)
        .await
        .map(DocumentResponse::from)
}

pub async fn update(
    pool: &SqlitePool,
    id: &str,
    payload: &UpdateDocumentRequest,
    existing: crate::models::ProjectDocument,
) -> sqlx::Result<DocumentResponse> {
    let title = payload.title.clone().unwrap_or(existing.title);
    let content = payload.content.clone().unwrap_or(existing.content);
    let tags = payload
        .tags
        .as_ref()
        .map(|t| serde_json::to_string(t).unwrap_or_default())
        .unwrap_or(existing.tags);

    repo::update(pool, id, &title, &content, &tags)
        .await
        .map(DocumentResponse::from)
}

pub async fn delete(pool: &SqlitePool, id: &str) -> sqlx::Result<bool> {
    repo::delete(pool, id).await
}
