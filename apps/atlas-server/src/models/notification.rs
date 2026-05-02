#[derive(sqlx::FromRow, Clone, Debug)]
pub struct Notification {
    pub id: String,
    pub project_id: Option<String>,
    pub session_id: Option<String>,
    pub title: Option<String>,
    pub message: String,
    #[sqlx(rename = "type")]
    pub kind: String,
    pub status: String,
    pub created_at: String,
}
