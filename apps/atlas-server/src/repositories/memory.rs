use sqlx::SqlitePool;

pub async fn upsert(
    pool: &SqlitePool,
    id: &str,
    project_id: &str,
    key: &str,
    value: &str,
) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO agent_memory (id, project_id, key, value)
         VALUES (?, ?, ?, ?)
         ON CONFLICT(project_id, key) DO UPDATE SET
           value = excluded.value,
           updated_at = CURRENT_TIMESTAMP",
    )
    .bind(id)
    .bind(project_id)
    .bind(key)
    .bind(value)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn find_value(pool: &SqlitePool, project_id: &str, key: &str) -> sqlx::Result<Option<String>> {
    sqlx::query_scalar::<_, String>(
        "SELECT value FROM agent_memory WHERE project_id = ? AND key = ?",
    )
    .bind(project_id)
    .bind(key)
    .fetch_optional(pool)
    .await
}

pub async fn find_all(pool: &SqlitePool, project_id: &str) -> sqlx::Result<Vec<MemoryRow>> {
    sqlx::query_as::<_, MemoryRow>(
        "SELECT key, value, updated_at FROM agent_memory WHERE project_id = ? ORDER BY updated_at DESC",
    )
    .bind(project_id)
    .fetch_all(pool)
    .await
}

pub async fn delete(pool: &SqlitePool, project_id: &str, key: &str) -> sqlx::Result<bool> {
    let res = sqlx::query("DELETE FROM agent_memory WHERE project_id = ? AND key = ?")
        .bind(project_id)
        .bind(key)
        .execute(pool)
        .await?;
    Ok(res.rows_affected() > 0)
}

#[derive(sqlx::FromRow, serde::Serialize)]
pub struct MemoryRow {
    pub key: String,
    pub value: String,
    pub updated_at: String,
}
