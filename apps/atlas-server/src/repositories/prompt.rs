use crate::models::Prompt;
use sqlx::SqlitePool;

pub async fn find_all(
    pool: &SqlitePool,
    project_id: Option<&str>,
    session_id: Option<&str>,
) -> sqlx::Result<Vec<Prompt>> {
    match (project_id, session_id) {
        (Some(pid), None) => sqlx::query_as::<_, Prompt>(
            "SELECT * FROM prompts WHERE project_id = ? ORDER BY updated_at DESC",
        )
        .bind(pid)
        .fetch_all(pool)
        .await,
        (None, Some(sid)) => sqlx::query_as::<_, Prompt>(
            "SELECT * FROM prompts WHERE session_id = ? ORDER BY updated_at DESC",
        )
        .bind(sid)
        .fetch_all(pool)
        .await,
        (Some(pid), Some(sid)) => sqlx::query_as::<_, Prompt>(
            "SELECT * FROM prompts WHERE project_id = ? OR session_id = ? ORDER BY updated_at DESC",
        )
        .bind(pid)
        .bind(sid)
        .fetch_all(pool)
        .await,
        (None, None) => sqlx::query_as::<_, Prompt>(
            "SELECT * FROM prompts ORDER BY updated_at DESC LIMIT 100",
        )
        .fetch_all(pool)
        .await,
    }
}

pub async fn find_by_id(pool: &SqlitePool, id: &str) -> sqlx::Result<Option<Prompt>> {
    sqlx::query_as::<_, Prompt>("SELECT * FROM prompts WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await
}

pub async fn create(
    pool: &SqlitePool,
    id: &str,
    project_id: Option<&str>,
    session_id: Option<&str>,
    title: &str,
    content: &str,
    category: &str,
) -> sqlx::Result<Prompt> {
    sqlx::query(
        "INSERT INTO prompts (id, project_id, session_id, title, content, category) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(id)
    .bind(project_id)
    .bind(session_id)
    .bind(title)
    .bind(content)
    .bind(category)
    .execute(pool)
    .await?;

    sqlx::query_as::<_, Prompt>("SELECT * FROM prompts WHERE id = ?")
        .bind(id)
        .fetch_one(pool)
        .await
}

pub async fn update(
    pool: &SqlitePool,
    id: &str,
    title: &str,
    content: &str,
    category: &str,
) -> sqlx::Result<Prompt> {
    sqlx::query(
        "UPDATE prompts SET title = ?, content = ?, category = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
    )
    .bind(title)
    .bind(content)
    .bind(category)
    .bind(id)
    .execute(pool)
    .await?;

    sqlx::query_as::<_, Prompt>("SELECT * FROM prompts WHERE id = ?")
        .bind(id)
        .fetch_one(pool)
        .await
}

pub async fn delete(pool: &SqlitePool, id: &str) -> sqlx::Result<bool> {
    let res = sqlx::query("DELETE FROM prompts WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(res.rows_affected() > 0)
}
