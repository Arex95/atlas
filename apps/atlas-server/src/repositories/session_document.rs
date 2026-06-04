use sqlx::SqlitePool;

#[derive(sqlx::FromRow, serde::Serialize)]
pub struct SessionDocumentRow {
    pub id: String,
    pub title: String,
    pub content: String,
    pub kind: String,
    pub links: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(sqlx::FromRow, serde::Serialize)]
pub struct SessionDocFullRow {
    pub id: String,
    pub title: String,
    pub content: String,
    pub kind: String,
    pub links: String,
}

pub async fn create(
    pool: &SqlitePool,
    id: &str,
    session_id: &str,
    title: &str,
    content: &str,
    kind: &str,
    links: &str,
) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO session_documents (id, session_id, title, content, kind, links)
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(id)
    .bind(session_id)
    .bind(title)
    .bind(content)
    .bind(kind)
    .bind(links)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn find_all(
    pool: &SqlitePool,
    session_id: &str,
    kind: Option<&str>,
) -> sqlx::Result<Vec<SessionDocumentRow>> {
    if let Some(k) = kind {
        sqlx::query_as::<_, SessionDocumentRow>(
            "SELECT id, title, content, kind, links, created_at, updated_at
             FROM session_documents WHERE session_id = ? AND kind = ? ORDER BY created_at DESC",
        )
        .bind(session_id)
        .bind(k)
        .fetch_all(pool)
        .await
    } else {
        sqlx::query_as::<_, SessionDocumentRow>(
            "SELECT id, title, content, kind, links, created_at, updated_at
             FROM session_documents WHERE session_id = ? ORDER BY created_at DESC",
        )
        .bind(session_id)
        .fetch_all(pool)
        .await
    }
}

pub async fn find_full_by_id(
    pool: &SqlitePool,
    id: &str,
) -> sqlx::Result<Option<SessionDocFullRow>> {
    sqlx::query_as::<_, SessionDocFullRow>(
        "SELECT id, title, content, kind, links FROM session_documents WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}

pub async fn find_linked(
    pool: &SqlitePool,
    links_json: &str,
) -> sqlx::Result<Vec<SessionDocFullRow>> {
    sqlx::query_as::<_, SessionDocFullRow>(
        "SELECT id, title, content, kind, links \
         FROM session_documents \
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
        "UPDATE session_documents \
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

pub async fn delete(pool: &SqlitePool, id: &str) -> sqlx::Result<bool> {
    let res = sqlx::query("DELETE FROM session_documents WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(res.rows_affected() > 0)
}
