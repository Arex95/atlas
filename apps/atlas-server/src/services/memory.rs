use crate::repositories::memory::{self as repo, MemoryRow};
use sqlx::SqlitePool;

/// Resolve project_id: accepts either the ULID id or the slug.
async fn resolve_project_id(pool: &SqlitePool, project_id: &str) -> sqlx::Result<String> {
    let row: Option<(String,)> = sqlx::query_as(
        "SELECT id FROM projects WHERE id = ? OR slug = ? LIMIT 1",
    )
    .bind(project_id)
    .bind(project_id)
    .fetch_optional(pool)
    .await?;
    row.map(|(id,)| id)
        .ok_or_else(|| sqlx::Error::RowNotFound)
}

pub async fn set(pool: &SqlitePool, project_id: &str, key: &str, value: &str) -> sqlx::Result<()> {
    let resolved = resolve_project_id(pool, project_id).await?;
    let id = ulid::Ulid::new().to_string();
    repo::upsert(pool, &id, &resolved, key, value).await
}

pub async fn get(pool: &SqlitePool, project_id: &str, key: &str) -> sqlx::Result<Option<String>> {
    let resolved = resolve_project_id(pool, project_id).await?;
    repo::find_value(pool, &resolved, key).await
}

pub async fn list(pool: &SqlitePool, project_id: &str) -> sqlx::Result<Vec<MemoryRow>> {
    let resolved = resolve_project_id(pool, project_id).await?;
    repo::find_all(pool, &resolved).await
}

pub async fn delete(pool: &SqlitePool, project_id: &str, key: &str) -> sqlx::Result<bool> {
    let resolved = resolve_project_id(pool, project_id).await?;
    repo::delete(pool, &resolved, key).await
}
