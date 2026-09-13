//! The terminal vocabulary: real PTYs, not transcripts.

mod error;
mod model;

pub use error::TerminalError;
pub use model::{RestoreOutcome, SpawnedTerminal, TerminalOutput};
