use std::time::Duration;

use thiserror::Error;

/// Errors any [`IssueTracker`](super::port::IssueTracker) implementation may return.
///
/// Variants are chosen so the caller can map to HTTP status codes
/// without inspecting the transport:
/// `NotFound` → 404, `Unauthorized` → 401, `RateLimited` → 429,
/// `Transport`/`Malformed` → 502, `Disabled` → the feature is off
/// and the caller should treat it as "no tracker configured",
/// `Invalid` → 422 (a write the tracker rejected as malformed),
/// `Conflict` → 409 (a write that raced tracker state).
#[derive(Debug, Error)]
pub enum TrackerError {
    #[error("resource not found")]
    NotFound,

    #[error("unauthorized: check the tracker credentials")]
    Unauthorized,

    #[error("rate limited by the tracker")]
    RateLimited { retry_after: Option<Duration> },

    #[error("transport failure talking to the tracker: {0}")]
    Transport(String),

    #[error("tracker returned a payload we could not decode: {0}")]
    Malformed(String),

    #[error("tracker feature is disabled by configuration")]
    Disabled,

    #[error("the write was rejected as invalid: {0}")]
    Invalid(String),

    #[error("the write conflicted with tracker state: {0}")]
    Conflict(String),
}
