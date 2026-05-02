use crate::models::Project;
use sqlx::SqlitePool;

pub async fn find_all(pool: &SqlitePool) -> sqlx::Result<Vec<Project>> {
    sqlx::query_as::<_, Project>("SELECT * FROM projects ORDER BY created_at DESC")
        .fetch_all(pool)
        .await
}

pub async fn find_by_slug(pool: &SqlitePool, slug: &str) -> sqlx::Result<Option<Project>> {
    sqlx::query_as::<_, Project>("SELECT * FROM projects WHERE slug = ?")
        .bind(slug)
        .fetch_optional(pool)
        .await
}

#[allow(clippy::too_many_arguments)]
pub async fn upsert(
    pool: &SqlitePool,
    id: &str,
    slug: &str,
    name: &str,
    description: &str,
    root_path: &str,
    index_path: &str,
    color: Option<&str>,
) -> sqlx::Result<Project> {
    sqlx::query_as::<_, Project>(
        "INSERT INTO projects (id, slug, name, description, root_path, index_path, color)
         VALUES (?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(slug) DO UPDATE SET
           name        = excluded.name,
           description = excluded.description,
           root_path   = excluded.root_path,
           index_path  = excluded.index_path,
           color       = excluded.color,
           updated_at  = CURRENT_TIMESTAMP
         RETURNING *",
    )
    .bind(id)
    .bind(slug)
    .bind(name)
    .bind(description)
    .bind(root_path)
    .bind(index_path)
    .bind(color)
    .fetch_one(pool)
    .await
}

#[allow(clippy::too_many_arguments)]
pub async fn update(
    pool: &SqlitePool,
    id: &str,
    name: &str,
    description: Option<&str>,
    color: Option<&str>,
    root_path: &str,
    index_path: &str,
    version: &str,
    author: Option<&str>,
) -> sqlx::Result<Project> {
    sqlx::query_as::<_, Project>(
        "UPDATE projects SET name = ?, description = ?, color = ?, root_path = ?, index_path = ?, version = ?, author = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ? RETURNING *",
    )
    .bind(name)
    .bind(description)
    .bind(color)
    .bind(root_path)
    .bind(index_path)
    .bind(version)
    .bind(author)
    .bind(id)
    .fetch_one(pool)
    .await
}

pub async fn delete(pool: &SqlitePool, slug: &str) -> sqlx::Result<bool> {
    let res = sqlx::query("DELETE FROM projects WHERE slug = ?")
        .bind(slug)
        .execute(pool)
        .await?;
    Ok(res.rows_affected() > 0)
}

pub async fn touch_synced(pool: &SqlitePool, id: &str) -> sqlx::Result<()> {
    sqlx::query("UPDATE projects SET last_synced_at = CURRENT_TIMESTAMP WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn update_simple(
    pool: &SqlitePool,
    slug: &str,
    name: Option<&str>,
    description: Option<&str>,
    color: Option<&str>,
    version: Option<&str>,
    author: Option<&str>,
) -> sqlx::Result<bool> {
    let res = sqlx::query(
        "UPDATE projects SET name = COALESCE(?1, name), description = COALESCE(?2, description), color = COALESCE(?3, color), version = COALESCE(?4, version), author = COALESCE(?5, author), updated_at = CURRENT_TIMESTAMP WHERE slug = ?6",
    )
    .bind(name)
    .bind(description)
    .bind(color)
    .bind(version)
    .bind(author)
    .bind(slug)
    .execute(pool)
    .await?;
    Ok(res.rows_affected() > 0)
}

pub async fn find_all_mcp(pool: &SqlitePool) -> sqlx::Result<Vec<ProjectMcpRow>> {
    sqlx::query_as::<_, ProjectMcpRow>("SELECT name, slug, root_path FROM projects")
        .fetch_all(pool)
        .await
}

#[derive(sqlx::FromRow, serde::Serialize)]
pub struct ProjectMcpRow {
    pub name: String,
    pub slug: String,
    pub root_path: String,
}

pub async fn resolve_id(pool: &SqlitePool, project_id_or_slug: &str) -> sqlx::Result<String> {
    let row: Option<(String,)> = sqlx::query_as(
        "SELECT id FROM projects WHERE id = ? OR slug = ? LIMIT 1",
    )
    .bind(project_id_or_slug)
    .bind(project_id_or_slug)
    .fetch_optional(pool)
    .await?;
    row.map(|(id,)| id).ok_or(sqlx::Error::RowNotFound)
}
