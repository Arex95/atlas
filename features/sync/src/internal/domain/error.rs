//! What can go wrong talking to a remote, in this crate's words.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum SyncError {
    #[error("bad remote_url: {0}")]
    BadUrl(String),

    #[error("unauthorized: the remote rejected the bearer token")]
    Unauthorized,

    #[error("remote returned {status}: {body}")]
    RemoteError { status: u16, body: String },

    #[error("transport failure talking to the remote: {0}")]
    Transport(String),

    #[error("malformed response from the remote: {0}")]
    Malformed(String),

    #[error("local storage failure: {0}")]
    Local(String),
}
