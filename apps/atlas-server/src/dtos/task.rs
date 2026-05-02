use garde::Validate;
use serde::{Deserialize, Serialize};

use crate::models::Task;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskResponse {
    pub id: String,
    pub project_id: String,
    pub title: String,
    pub description: String,
    pub status: String,
    pub priority: String,
    pub due_date: Option<String>,
    pub assigned_to: Option<String>,
    pub tags: Vec<String>,
    pub parent_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl From<Task> for TaskResponse {
    fn from(t: Task) -> Self {
        let tags: Vec<String> = serde_json::from_str(&t.tags).unwrap_or_default();
        Self {
            id: t.id,
            project_id: t.project_id,
            title: t.title,
            description: t.description,
            status: t.status,
            priority: t.priority,
            due_date: t.due_date,
            assigned_to: t.assigned_to,
            tags,
            parent_id: t.parent_id,
            created_at: t.created_at,
            updated_at: t.updated_at,
        }
    }
}

#[derive(Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct CreateTaskRequest {
    #[garde(length(min = 1, max = 300))]
    pub title: String,
    #[garde(skip)]
    pub description: Option<String>,
    #[garde(skip)]
    pub project_id: String,
    #[garde(skip)]
    pub status: Option<String>,
    #[garde(skip)]
    pub priority: Option<String>,
    #[garde(skip)]
    pub due_date: Option<String>,
    #[garde(skip)]
    pub assigned_to: Option<String>,
    #[garde(skip)]
    pub tags: Option<Vec<String>>,
    #[garde(skip)]
    pub parent_id: Option<String>,
}

#[derive(Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct UpdateTaskRequest {
    #[garde(inner(length(min = 1, max = 300)))]
    pub title: Option<String>,
    #[garde(skip)]
    pub description: Option<String>,
    #[garde(skip)]
    pub status: Option<String>,
    #[garde(skip)]
    pub priority: Option<String>,
    #[garde(skip)]
    pub due_date: Option<String>,
    #[garde(skip)]
    pub assigned_to: Option<String>,
    #[garde(skip)]
    pub tags: Option<Vec<String>>,
    #[garde(skip)]
    pub parent_id: Option<String>,
}
