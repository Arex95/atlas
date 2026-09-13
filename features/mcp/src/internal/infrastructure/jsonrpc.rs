//! JSON-RPC 2.0 envelopes and error mapping.
//!
//! One request → one response. Notifications (no `id`) yield an
//! empty response (204 in the HTTP layer). Batches are refused
//! with `-32600` — this issue does not implement them.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use atlas_afg::api::AfgError;
use atlas_graph::api::GraphError;
use atlas_memory::api::MemoryError;
use atlas_messaging::api::MessagingError;
use atlas_notes::api::NotesError;
use atlas_sessions::api::SessionsError;
use atlas_terminal::api::TerminalError;
use atlas_tracker::api::TrackerError;

/// Application-error codes carved out from the JSON-RPC reserved
/// range (`-32000` .. `-32099`). Kept as constants so nobody
/// invents new ones inline.
pub mod code {
    pub const PARSE_ERROR: i32 = -32700;
    pub const INVALID_REQUEST: i32 = -32600;
    pub const METHOD_NOT_FOUND: i32 = -32601;
    pub const INVALID_PARAMS: i32 = -32602;
    pub const INTERNAL_ERROR: i32 = -32603;

    pub const TRACKER_NOT_FOUND: i32 = -32000;
    pub const TRACKER_UNAUTHORIZED: i32 = -32001;
    pub const TRACKER_RATE_LIMITED: i32 = -32002;
    pub const TRACKER_DISABLED: i32 = -32003;
    pub const TRACKER_INVALID: i32 = -32004;
    pub const TRACKER_CONFLICT: i32 = -32005;
    pub const SESSION_NOT_FOUND: i32 = -32006;
    pub const TERMINAL_NOT_RUNNING: i32 = -32007;
    pub const TERMINAL_PATH_NOT_FOUND: i32 = -32008;
    pub const AFG_NOT_FOUND: i32 = -32009;
    pub const TERMINAL_CREDENTIALS_MISSING: i32 = -32010;
    pub const MEMORY_NOT_FOUND: i32 = -32011;
    pub const NOTE_NOT_FOUND: i32 = -32012;
    pub const GRAPH_NOT_FOUND: i32 = -32012;
}

#[derive(Debug, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    #[serde(default)]
    pub id: Option<Value>,
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

#[derive(Debug, Serialize)]
pub struct JsonRpcSuccess {
    pub jsonrpc: &'static str,
    pub id: Value,
    pub result: Value,
}

#[derive(Debug, Serialize)]
pub struct JsonRpcErrorResponse {
    pub jsonrpc: &'static str,
    pub id: Value,
    pub error: JsonRpcError,
}

#[derive(Debug, Serialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl JsonRpcError {
    #[must_use]
    pub fn new(code: i32, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            data: None,
        }
    }

    #[must_use]
    pub fn with_data(mut self, data: Value) -> Self {
        self.data = Some(data);
        self
    }
}

#[must_use]
pub fn success(id: Value, result: Value) -> JsonRpcSuccess {
    JsonRpcSuccess {
        jsonrpc: "2.0",
        id,
        result,
    }
}

#[must_use]
pub fn error(id: Value, err: JsonRpcError) -> JsonRpcErrorResponse {
    JsonRpcErrorResponse {
        jsonrpc: "2.0",
        id,
        error: err,
    }
}

