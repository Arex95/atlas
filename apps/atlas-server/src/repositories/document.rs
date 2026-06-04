use crate::models::ProjectDocument;
use sqlx::SqlitePool;

pub async fn find_all(
    pool: &SqlitePool,
    project_id: &str,
    kind: Option<&str>,
) -> sqlx::Result<Vec<ProjectDocument>> {
    if let Some(k) = kind {
        sqlx::query_as::<_, ProjectDocument>(
            "SELECT * FROM project_documents WHERE project_id = ? AND kind = ? ORDER BY updated_at DESC",
        )
        .bind(project_id)
        .bind(k)
        .fetch_all(pool)
        .await
    } else {
        sqlx::query_as::<_, ProjectDocument>(
            "SELECT * FROM project_documents WHERE project_id = ? ORDER BY updated_at DESC",
        )
        .bind(project_id)
        .fetch_all(pool)
        .await
    }
}

pub async fn find_by_id(pool: &SqlitePool, id: &str) -> sqlx::Result<Option<ProjectDocument>> {
    sqlx::query_as::<_, ProjectDocument>("SELECT * FROM project_documents WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await
}

pub async fn create(
    pool: &SqlitePool,
    id: &str,
    project_id: &str,
    title: &str,
    content: &str,
    kind: &str,
    tags: &str,
) -> sqlx::Result<ProjectDocument> {
    sqlx::query(
        "INSERT INTO project_documents (id, project_id, title, content, kind, tags) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(id)
    .bind(project_id)
    .bind(title)
    .bind(content)
    .bind(kind)
    .bind(tags)
    .execute(pool)
    .await?;

    sqlx::query_as::<_, ProjectDocument>("SELECT * FROM project_documents WHERE id = ?")
        .bind(id)
        .fetch_one(pool)
        .await
}

pub async fn update(
    pool: &SqlitePool,
    id: &str,
    title: &str,
    content: &str,
    tags: &str,
) -> sqlx::Result<ProjectDocument> {
    sqlx::query(
        "UPDATE project_documents SET title = ?, content = ?, tags = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
    )
    .bind(title)
    .bind(content)
    .bind(tags)
    .bind(id)
    .execute(pool)
    .await?;

    sqlx::query_as::<_, ProjectDocument>("SELECT * FROM project_documents WHERE id = ?")
        .bind(id)
        .fetch_one(pool)
        .await
}

pub async fn delete(pool: &SqlitePool, id: &str) -> sqlx::Result<bool> {
    let res = sqlx::query("DELETE FROM project_documents WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(res.rows_affected() > 0)
}

pub async fn find_for_mcp(
    pool: &SqlitePool,
    project_id: &str,
    kind: Option<&str>,
) -> sqlx::Result<Vec<DocMcpRow>> {
    if let Some(k) = kind {
        sqlx::query_as::<_, DocMcpRow>(
            "SELECT id, title, kind, updated_at FROM project_documents WHERE project_id = ? AND kind = ? ORDER BY updated_at DESC",
        )
        .bind(project_id)
        .bind(k)
        .fetch_all(pool)
        .await
    } else {
        sqlx::query_as::<_, DocMcpRow>(
            "SELECT id, title, kind, updated_at FROM project_documents WHERE project_id = ? ORDER BY updated_at DESC",
        )
        .bind(project_id)
        .fetch_all(pool)
        .await
    }
}

pub async fn find_full_by_id(pool: &SqlitePool, id: &str) -> sqlx::Result<Option<DocFullRow>> {
    sqlx::query_as::<_, DocFullRow>(
        "SELECT id, title, content, kind, links FROM project_documents WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}

/// Returns all documents whose IDs appear in the given JSON array string, e.g. '["id1","id2"]'.
pub async fn find_linked(pool: &SqlitePool, links_json: &str) -> sqlx::Result<Vec<DocFullRow>> {
    sqlx::query_as::<_, DocFullRow>(
        "SELECT id, title, content, kind, links \
         FROM project_documents \
         WHERE id IN (SELECT value FROM json_each(?))",
    )
    .bind(links_json)
    .fetch_all(pool)
    .await
}

pub async fn write_content(
    pool: &SqlitePool,
    id: &str,
    content: &str,
    title: Option<&str>,
    links: Option<&str>,
) -> sqlx::Result<bool> {
    let res = sqlx::query(
        "UPDATE project_documents \
         SET content = ?, \
             title = COALESCE(?, title), \
             links = COALESCE(?, links), \
             updated_at = CURRENT_TIMESTAMP \
         WHERE id = ?",
    )
    .bind(content)
    .bind(title)
    .bind(links)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(res.rows_affected() > 0)
}

pub async fn create_simple(
    pool: &SqlitePool,
    id: &str,
    project_id: &str,
    title: &str,
    content: &str,
    kind: &str,
    links: &str,
) -> sqlx::Result<bool> {
    let res = sqlx::query(
        "INSERT INTO project_documents (id, project_id, title, content, kind, tags, links) \
         VALUES (?, ?, ?, ?, ?, '[]', ?)",
    )
    .bind(id)
    .bind(project_id)
    .bind(title)
    .bind(content)
    .bind(kind)
    .bind(links)
    .execute(pool)
    .await?;
    Ok(res.rows_affected() > 0)
}

#[derive(sqlx::FromRow, serde::Serialize)]
pub struct DocMcpRow {
    pub id: String,
    pub title: String,
    pub kind: String,
    pub updated_at: String,
}

#[derive(sqlx::FromRow, serde::Serialize)]
pub struct DocFullRow {
    pub id: String,
    pub title: String,
    pub content: String,
    pub kind: String,
    pub links: String,
}
