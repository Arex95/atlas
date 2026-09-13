//! The notes vocabulary.

mod error;
mod model;

pub use error::NotesError;
pub use model::Note;

/// Longest a note's name may be.
///
/// A name is an address a human types, not a document. Something
/// longer is a body that ended up in the wrong field, and refusing it
/// beats storing a row nobody can address again.
pub const MAX_NAME_LEN: usize = 200;
