use garde::Validate;
use serde::{Deserialize, Serialize};

use crate::models::ProjectDocument;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentResponse {
    pub id: String,
    pub project_id: String,
    pub title: String,
    pub content: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub tags: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl From<ProjectDocument> for DocumentResponse {
    fn from(d: ProjectDocument) -> Self {
        let tags: Vec<String> =
            serde_json::from_str(&d.tags).unwrap_or_default();
        Self {
            id: d.id,
            project_id: d.project_id,
            title: d.title,
            content: d.content,
            kind: d.kind,
            tags,
            created_at: d.created_at,
            updated_at: d.updated_at,
        }
    }
}

#[derive(Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct CreateDocumentRequest {
    #[garde(length(min = 1, max = 255))]
    pub title: String,
    #[garde(skip)]
    pub content: Option<String>,
    #[garde(inner(length(min = 1, max = 50)))]
    pub tags: Option<Vec<String>>,
    #[garde(length(min = 1, max = 50))]
    #[serde(rename = "type", default = "default_kind")]
    pub kind: String,
    #[garde(length(min = 1))]
    pub project_id: String,
}

fn default_kind() -> String {
    "document".to_string()
}

#[derive(Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct UpdateDocumentRequest {
    #[garde(length(min = 1, max = 255))]
    pub title: Option<String>,
    #[garde(skip)]
    pub content: Option<String>,
    #[garde(inner(length(min = 1, max = 50)))]
    pub tags: Option<Vec<String>>,
}