#[must_use]
pub fn tracker_error_to_jsonrpc(err: &TrackerError) -> JsonRpcError {
    match err {
        TrackerError::NotFound => JsonRpcError::new(code::TRACKER_NOT_FOUND, "not found")
            .with_data(serde_json::json!({ "kind": "not_found" })),
        TrackerError::Unauthorized => {
            JsonRpcError::new(code::TRACKER_UNAUTHORIZED, "tracker: unauthorized")
                .with_data(serde_json::json!({ "kind": "unauthorized" }))
        }
        TrackerError::RateLimited { retry_after } => {
            let mut data = serde_json::json!({ "kind": "rate_limited" });
            if let Some(after) = retry_after {
                data["retry_after_secs"] = serde_json::json!(after.as_secs());
            }
            JsonRpcError::new(code::TRACKER_RATE_LIMITED, "tracker: rate limited").with_data(data)
        }
        TrackerError::Disabled => {
            JsonRpcError::new(code::TRACKER_DISABLED, "tracker feature is disabled")
                .with_data(serde_json::json!({ "kind": "disabled" }))
        }
        TrackerError::Invalid(msg) => JsonRpcError::new(
            code::TRACKER_INVALID,
            format!("tracker: rejected as invalid: {msg}"),
        )
        .with_data(serde_json::json!({ "kind": "invalid" })),
        TrackerError::Conflict(msg) => JsonRpcError::new(
            code::TRACKER_CONFLICT,
            format!("tracker: write conflicted with tracker state: {msg}"),
        )
        .with_data(serde_json::json!({ "kind": "conflict" })),
        TrackerError::Transport(msg) | TrackerError::Malformed(msg) => {
            JsonRpcError::new(code::INTERNAL_ERROR, format!("tracker: {msg}"))
        }
    }
}

#[must_use]
pub fn messaging_error_to_jsonrpc(err: &MessagingError) -> JsonRpcError {
    match err {
        MessagingError::EmptyProject => {
            JsonRpcError::new(code::INVALID_PARAMS, "project must not be empty")
        }
        MessagingError::EmptyFromSession => {
            JsonRpcError::new(code::INVALID_PARAMS, "from must not be empty")
        }
        MessagingError::Storage(msg) => {
            JsonRpcError::new(code::INTERNAL_ERROR, format!("messaging: {msg}"))
        }
    }
}

#[must_use]
pub fn sessions_error_to_jsonrpc(err: &SessionsError) -> JsonRpcError {
    match err {
        SessionsError::EmptyProject => {
            JsonRpcError::new(code::INVALID_PARAMS, "project must not be empty")
        }
        SessionsError::EmptyRemoteUrl => {
            JsonRpcError::new(code::INVALID_PARAMS, "remote_url must not be empty")
        }
        SessionsError::EmptyOwnerId => {
            JsonRpcError::new(code::INVALID_PARAMS, "owner_id must not be empty")
        }
        SessionsError::OwnerMismatch => {
            JsonRpcError::new(code::INTERNAL_ERROR, "session id owned by a different user")
        }
        SessionsError::InvalidStatus(status) => JsonRpcError::new(
            code::INVALID_PARAMS,
            format!("status must be \"active\" or \"archived\", got {status:?}"),
        ),
        SessionsError::NotFound => JsonRpcError::new(code::SESSION_NOT_FOUND, "session not found")
            .with_data(serde_json::json!({ "kind": "not_found" })),
        SessionsError::Storage(msg) => {
            JsonRpcError::new(code::INTERNAL_ERROR, format!("sessions: {msg}"))
        }
    }
}

#[must_use]
pub fn terminal_error_to_jsonrpc(err: &TerminalError) -> JsonRpcError {
    match err {
        TerminalError::SessionNotFound => JsonRpcError::new(
            code::SESSION_NOT_FOUND,
            "no session registered with that id",
        )
        .with_data(serde_json::json!({ "kind": "not_found" })),
        TerminalError::PathNotFound(path) => JsonRpcError::new(
            code::TERMINAL_PATH_NOT_FOUND,
            format!(
                "resolved path does not exist on this machine: {}",
                path.display()
            ),
        )
        .with_data(serde_json::json!({ "kind": "path_not_found" })),
        TerminalError::NotRunning => JsonRpcError::new(
            code::TERMINAL_NOT_RUNNING,
            "no PTY is running for that session",
        )
        .with_data(serde_json::json!({ "kind": "not_running" })),
        TerminalError::Spawn(msg) => JsonRpcError::new(
            code::INTERNAL_ERROR,
            format!("terminal: failed to spawn: {msg}"),
        ),
        TerminalError::Io(msg) => {
            JsonRpcError::new(code::INTERNAL_ERROR, format!("terminal: {msg}"))
        }
        TerminalError::CredentialsMissing(msg) => JsonRpcError::new(
            code::TERMINAL_CREDENTIALS_MISSING,
            format!("git clone failed, likely missing credentials: {msg}"),
        )
        .with_data(serde_json::json!({ "kind": "credentials_missing" })),
        TerminalError::CloneFailed(msg) => JsonRpcError::new(
            code::INTERNAL_ERROR,
            format!("terminal: git clone failed: {msg}"),
        ),
    }
}

