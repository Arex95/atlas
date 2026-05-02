#[derive(sqlx::FromRow, Clone, Debug)]
pub struct Task {
    pub id: String,
    pub project_id: String,
    pub title: String,
    pub description: String,
    pub status: String,
    pub priority: String,
    pub due_date: Option<String>,
    pub assigned_to: Option<String>,
    pub tags: String,
    pub parent_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}
