#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum NotesError {
    #[error("a note's name must not be empty")]
    EmptyName,

    #[error("a note's name must be at most {0} characters")]
    NameTooLong(usize),

    #[error("owner_id must not be empty")]
    EmptyOwnerId,

    #[error("no note by that name")]
    NotFound,

    #[error("storage failure: {0}")]
    Storage(String),
}
