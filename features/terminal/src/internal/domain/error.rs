//! What can go wrong driving a PTY, in this crate's words.

use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum TerminalError {
    #[error("no session registered with that id")]
    SessionNotFound,

    #[error("resolved path does not exist on this machine: {0}")]
    PathNotFound(PathBuf),

    #[error("no PTY is running for that session")]
    NotRunning,

    #[error("failed to spawn the PTY: {0}")]
    Spawn(String),

    #[error("PTY I/O failure: {0}")]
    Io(String),

    /// `git clone` failed in a way that looks like missing/invalid
    /// credentials (Atlas "detects and warns, does not
    /// resolve" this) — distinct from `CloneFailed` so a caller can
    /// tell a developer specifically "your git credentials aren't
    /// set up on this machine" instead of a generic failure.
    #[error("git clone failed, likely missing credentials: {0}")]
    CredentialsMissing(String),

    #[error("git clone failed: {0}")]
    CloneFailed(String),
}
