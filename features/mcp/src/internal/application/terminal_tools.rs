//! Every one of these names the session to act on, so every one of
//! them checks that the caller owns it. Scoped by **owner**, not by
//! session: Atlas exists to let one developer's agents drive one
//! another, so a session's credential may reach that developer's
//! other sessions — and nobody else's.
//!
//! Before this, `terminal.write` took any session id and wrote to its
//! PTY, which is arbitrary command execution in another developer's
//! shell. Verified against a real container.
//!
//! Concrete implementations of the four terminal tools.

use atlas_sessions::api::Caller;
use atlas_sessions::api::SessionId;
use atlas_terminal::api::PtyPool;
use serde::Deserialize;
use serde_json::Value;

use crate::internal::infrastructure::jsonrpc::{JsonRpcError, code, terminal_error_to_jsonrpc};

#[derive(Deserialize)]
struct SpawnParams {
    session_id: String,
}

#[derive(Deserialize)]
struct WriteParams {
    session_id: String,
    input: String,
}

#[derive(Deserialize)]
struct ReadOutputParams {
    session_id: String,
    #[serde(default)]
    since_offset: usize,
}

#[derive(Deserialize)]
struct CloseParams {
    session_id: String,
}

#[derive(Deserialize)]
struct RestoreParams {
    session_id: String,
}

fn invalid_params(msg: impl Into<String>) -> JsonRpcError {
    JsonRpcError::new(code::INVALID_PARAMS, msg)
}

pub async fn spawn(pool: &PtyPool, caller: &Caller, params: Value) -> Result<Value, JsonRpcError> {
    let params: SpawnParams =
        serde_json::from_value(params).map_err(|e| invalid_params(format!("bad params: {e}")))?;
    let spawned = pool
        .spawn(&SessionId(params.session_id), caller.owner_id())
        .await
        .map_err(|e| terminal_error_to_jsonrpc(&e))?;
    Ok(serde_json::json!({
        "session_id": spawned.session_id.0,
        "pid": spawned.pid,
        "resolved_path": spawned.resolved_path.display().to_string(),
    }))
}

pub fn write(pool: &PtyPool, caller: &Caller, params: Value) -> Result<Value, JsonRpcError> {
    let params: WriteParams =
        serde_json::from_value(params).map_err(|e| invalid_params(format!("bad params: {e}")))?;
    pool.write(
        &SessionId(params.session_id),
        caller.owner_id(),
        params.input.as_bytes(),
    )
    .map_err(|e| terminal_error_to_jsonrpc(&e))?;
    Ok(Value::Bool(true))
}

pub fn read_output(pool: &PtyPool, caller: &Caller, params: Value) -> Result<Value, JsonRpcError> {
    let params: ReadOutputParams =
        serde_json::from_value(params).map_err(|e| invalid_params(format!("bad params: {e}")))?;
    let output = pool
        .read_output(
            &SessionId(params.session_id),
            caller.owner_id(),
            params.since_offset,
        )
        .map_err(|e| terminal_error_to_jsonrpc(&e))?;
    Ok(serde_json::json!({
        "data": String::from_utf8_lossy(&output.data),
        "next_offset": output.next_offset,
    }))
}

pub fn close(pool: &PtyPool, caller: &Caller, params: Value) -> Result<Value, JsonRpcError> {
    let params: CloseParams =
        serde_json::from_value(params).map_err(|e| invalid_params(format!("bad params: {e}")))?;
    pool.close(&SessionId(params.session_id), caller.owner_id())
        .map_err(|e| terminal_error_to_jsonrpc(&e))?;
    Ok(Value::Bool(true))
}

pub async fn restore(
    pool: &PtyPool,
    caller: &Caller,
    params: Value,
) -> Result<Value, JsonRpcError> {
    let params: RestoreParams =
        serde_json::from_value(params).map_err(|e| invalid_params(format!("bad params: {e}")))?;
    let outcome = pool
        .restore(&SessionId(params.session_id), caller.owner_id())
        .await
        .map_err(|e| terminal_error_to_jsonrpc(&e))?;
    let outcome_str = match outcome {
        atlas_terminal::api::RestoreOutcome::AlreadyPresent => "already_present",
        atlas_terminal::api::RestoreOutcome::Cloned => "cloned",
    };
    Ok(serde_json::json!({ "outcome": outcome_str }))
}
