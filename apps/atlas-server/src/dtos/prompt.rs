use garde::Validate;
use serde::{Deserialize, Serialize};

use crate::models::Prompt;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptResponse {
    pub id: String,
    pub project_id: Option<String>,
    pub session_id: Option<String>,
    pub title: String,
    pub content: String,
    pub category: String,
    pub created_at: String,
    pub updated_at: String,
}

impl From<Prompt> for PromptResponse {
    fn from(p: Prompt) -> Self {
        Self {
            id: p.id,
            project_id: p.project_id,
            session_id: p.session_id,
            title: p.title,
            content: p.content,
            category: p.category,
            created_at: p.created_at,
            updated_at: p.updated_at,
        }
    }
}

#[derive(Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct CreatePromptRequest {
    #[garde(length(min = 1, max = 255))]
    pub title: String,
    #[garde(length(min = 1))]
    pub content: String,
    #[garde(length(min = 1, max = 50))]
    #[serde(default = "default_category")]
    pub category: String,
    #[garde(skip)]
    pub project_id: Option<String>,
    #[garde(skip)]
    pub session_id: Option<String>,
}

fn default_category() -> String {
    "general".to_string()
}

#[derive(Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct UpdatePromptRequest {
    #[garde(length(min = 1, max = 255))]
    pub title: Option<String>,
    #[garde(skip)]
    pub content: Option<String>,
    #[garde(length(min = 1, max = 50))]
    pub category: Option<String>,
}
