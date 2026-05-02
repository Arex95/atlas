use garde::Validate;
use serde::{Deserialize, Serialize};

use crate::models::AgentSkill;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillResponse {
    pub id: String,
    pub project_id: Option<String>,
    pub name: String,
    pub description: String,
    pub script: String,
    pub usage_count: i64,
    pub created_at: String,
    pub updated_at: String,
}

impl From<AgentSkill> for SkillResponse {
    fn from(s: AgentSkill) -> Self {
        Self {
            id: s.id,
            project_id: s.project_id,
            name: s.name,
            description: s.description,
            script: s.script,
            usage_count: s.usage_count,
            created_at: s.created_at,
            updated_at: s.updated_at,
        }
    }
}

#[derive(Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct CreateSkillRequest {
    #[garde(length(min = 1, max = 100))]
    pub name: String,
    #[garde(length(min = 1, max = 500))]
    pub description: String,
    #[garde(length(min = 1))]
    pub script: String,
    #[garde(length(min = 1))]
    pub project_id: String,
}

#[derive(Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSkillRequest {
    #[garde(length(min = 1, max = 100))]
    pub name: Option<String>,
    #[garde(length(min = 1, max = 500))]
    pub description: Option<String>,
    #[garde(skip)]
    pub script: Option<String>,
}
