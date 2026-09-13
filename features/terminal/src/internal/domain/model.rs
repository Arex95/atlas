//! What a terminal is, in this crate's words: a spawned process, the
//! output read back from it, and how a missing workspace was restored.

use std::path::PathBuf;

use atlas_sessions::api::SessionId;

/// What `spawn` and `read_output` hand back — no persistence, this
/// is Type 3 ephemeral state: it exists only in the
/// running `atlas-server` process's memory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpawnedTerminal {
    pub session_id: SessionId,
    pub pid: u32,
    pub resolved_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalOutput {
    pub data: Vec<u8>,
    /// Pass this back as the next call's `since_offset`.
    pub next_offset: usize,
}

/// Outcome of `PtyPool::restore` .
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestoreOutcome {
    /// The resolved path already existed — no git operation ran.
    AlreadyPresent,
    /// The resolved path was missing and a fresh `git clone`
    /// created it.
    Cloned,
}
