#[derive(sqlx::FromRow, Clone, Debug)]
pub struct AgentSkill {
    pub id: String,
    pub project_id: Option<String>,
    pub name: String,
    pub description: String,
    pub script: String,
    pub usage_count: i64,
    pub created_at: String,
    pub updated_at: String,
}