#[must_use]
pub fn graph_error_to_jsonrpc(err: &GraphError) -> JsonRpcError {
    match err {
        GraphError::EmptyProject | GraphError::RootNotADirectory(_) => {
            JsonRpcError::new(code::INVALID_PARAMS, format!("graph: {err}"))
        }
        GraphError::NotFound => JsonRpcError::new(code::GRAPH_NOT_FOUND, err.to_string())
            .with_data(serde_json::json!({ "kind": "not_found" })),
        GraphError::Walk(msg) => JsonRpcError::new(
            code::INTERNAL_ERROR,
            format!("graph: could not read the project tree: {msg}"),
        ),
        GraphError::Storage(msg) => {
            JsonRpcError::new(code::INTERNAL_ERROR, format!("graph: storage: {msg}"))
        }
    }
}

pub fn memory_error_to_jsonrpc(err: &MemoryError) -> JsonRpcError {
    match err {
        MemoryError::EmptyKey | MemoryError::EmptyProject | MemoryError::EmptyOwner => {
            JsonRpcError::new(code::INVALID_PARAMS, format!("memory: {err}"))
        }
        MemoryError::NotFound => JsonRpcError::new(code::MEMORY_NOT_FOUND, err.to_string())
            .with_data(serde_json::json!({ "kind": "not_found" })),
        MemoryError::Corrupt(msg) => JsonRpcError::new(
            code::INTERNAL_ERROR,
            format!("memory: stored value is unreadable: {msg}"),
        ),
        MemoryError::Storage(msg) => {
            JsonRpcError::new(code::INTERNAL_ERROR, format!("memory: storage: {msg}"))
        }
    }
}

pub fn notes_error_to_jsonrpc(err: &NotesError) -> JsonRpcError {
    match err {
        NotesError::EmptyName | NotesError::NameTooLong(_) | NotesError::EmptyOwnerId => {
            JsonRpcError::new(code::INVALID_PARAMS, format!("notes: {err}"))
        }
        // Its own code rather than a generic not-found, so a caller can
        // tell "no note by that name" from a transport failure without
        // matching on prose.
        NotesError::NotFound => JsonRpcError::new(code::NOTE_NOT_FOUND, err.to_string())
            .with_data(serde_json::json!({ "kind": "not_found" })),
        NotesError::Storage(msg) => {
            JsonRpcError::new(code::INTERNAL_ERROR, format!("notes: storage: {msg}"))
        }
    }
}

pub fn afg_error_to_jsonrpc(err: &AfgError) -> JsonRpcError {
    match err {
        AfgError::Parse(msg) => {
            JsonRpcError::new(code::INVALID_PARAMS, format!("afg: invalid YAML: {msg}"))
        }
        AfgError::Validation(msg) => {
            JsonRpcError::new(code::INVALID_PARAMS, format!("afg: invalid spec: {msg}"))
        }
        AfgError::NotFound(what) => {
            JsonRpcError::new(code::AFG_NOT_FOUND, format!("{what} not found"))
                .with_data(serde_json::json!({ "kind": "not_found" }))
        }
        AfgError::Io(msg) => JsonRpcError::new(
            code::INTERNAL_ERROR,
            format!("afg: could not read workflow file: {msg}"),
        ),
        AfgError::MissingSource => JsonRpcError::new(
            code::INVALID_PARAMS,
            "afg: provide either source_path or yaml",
        ),
        AfgError::PathOutsideProject => JsonRpcError::new(
            code::INVALID_PARAMS,
            "afg: workflow source path escapes the project root",
        ),
        AfgError::Storage(msg) => JsonRpcError::new(code::INTERNAL_ERROR, format!("afg: {msg}")),
    }
}
