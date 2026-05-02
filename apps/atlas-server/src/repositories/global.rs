use sqlx::SqlitePool;

pub async fn list_memory(pool: &SqlitePool) -> sqlx::Result<Vec<GlobalMemoryRow>> {
    sqlx::query_as::<_, GlobalMemoryRow>("SELECT * FROM global_memory ORDER BY key ASC")
        .fetch_all(pool)
        .await
}

pub async fn upsert_memory(
    pool: &SqlitePool,
    id: &str,
    key: &str,
    value: &str,
    description: &str,
) -> sqlx::Result<GlobalMemoryRow> {
    sqlx::query(
        "INSERT INTO global_memory (id, key, value, description)
         VALUES (?, ?, ?, ?)
         ON CONFLICT(key) DO UPDATE SET
             value       = excluded.value,
             description = excluded.description,
             updated_at  = CURRENT_TIMESTAMP",
    )
    .bind(id)
    .bind(key)
    .bind(value)
    .bind(description)
    .execute(pool)
    .await?;

    sqlx::query_as::<_, GlobalMemoryRow>("SELECT * FROM global_memory WHERE key = ?")
        .bind(key)
        .fetch_one(pool)
        .await
}

pub async fn delete_memory(pool: &SqlitePool, key: &str) -> sqlx::Result<bool> {
    let res = sqlx::query("DELETE FROM global_memory WHERE key = ?")
        .bind(key)
        .execute(pool)
        .await?;
    Ok(res.rows_affected() > 0)
}

pub async fn list_skills(pool: &SqlitePool) -> sqlx::Result<Vec<GlobalSkillRow>> {
    sqlx::query_as::<_, GlobalSkillRow>(
        "SELECT * FROM global_skills ORDER BY usage_count DESC, name ASC",
    )
    .fetch_all(pool)
    .await
}

