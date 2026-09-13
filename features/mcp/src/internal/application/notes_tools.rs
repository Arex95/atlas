//! The four personal-note tools (Type 2).
//!
//! **There is no `owner_id` parameter, and no `scope` either.** A note
//! is personal by construction — there is no shared variant to select
//! — so the only thing a caller could name is somebody else, and that
//! is precisely what must not be nameable. The owner comes from the
//! credential the request authenticated with.
//!
//! Addressed by a name the developer chooses rather than by a
//! generated id, so a write is idempotent and a terminal can reach a
//! note without having copied an id around. A scratchpad is simply the
//! note called `scratchpad`.

use atlas_notes::api::NoteStore;
use atlas_sessions::api::Caller;
use serde::Deserialize;
use serde_json::Value;

use crate::internal::infrastructure::jsonrpc::{JsonRpcError, code, notes_error_to_jsonrpc};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WriteParams {
    name: String,
    body: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NameParams {
    name: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NoParams {}

fn invalid_params(msg: impl Into<String>) -> JsonRpcError {
    JsonRpcError::new(code::INVALID_PARAMS, msg)
}

pub async fn write(
    store: &NoteStore,
    caller: &Caller,
    params: Value,
) -> Result<Value, JsonRpcError> {
    let params: WriteParams =
        serde_json::from_value(params).map_err(|e| invalid_params(format!("bad params: {e}")))?;

    let note = store
        .write(caller.owner_id(), &params.name, &params.body)
        .await
        .map_err(|e| notes_error_to_jsonrpc(&e))?;
    Ok(serde_json::to_value(note).unwrap_or(Value::Null))
}

pub async fn read(
    store: &NoteStore,
    caller: &Caller,
    params: Value,
) -> Result<Value, JsonRpcError> {
    let params: NameParams =
        serde_json::from_value(params).map_err(|e| invalid_params(format!("bad params: {e}")))?;

    let note = store
        .read(caller.owner_id(), &params.name)
        .await
        .map_err(|e| notes_error_to_jsonrpc(&e))?;
    Ok(serde_json::to_value(note).unwrap_or(Value::Null))
}

pub async fn list(
    store: &NoteStore,
    caller: &Caller,
    params: Value,
) -> Result<Value, JsonRpcError> {
    // Parsed even though it takes nothing, so a caller that passed
    // `name` — expecting `read` — is told rather than handed the whole
    // list as though the argument had been honoured.
    let _: NoParams =
        serde_json::from_value(params).map_err(|e| invalid_params(format!("bad params: {e}")))?;

    let notes = store
        .list(caller.owner_id())
        .await
        .map_err(|e| notes_error_to_jsonrpc(&e))?;
    Ok(serde_json::to_value(notes).unwrap_or(Value::Null))
}

pub async fn delete(
    store: &NoteStore,
    caller: &Caller,
    params: Value,
) -> Result<Value, JsonRpcError> {
    let params: NameParams =
        serde_json::from_value(params).map_err(|e| invalid_params(format!("bad params: {e}")))?;

    store
        .delete(caller.owner_id(), &params.name)
        .await
        .map_err(|e| notes_error_to_jsonrpc(&e))?;
    Ok(serde_json::json!({ "deleted": true }))
}
