use crate::models::AgentSkill;
use sqlx::SqlitePool;

pub async fn find_all(pool: &SqlitePool, project_id: &str) -> sqlx::Result<Vec<AgentSkill>> {
    sqlx::query_as::<_, AgentSkill>(
        "SELECT * FROM agent_skills WHERE project_id = ? ORDER BY usage_count DESC, updated_at DESC",
    )
    .bind(project_id)
    .fetch_all(pool)
    .await
}

pub async fn find_by_id(pool: &SqlitePool, id: &str) -> sqlx::Result<Option<AgentSkill>> {
    sqlx::query_as::<_, AgentSkill>("SELECT * FROM agent_skills WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await
}

pub async fn create(
    pool: &SqlitePool,
    id: &str,
    project_id: &str,
    name: &str,
    description: &str,
    script: &str,
) -> sqlx::Result<AgentSkill> {
    sqlx::query(
        "INSERT INTO agent_skills (id, project_id, name, description, script) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(id)
    .bind(project_id)
    .bind(name)
    .bind(description)
    .bind(script)
    .execute(pool)
    .await?;

    sqlx::query_as::<_, AgentSkill>("SELECT * FROM agent_skills WHERE id = ?")
        .bind(id)
        .fetch_one(pool)
        .await
}

pub async fn update(
    pool: &SqlitePool,
    id: &str,
    name: &str,
    description: &str,
    script: &str,
) -> sqlx::Result<AgentSkill> {
    sqlx::query(
        "UPDATE agent_skills SET name = ?, description = ?, script = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
    )
    .bind(name)
    .bind(description)
    .bind(script)
    .bind(id)
    .execute(pool)
    .await?;

    sqlx::query_as::<_, AgentSkill>("SELECT * FROM agent_skills WHERE id = ?")
        .bind(id)
        .fetch_one(pool)
        .await
}

pub async fn delete(pool: &SqlitePool, id: &str) -> sqlx::Result<bool> {
    let res = sqlx::query("DELETE FROM agent_skills WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(res.rows_affected() > 0)
}

pub async fn find_for_mcp(pool: &SqlitePool, project_id: &str) -> sqlx::Result<Vec<SkillMcpRow>> {
    sqlx::query_as::<_, SkillMcpRow>(
        "SELECT id, name, description, usage_count FROM agent_skills WHERE project_id = ? ORDER BY usage_count DESC",
    )
    .bind(project_id)
    .fetch_all(pool)
    .await
}

pub async fn find_with_global(pool: &SqlitePool, skill_id: &str) -> sqlx::Result<Option<SkillRunRow>> {
    sqlx::query_as::<_, SkillRunRow>(
        "SELECT script, name, project_id, 0 AS is_global FROM agent_skills WHERE id = ?
         UNION ALL
         SELECT script, name, NULL AS project_id, 1 AS is_global FROM global_skills WHERE id = ?
         LIMIT 1",
    )
    .bind(skill_id)
    .bind(skill_id)
    .fetch_optional(pool)
    .await
}

pub async fn increment_usage(pool: &SqlitePool, skill_id: &str, is_global: bool) -> sqlx::Result<()> {
    let table = if is_global { "global_skills" } else { "agent_skills" };
    sqlx::query(
        &format!("UPDATE {table} SET usage_count = usage_count + 1, updated_at = CURRENT_TIMESTAMP WHERE id = ?"),
    )
    .bind(skill_id)
    .execute(pool)
    .await?;
    Ok(())
}

#[derive(sqlx::FromRow, serde::Serialize)]
pub struct SkillMcpRow {
    pub id: String,
    pub name: String,
    pub description: String,
    pub usage_count: i64,
}

#[derive(sqlx::FromRow)]
pub struct SkillRunRow {
    pub script: String,
    pub name: String,
    pub project_id: Option<String>,
    pub is_global: bool,
}