pub async fn create_skill(
    pool: &SqlitePool,
    id: &str,
    name: &str,
    description: &str,
    trigger: Option<&str>,
    script: &str,
) -> sqlx::Result<GlobalSkillRow> {
    sqlx::query(
        "INSERT INTO global_skills (id, name, description, trigger, script) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(id)
    .bind(name)
    .bind(description)
    .bind(trigger)
    .bind(script)
    .execute(pool)
    .await?;

    sqlx::query_as::<_, GlobalSkillRow>("SELECT * FROM global_skills WHERE id = ?")
        .bind(id)
        .fetch_one(pool)
        .await
}

pub async fn update_skill(
    pool: &SqlitePool,
    id: &str,
    name: Option<&str>,
    description: Option<&str>,
    trigger: Option<&str>,
    script: Option<&str>,
) -> sqlx::Result<Option<GlobalSkillRow>> {
    let res = sqlx::query(
        "UPDATE global_skills
         SET name        = COALESCE(?1, name),
             description = COALESCE(?2, description),
             trigger     = COALESCE(?3, trigger),
             script      = COALESCE(?4, script),
             updated_at  = CURRENT_TIMESTAMP
         WHERE id = ?5",
    )
    .bind(name)
    .bind(description)
    .bind(trigger)
    .bind(script)
    .bind(id)
    .execute(pool)
    .await?;

    if res.rows_affected() == 0 {
        return Ok(None);
    }

    let row = sqlx::query_as::<_, GlobalSkillRow>("SELECT * FROM global_skills WHERE id = ?")
        .bind(id)
        .fetch_one(pool)
        .await?;
    Ok(Some(row))
}

pub async fn delete_skill(pool: &SqlitePool, id: &str) -> sqlx::Result<bool> {
    let res = sqlx::query("DELETE FROM global_skills WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(res.rows_affected() > 0)
}

pub async fn list_prompts(pool: &SqlitePool) -> sqlx::Result<Vec<GlobalPromptRow>> {
    sqlx::query_as::<_, GlobalPromptRow>("SELECT * FROM global_prompts ORDER BY updated_at DESC")
        .fetch_all(pool)
        .await
}

pub async fn create_prompt(
    pool: &SqlitePool,
    id: &str,
    title: &str,
    content: &str,
) -> sqlx::Result<GlobalPromptRow> {
    sqlx::query("INSERT INTO global_prompts (id, title, content) VALUES (?, ?, ?)")
        .bind(id)
        .bind(title)
        .bind(content)
        .execute(pool)
        .await?;

    sqlx::query_as::<_, GlobalPromptRow>("SELECT * FROM global_prompts WHERE id = ?")
        .bind(id)
        .fetch_one(pool)
        .await
}

pub async fn update_prompt(
    pool: &SqlitePool,
    id: &str,
    title: Option<&str>,
    content: Option<&str>,
) -> sqlx::Result<Option<GlobalPromptRow>> {
    let res = sqlx::query(
        "UPDATE global_prompts
         SET title      = COALESCE(?1, title),
             content    = COALESCE(?2, content),
             updated_at = CURRENT_TIMESTAMP
         WHERE id = ?3",
    )
    .bind(title)
    .bind(content)
    .bind(id)
    .execute(pool)
    .await?;

    if res.rows_affected() == 0 {
        return Ok(None);
    }

    let row = sqlx::query_as::<_, GlobalPromptRow>("SELECT * FROM global_prompts WHERE id = ?")
        .bind(id)
        .fetch_one(pool)
        .await?;
    Ok(Some(row))
}

pub async fn delete_prompt(pool: &SqlitePool, id: &str) -> sqlx::Result<bool> {
    let res = sqlx::query("DELETE FROM global_prompts WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(res.rows_affected() > 0)
}

pub async fn list_memory_mcp(pool: &SqlitePool) -> sqlx::Result<Vec<GlobalMemoryMcpRow>> {
    sqlx::query_as::<_, GlobalMemoryMcpRow>(
        "SELECT key, value, description FROM global_memory ORDER BY key ASC",
    )
    .fetch_all(pool)
    .await
}

pub async fn list_skills_mcp(pool: &SqlitePool) -> sqlx::Result<Vec<GlobalSkillMcpRow>> {
    sqlx::query_as::<_, GlobalSkillMcpRow>(
        "SELECT id, name, description, trigger FROM global_skills ORDER BY name ASC",
    )
    .fetch_all(pool)
    .await
}

pub async fn list_prompts_mcp(pool: &SqlitePool) -> sqlx::Result<Vec<GlobalPromptMcpRow>> {
    sqlx::query_as::<_, GlobalPromptMcpRow>(
        "SELECT id, title, content FROM global_prompts ORDER BY title ASC",
    )
    .fetch_all(pool)
    .await
}

pub async fn list_skills_full_mcp(pool: &SqlitePool) -> sqlx::Result<Vec<GlobalSkillFullMcpRow>> {
    sqlx::query_as::<_, GlobalSkillFullMcpRow>(
        "SELECT id, name, description, trigger, script FROM global_skills ORDER BY name ASC",
    )
    .fetch_all(pool)
    .await
}

#[derive(sqlx::FromRow, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GlobalMemoryRow {
    pub id: String,
    pub key: String,
    pub value: String,
    pub description: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(sqlx::FromRow, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GlobalSkillRow {
    pub id: String,
    pub name: String,
    pub description: String,
    pub trigger: Option<String>,
    pub script: String,
    pub usage_count: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(sqlx::FromRow, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GlobalPromptRow {
    pub id: String,
    pub title: String,
    pub content: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(sqlx::FromRow, serde::Serialize)]
pub struct GlobalMemoryMcpRow {
    pub key: String,
    pub value: String,
    pub description: String,
}

#[derive(sqlx::FromRow, serde::Serialize)]
pub struct GlobalSkillMcpRow {
    pub id: String,
    pub name: String,
    pub description: String,
    pub trigger: Option<String>,
}

#[derive(sqlx::FromRow, serde::Serialize)]
pub struct GlobalPromptMcpRow {
    pub id: String,
    pub title: String,
    pub content: String,
}

#[derive(sqlx::FromRow, serde::Serialize)]
pub struct GlobalSkillFullMcpRow {
    pub id: String,
    pub name: String,
    pub description: String,
    pub trigger: Option<String>,
    pub script: String,
}
