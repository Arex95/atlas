#[derive(sqlx::FromRow, Clone, Debug)]
pub struct Prompt {
    pub id: String,
    pub project_id: Option<String>,
    pub session_id: Option<String>,
    pub title: String,
    pub content: String,
    pub category: String,
    pub created_at: String,
    pub updated_at: String,
}
