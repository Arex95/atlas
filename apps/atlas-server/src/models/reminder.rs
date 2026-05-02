#[derive(sqlx::FromRow, Clone, Debug)]
pub struct Reminder {
    pub id: String,
    pub project_id: Option<String>,
    pub title: String,
    pub description: String,
    pub due_at: String,
    #[sqlx(rename = "kind")]
    pub kind: String,
    pub status: String,
    pub last_notified_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}
