use serde::Serialize;

pub mod document;
pub mod notification;
pub mod profile;
pub mod project;
pub mod prompt;
pub mod reminder;
pub mod session;
pub mod skill;
pub mod task;

#[derive(Serialize)]
pub struct ApiResponse<T: Serialize> {
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub count: Option<usize>,
}
