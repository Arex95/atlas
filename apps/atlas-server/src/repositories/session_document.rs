use sqlx::SqlitePool;

#[derive(sqlx::FromRow, serde::Serialize)]
pub struct SessionDocumentRow {
    pub id: String,
    pub title: String,
    pub content: String,
    pub kind: String,
    pub created_at: String,
    pub updated_at: String,
}

pub async fn create(
    pool: &SqlitePool,
    id: &str,
    session_id: &str,
    title: &str,
    content: &str,
    kind: &str,
) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO session_documents (id, session_id, title, content, kind)
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(id)
    .bind(session_id)
    .bind(title)
    .bind(content)
    .bind(kind)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn find_all(pool: &SqlitePool, session_id: &str, kind: Option<&str>) -> sqlx::Result<Vec<SessionDocumentRow>> {
    if let Some(k) = kind {
        sqlx::query_as::<_, SessionDocumentRow>(
            "SELECT id, title, content, kind, created_at, updated_at
             FROM session_documents WHERE session_id = ? AND kind = ? ORDER BY created_at DESC",
        )
        .bind(session_id)
        .bind(k)
        .fetch_all(pool)
        .await
    } else {
        sqlx::query_as::<_, SessionDocumentRow>(
            "SELECT id, title, content, kind, created_at, updated_at
             FROM session_documents WHERE session_id = ? ORDER BY created_at DESC",
        )
        .bind(session_id)
        .fetch_all(pool)
        .await
    }
}

pub async fn write_content(pool: &SqlitePool, id: &str, content: &str, title: Option<&str>) -> sqlx::Result<bool> {
    let res = if let Some(t) = title {
        sqlx::query(
            "UPDATE session_documents SET content = ?, title = ?,
             updated_at = STRFTIME('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?",
        )
        .bind(content)
        .bind(t)
        .bind(id)
        .execute(pool)
        .await?
    } else {
        sqlx::query(
            "UPDATE session_documents SET content = ?,
             updated_at = STRFTIME('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?",
        )
        .bind(content)
        .bind(id)
        .execute(pool)
        .await?
    };
    Ok(res.rows_affected() > 0)
}

pub async fn delete(pool: &SqlitePool, id: &str) -> sqlx::Result<bool> {
    let res = sqlx::query("DELETE FROM session_documents WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(res.rows_affected() > 0)
}
