//! The four agent-memory tools (Type 1 / Type 2 split).
//!
//! Every tool takes an explicit `scope`, modelled as a serde-tagged
//! enum so the wrong combination of fields fails to parse rather than
//! being silently accepted. That mirrors the CHECK constraint in the
//! schema, so the same rule holds at the wire, in the store, and in
//! the database.
//!
//! **`owner_id` is not a parameter, and sending it is an error.**
//! Rejected rather than ignored: a client written against the old
//! contract would otherwise keep succeeding while writing to a
//! different bucket than it named, which is a worse failure than not
//! working at all.
//!
//! Personal scope acts on the
//! caller's own bucket, taken from the credential the request
//! authenticated with. It used to be an argument, and that meant
//! anyone holding the shared MCP token could name another developer
//! and read, list or delete their personal memory — the exact thing
//! the state model forbids. There is now no field to lie in.

use atlas_memory::api::MemoryStore;
use atlas_sessions::api::Caller;
use serde::Deserialize;
use serde_json::Value;

use crate::internal::infrastructure::jsonrpc::{JsonRpcError, code, memory_error_to_jsonrpc};

#[derive(Deserialize)]
#[serde(tag = "scope", rename_all = "lowercase", deny_unknown_fields)]
enum RememberParams {
    Project {
        project: String,
        key: String,
        value: Value,
    },
    Personal {
        key: String,
        value: Value,
    },
}

#[derive(Deserialize)]
#[serde(tag = "scope", rename_all = "lowercase", deny_unknown_fields)]
enum KeyParams {
    Project { project: String, key: String },
    Personal { key: String },
}

#[derive(Deserialize)]
#[serde(tag = "scope", rename_all = "lowercase", deny_unknown_fields)]
enum ListParams {
    Project { project: String },
    Personal {},
}

fn invalid_params(msg: impl Into<String>) -> JsonRpcError {
    JsonRpcError::new(code::INVALID_PARAMS, msg)
}

pub async fn remember(
    store: &MemoryStore,
    caller: &Caller,
    params: Value,
) -> Result<Value, JsonRpcError> {
    let params: RememberParams =
        serde_json::from_value(params).map_err(|e| invalid_params(format!("bad params: {e}")))?;

    let entry = match params {
        RememberParams::Project {
            project,
            key,
            value,
        } => store.remember_project(&project, &key, &value).await,
        RememberParams::Personal { key, value } => {
            store
                .remember_personal(caller.owner_id(), &key, &value)
                .await
        }
    }
    .map_err(|e| memory_error_to_jsonrpc(&e))?;

    Ok(serde_json::to_value(entry).unwrap_or(Value::Null))
}

pub async fn recall(
    store: &MemoryStore,
    caller: &Caller,
    params: Value,
) -> Result<Value, JsonRpcError> {
    let params: KeyParams =
        serde_json::from_value(params).map_err(|e| invalid_params(format!("bad params: {e}")))?;

    let entry = match params {
        KeyParams::Project { project, key } => store.recall_project(&project, &key).await,
        KeyParams::Personal { key } => store.recall_personal(caller.owner_id(), &key).await,
    }
    .map_err(|e| memory_error_to_jsonrpc(&e))?;

    Ok(serde_json::to_value(entry).unwrap_or(Value::Null))
}

pub async fn list(
    store: &MemoryStore,
    caller: &Caller,
    params: Value,
) -> Result<Value, JsonRpcError> {
    let params: ListParams =
        serde_json::from_value(params).map_err(|e| invalid_params(format!("bad params: {e}")))?;

    let entries = match params {
        ListParams::Project { project } => store.list_project(&project).await,
        ListParams::Personal {} => store.list_personal(caller.owner_id()).await,
    }
    .map_err(|e| memory_error_to_jsonrpc(&e))?;

    Ok(serde_json::to_value(entries).unwrap_or(Value::Null))
}

pub async fn forget(
    store: &MemoryStore,
    caller: &Caller,
    params: Value,
) -> Result<Value, JsonRpcError> {
    let params: KeyParams =
        serde_json::from_value(params).map_err(|e| invalid_params(format!("bad params: {e}")))?;

    match params {
        KeyParams::Project { project, key } => store.forget_project(&project, &key).await,
        KeyParams::Personal { key } => store.forget_personal(caller.owner_id(), &key).await,
    }
    .map_err(|e| memory_error_to_jsonrpc(&e))?;

    Ok(serde_json::json!({ "forgotten": true }))
}
