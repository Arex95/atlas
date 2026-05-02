use garde::Validate;
use serde::{Deserialize, Serialize};

use crate::models::Reminder;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReminderResponse {
    pub id: String,
    pub project_id: Option<String>,
    pub title: String,
    pub description: String,
    pub due_at: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub status: String,
    pub last_notified_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl From<Reminder> for ReminderResponse {
    fn from(r: Reminder) -> Self {
        Self {
            id: r.id,
            project_id: r.project_id,
            title: r.title,
            description: r.description,
            due_at: r.due_at,
            kind: r.kind,
            status: r.status,
            last_notified_at: r.last_notified_at,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

#[derive(Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct CreateReminderRequest {
    #[garde(length(min = 1, max = 255))]
    pub title: String,
    #[garde(skip)]
    pub description: Option<String>,
    #[garde(length(min = 1))]
    pub due_at: String,
    #[garde(length(min = 1, max = 50))]
    #[serde(rename = "type", default = "default_kind")]
    pub kind: String,
    #[garde(skip)]
    pub project_id: Option<String>,
}

fn default_kind() -> String {
    "reminder".to_string()
}

#[derive(Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct UpdateReminderRequest {
    #[garde(length(min = 1, max = 255))]
    pub title: Option<String>,
    #[garde(skip)]
    pub description: Option<String>,
    #[garde(length(min = 1))]
    pub due_at: Option<String>,
    #[garde(length(min = 1, max = 20))]
    pub status: Option<String>,
}
