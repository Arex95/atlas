use thiserror::Error;

#[derive(Debug, Error)]
pub enum MessagingError {
    #[error("project must not be empty")]
    EmptyProject,

    #[error("from_session must not be empty")]
    EmptyFromSession,

    #[error("storage failure: {0}")]
    Storage(String),
}
