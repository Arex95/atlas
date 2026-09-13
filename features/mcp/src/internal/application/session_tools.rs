//! Concrete implementations of the four session-registry tools.
//!
//! **`owner_id` is not a parameter, and sending it is an error.**
//! Rejected rather than ignored: a client written against the old
//! contract would otherwise keep succeeding while acting on a
//! different owner than it named.
//!
//! Every one of these acts on the
//! caller's own sessions, taken from the credential the request
//! authenticated with. It used to be an argument, which meant anyone
//! holding the shared MCP token could name another developer and read
//! or modify their sessions.
//!
//! `create` is where the credential itself comes from: it mints a
//! session token and returns it **once**. That token is what the
//! session's agent authenticates with afterwards, and it is the only
//! time it is ever visible.

use atlas_sessions::api::{Caller, NewSession, SessionId, SessionStatus, SessionStore};
use serde::Deserialize;
use serde_json::Value;

use crate::internal::infrastructure::jsonrpc::{JsonRpcError, code, sessions_error_to_jsonrpc};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateParams {
    project: String,
    remote_url: String,
    branch: String,
    relative_path: String,
    #[serde(default)]
    agent_kind: Option<String>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    resume_command: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ListParams {
    project: String,
    #[serde(default)]
    status: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GetParams {
    id: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UpdateStatusParams {
    id: String,
    status: String,
}

fn invalid_params(msg: impl Into<String>) -> JsonRpcError {
    JsonRpcError::new(code::INVALID_PARAMS, msg)
}

fn parse_status(raw: &str) -> Result<SessionStatus, JsonRpcError> {
    SessionStatus::parse(raw).ok_or_else(|| {
        invalid_params(format!(
            "status must be \"active\" or \"archived\", got {raw:?}"
        ))
    })
}

pub async fn create(
    store: &SessionStore,
    caller: &Caller,
    params: Value,
) -> Result<Value, JsonRpcError> {
    let params: CreateParams =
        serde_json::from_value(params).map_err(|e| invalid_params(format!("bad params: {e}")))?;
    let input = NewSession {
        project: params.project,
        owner_id: caller.owner_id().to_owned(),
        remote_url: params.remote_url,
        branch: params.branch,
        relative_path: params.relative_path,
        agent_kind: params.agent_kind,
        title: params.title,
        resume_command: params.resume_command,
    };
    let (session, token) = store
        .create_with_token(input)
        .await
        .map_err(|e| sessions_error_to_jsonrpc(&e))?;

    let mut body = serde_json::to_value(session).unwrap_or(Value::Null);
    if let Some(object) = body.as_object_mut() {
        // Shown here and nowhere else, ever: only its hash is stored.
        // The agent for this session authenticates with it, and every
        // call it then makes is attributable to this session and its
        // owner instead of asserting an identity in its arguments.
        object.insert("session_token".to_owned(), Value::String(token));
    }
    Ok(body)
}

pub async fn list(
    store: &SessionStore,
    caller: &Caller,
    params: Value,
) -> Result<Value, JsonRpcError> {
    let params: ListParams =
        serde_json::from_value(params).map_err(|e| invalid_params(format!("bad params: {e}")))?;
    let status = params.status.as_deref().map(parse_status).transpose()?;
    let sessions = store
        .list(&params.project, caller.owner_id(), status)
        .await
        .map_err(|e| sessions_error_to_jsonrpc(&e))?;
    Ok(serde_json::to_value(sessions).unwrap_or(Value::Null))
}

pub async fn get(
    store: &SessionStore,
    caller: &Caller,
    params: Value,
) -> Result<Value, JsonRpcError> {
    let params: GetParams =
        serde_json::from_value(params).map_err(|e| invalid_params(format!("bad params: {e}")))?;
    let session = store
        .get(&SessionId(params.id), caller.owner_id())
        .await
        .map_err(|e| sessions_error_to_jsonrpc(&e))?;
    Ok(serde_json::to_value(session).unwrap_or(Value::Null))
}

pub async fn update_status(
    store: &SessionStore,
    caller: &Caller,
    params: Value,
) -> Result<Value, JsonRpcError> {
    let params: UpdateStatusParams =
        serde_json::from_value(params).map_err(|e| invalid_params(format!("bad params: {e}")))?;
    let status = parse_status(&params.status)?;
    let session = store
        .update_status(&SessionId(params.id), caller.owner_id(), status)
        .await
        .map_err(|e| sessions_error_to_jsonrpc(&e))?;
    Ok(serde_json::to_value(session).unwrap_or(Value::Null))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SetResumeCommandParams {
    id: String,
    /// Omitted or null clears it, so a session can go back to a bare
    /// shell without being recreated.
    #[serde(default)]
    command: Option<String>,
}

pub async fn set_resume_command(
    store: &SessionStore,
    caller: &Caller,
    params: Value,
) -> Result<Value, JsonRpcError> {
    let params: SetResumeCommandParams =
        serde_json::from_value(params).map_err(|e| invalid_params(format!("bad params: {e}")))?;

    let session = store
        .set_resume_command(
            &SessionId(params.id),
            caller.owner_id(),
            params.command.as_deref(),
        )
        .await
        .map_err(|e| sessions_error_to_jsonrpc(&e))?;
    Ok(serde_json::to_value(session).unwrap_or(Value::Null))
}
