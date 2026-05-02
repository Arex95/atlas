use crate::models::{AiSession, ConversationTurn};
use garde::Validate;
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationTurnResponse {
    pub id: String,
    pub session_id: String,
    pub role: String,
    pub content: String,
    pub created_at: String,
}

impl From<ConversationTurn> for ConversationTurnResponse {
    fn from(m: ConversationTurn) -> Self {
        Self {
            id: m.id,
            session_id: m.session_id,
            role: m.role,
            content: m.content,
            created_at: m.created_at,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiSessionResponse {
    pub id: String,
    pub project_id: Option<String>,
    pub provider: String,
    pub model: String,
    pub status: String,
    pub working_directory: String,
    pub mode: String,
    pub started_at: String,
    pub last_activity_at: String,
    pub title: Option<String>,
    pub author: String,
    pub git: Option<crate::git::GitInfo>,
    pub history: Vec<ConversationTurnResponse>,
    pub is_saved: bool,
    pub custom_name: Option<String>,
    pub custom_description: Option<String>,
    pub color: Option<String>,
    pub icon: Option<String>,
    pub linked_task_id: Option<String>,
}

impl From<AiSession> for AiSessionResponse {
    fn from(s: AiSession) -> Self {
        Self {
            id: s.id,
            project_id: s.project_id,
            provider: s.provider,
            model: s.model,
            status: s.status,
            working_directory: s.working_directory,
            mode: s.mode,
            started_at: s.started_at,
            last_activity_at: s.last_activity_at,
            title: s.title,
            author: s.author,
            git: None,
            history: vec![],
            is_saved: s.is_saved != 0,
            custom_name: s.custom_name,
            custom_description: s.custom_description,
            color: s.color,
            icon: s.icon,
            linked_task_id: s.linked_task_id,
        }
    }
}

impl AiSessionResponse {
    pub fn with_git(mut self) -> Self {
        self.git = crate::git::get_git_info(&self.working_directory);
        self
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSessionRequest {
    pub provider: String,
    pub model: String,
    pub mode: String,
    pub working_directory: String,
    pub title: String,
}

#[derive(Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct SaveSessionRequest {
    #[garde(length(min = 1, max = 200))]
    pub custom_name: String,
    #[garde(inner(length(max = 2000)))]
    pub custom_description: Option<String>,
    #[garde(skip)]
    pub color: Option<String>,
    #[garde(skip)]
    pub icon: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateMessageRequest {
    pub role: String,
    pub content: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalResize {
    pub session_id: String,
    pub rows: u16,
    pub cols: u16,
}

#[derive(Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct OrchestratorMessageResponse {
    pub id: String,
    pub from_id: String,
    pub content: String,
    pub timestamp: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSessionRequest {
    pub custom_name: Option<String>,
    pub custom_description: Option<String>,
    pub color: Option<String>,
    pub icon: Option<String>,
    pub linked_task_id: Option<String>,
}
