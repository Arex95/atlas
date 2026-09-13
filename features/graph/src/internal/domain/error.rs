use thiserror::Error;

#[derive(Debug, Error)]
pub enum GraphError {
    #[error("project must not be empty")]
    EmptyProject,

    #[error("project root {0:?} is not a directory")]
    RootNotADirectory(String),

    #[error("could not read the project tree: {0}")]
    Walk(String),

    #[error("no node found for that identity")]
    NotFound,

    #[error("storage failure: {0}")]
    Storage(String),
}
