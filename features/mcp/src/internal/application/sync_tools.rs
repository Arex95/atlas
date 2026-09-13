//! Concrete implementation of the `sync.sessions_now` tool
//! (`focus` mode). Constructs a fresh `SyncClient` per
//! call — there is nothing to hold open between calls in `focus`
//! mode, which is the entire point of it.

use atlas_memory::api::MemoryStore;
use atlas_notes::api::NoteStore;
use atlas_sessions::api::{Caller, SessionStore};
use atlas_sync::api::{LiveConfig, SyncClient, SyncConfig, SyncError, SyncMode, SyncSupervisor};
use serde::Deserialize;
use serde_json::Value;

use crate::internal::infrastructure::jsonrpc::{JsonRpcError, code};

#[derive(Deserialize)]
struct SessionsNowParams {
    remote_url: String,
    bearer_token: String,
}

#[derive(Deserialize)]
#[serde(tag = "mode", rename_all = "lowercase")]
enum SetModeParams {
    Focus,
    Auto {
        remote_url: String,
        bearer_token: String,
        interval_secs: u64,
    },
    /// No `interval_secs`: `live` has no cadence to configure, it
    /// reacts to the remote's stream.
    Live {
        remote_url: String,
        bearer_token: String,
    },
}

fn invalid_params(msg: impl Into<String>) -> JsonRpcError {
    JsonRpcError::new(code::INVALID_PARAMS, msg)
}

pub async fn sessions_now(
    sessions: &SessionStore,
    caller: &Caller,
    params: Value,
) -> Result<Value, JsonRpcError> {
    let params: SessionsNowParams =
        serde_json::from_value(params).map_err(|e| invalid_params(format!("bad params: {e}")))?;

    let client = SyncClient::new(&params.remote_url).map_err(sync_error_to_jsonrpc)?;
    let report = client
        .sync_sessions(&params.bearer_token, caller.owner_id(), sessions)
        .await
        .map_err(sync_error_to_jsonrpc)?;

    Ok(serde_json::json!({ "pushed": report.pushed, "pulled": report.pulled }))
}

pub async fn notes_now(
    notes: &NoteStore,
    caller: &Caller,
    params: Value,
) -> Result<Value, JsonRpcError> {
    let params: SessionsNowParams =
        serde_json::from_value(params).map_err(|e| invalid_params(format!("bad params: {e}")))?;

    let client = SyncClient::new(&params.remote_url).map_err(sync_error_to_jsonrpc)?;
    let report = client
        .sync_notes(&params.bearer_token, caller.owner_id(), notes)
        .await
        .map_err(sync_error_to_jsonrpc)?;
    Ok(serde_json::json!({ "pushed": report.pushed, "pulled": report.pulled }))
}

pub async fn memory_now(
    memory: &MemoryStore,
    caller: &Caller,
    params: Value,
) -> Result<Value, JsonRpcError> {
    let params: SessionsNowParams =
        serde_json::from_value(params).map_err(|e| invalid_params(format!("bad params: {e}")))?;

    let client = SyncClient::new(&params.remote_url).map_err(sync_error_to_jsonrpc)?;
    let report = client
        .sync_memory(&params.bearer_token, caller.owner_id(), memory)
        .await
        .map_err(sync_error_to_jsonrpc)?;

    Ok(serde_json::json!({ "pushed": report.pushed, "pulled": report.pulled }))
}

pub async fn set_mode(
    sync: &SyncSupervisor,
    caller: &Caller,
    params: Value,
) -> Result<Value, JsonRpcError> {
    let params: SetModeParams =
        serde_json::from_value(params).map_err(|e| invalid_params(format!("bad params: {e}")))?;

    match params {
        SetModeParams::Focus => {
            sync.set_focus().await;
        }
        SetModeParams::Auto {
            remote_url,
            bearer_token,
            interval_secs,
        } => {
            sync.set_auto(SyncConfig {
                remote_url,
                bearer_token,
                owner_id: caller.owner_id().to_owned(),
                interval_secs,
            })
            .await
            .map_err(sync_error_to_jsonrpc)?;
        }
        SetModeParams::Live {
            remote_url,
            bearer_token,
        } => {
            sync.set_live(LiveConfig {
                remote_url,
                bearer_token,
                owner_id: caller.owner_id().to_owned(),
            })
            .await
            .map_err(sync_error_to_jsonrpc)?;
        }
    }

    Ok(status_json(&sync.status().await))
}

pub async fn status(sync: &SyncSupervisor) -> Value {
    status_json(&sync.status().await)
}

fn status_json(status: &atlas_sync::api::SyncStatusReport) -> Value {
    let mode = match status.mode {
        SyncMode::Focus => "focus",
        SyncMode::Auto => "auto",
        SyncMode::Live => "live",
    };
    serde_json::json!({
        "mode": mode,
        "interval_secs": status.interval_secs,
        "stream_connected": status.stream_connected,
        "last_synced_at": status.last_synced_at,
        "last_report": status.last_report.map(|r| serde_json::json!({ "pushed": r.pushed, "pulled": r.pulled })),
        "last_error": status.last_error,
    })
}

fn sync_error_to_jsonrpc(err: SyncError) -> JsonRpcError {
    match err {
        SyncError::BadUrl(msg) => invalid_params(format!("remote_url: {msg}")),
        SyncError::Unauthorized => {
            JsonRpcError::new(code::INTERNAL_ERROR, "remote rejected the bearer token")
        }
        SyncError::RemoteError { status, body } => JsonRpcError::new(
            code::INTERNAL_ERROR,
            format!("remote returned {status}: {body}"),
        ),
        SyncError::Transport(msg) => {
            JsonRpcError::new(code::INTERNAL_ERROR, format!("sync transport: {msg}"))
        }
        SyncError::Malformed(msg) => JsonRpcError::new(
            code::INTERNAL_ERROR,
            format!("sync: malformed response: {msg}"),
        ),
        SyncError::Local(msg) => {
            JsonRpcError::new(code::INTERNAL_ERROR, format!("sync: local storage: {msg}"))
        }
    }
}
