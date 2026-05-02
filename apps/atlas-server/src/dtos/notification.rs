use garde::Validate;
use serde::{Deserialize, Serialize};

use crate::models::Notification;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NotificationResponse {
    pub id: String,
    pub project_id: Option<String>,
    pub session_id: Option<String>,
    pub title: Option<String>,
    pub message: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub status: String,
    pub created_at: String,
}

impl From<Notification> for NotificationResponse {
    fn from(n: Notification) -> Self {
        Self {
            id: n.id,
            project_id: n.project_id,
            session_id: n.session_id,
            title: n.title,
            message: n.message,
            kind: n.kind,
            status: n.status,
            created_at: n.created_at,
        }
    }
}

#[derive(Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct CreateNotificationRequest {
    #[garde(skip)]
    pub project_id: Option<String>,
    #[garde(skip)]
    pub session_id: Option<String>,
    #[garde(length(min = 1, max = 200))]
    pub title: Option<String>,
    #[garde(length(min = 1, max = 2000))]
    pub message: String,
    #[garde(length(min = 1, max = 20))]
    #[serde(rename = "type", default = "default_kind")]
    pub kind: String,
}

fn default_kind() -> String {
    "info".to_string()
}
