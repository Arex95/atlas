use thiserror::Error;

#[derive(Debug, Error)]
pub enum MemoryError {
    #[error("key must not be empty")]
    EmptyKey,

    #[error("project must not be empty")]
    EmptyProject,

    #[error("owner_id must not be empty")]
    EmptyOwner,

    #[error("no memory found for that key")]
    NotFound,

    #[error("stored value was not valid json: {0}")]
    Corrupt(String),

    #[error("storage failure: {0}")]
    Storage(String),
}
