use thiserror::Error;

#[derive(Debug, Error)]
pub enum SessionsError {
    #[error("project must not be empty")]
    EmptyProject,

    #[error("remote_url must not be empty")]
    EmptyRemoteUrl,

    #[error("owner_id must not be empty")]
    EmptyOwnerId,

    #[error("a session with this id is owned by someone else")]
    OwnerMismatch,

    #[error("unknown status {0:?}; expected \"active\" or \"archived\"")]
    InvalidStatus(String),

    #[error("session not found")]
    NotFound,

    #[error("storage failure: {0}")]
    Storage(String),
}
