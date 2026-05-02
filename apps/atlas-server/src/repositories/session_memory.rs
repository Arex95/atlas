use sqlx::SqlitePool;

#[derive(sqlx::FromRow, serde::Serialize)]
pub struct SessionMemoryRow {
    pub key: String,
    pub value: String,
    pub updated_at: String,
}

pub async fn upsert(pool: &SqlitePool, id: &str, session_id: &str, key: &str, value: &str) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO session_memory (id, session_id, key, value)
         VALUES (?, ?, ?, ?)
         ON CONFLICT(session_id, key) DO UPDATE SET
           value = excluded.value,
           updated_at = STRFTIME('%Y-%m-%dT%H:%M:%fZ', 'now')",
    )
    .bind(id)
    .bind(session_id)
    .bind(key)
    .bind(value)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn find_value(pool: &SqlitePool, session_id: &str, key: &str) -> sqlx::Result<Option<String>> {
    sqlx::query_scalar::<_, String>(
        "SELECT value FROM session_memory WHERE session_id = ? AND key = ?",
    )
    .bind(session_id)
    .bind(key)
    .fetch_optional(pool)
    .await
}

#[allow(dead_code)]
pub async fn find_all(pool: &SqlitePool, session_id: &str) -> sqlx::Result<Vec<SessionMemoryRow>> {
    sqlx::query_as::<_, SessionMemoryRow>(
        "SELECT key, value, updated_at FROM session_memory WHERE session_id = ? ORDER BY updated_at DESC",
    )
    .bind(session_id)
    .fetch_all(pool)
    .await
}

#[allow(dead_code)]
pub async fn delete(pool: &SqlitePool, session_id: &str, key: &str) -> sqlx::Result<bool> {
    let res = sqlx::query("DELETE FROM session_memory WHERE session_id = ? AND key = ?")
        .bind(session_id)
        .bind(key)
        .execute(pool)
        .await?;
    Ok(res.rows_affected() > 0)
}
