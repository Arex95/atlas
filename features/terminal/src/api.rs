//! Public surface of the `atlas-terminal` crate.

pub use crate::internal::domain::{RestoreOutcome, SpawnedTerminal, TerminalError, TerminalOutput};
pub use crate::internal::infrastructure::PtyPool;
