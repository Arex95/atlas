#[allow(dead_code)]
#[derive(sqlx::FromRow, Clone, Debug)]
pub struct AiSession {
    pub id: String,
    pub project_id: Option<String>,
    pub provider: String,
    pub model: String,
    pub status: String,
    pub pid: Option<i64>,
    pub pty_fd: Option<i64>,
    pub working_directory: String,
    pub prompt: Option<String>,
    pub mode: String,
    pub linked_task_id: Option<String>,
    pub started_at: String,
    pub stopped_at: Option<String>,
    pub last_activity_at: String,
    pub title: Option<String>,
    pub author: String,
    pub is_saved: i64,
    pub custom_name: Option<String>,
    pub custom_description: Option<String>,
    pub color: Option<String>,
    pub icon: Option<String>,
}

#[derive(sqlx::FromRow, Clone, Debug, serde::Serialize)]
pub struct ConversationTurn {
    pub id: String,
    pub session_id: String,
    pub role: String,
    pub content: String,
    pub created_at: String,
}
