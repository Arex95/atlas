//! Concrete implementations of the two messaging tools.

use atlas_messaging::api::{MessageStore, NewMessage};
use atlas_sessions::api::{Caller, SessionStore};
use serde::Deserialize;
use serde_json::Value;

use crate::internal::infrastructure::jsonrpc::{
    JsonRpcError, code, messaging_error_to_jsonrpc, sessions_error_to_jsonrpc,
};

const DEFAULT_MESSAGE_TYPE: &str = "message";

#[derive(Deserialize)]
struct SendMessageParams {
    project: String,
    from: String,
    #[serde(default)]
    to: Option<String>,
    #[serde(default)]
    #[serde(rename = "type")]
    message_type: Option<String>,
    payload: Value,
    #[serde(default)]
    correlation_id: Option<String>,
    #[serde(default)]
    reply_to: Option<String>,
}

#[derive(Deserialize)]
struct ReadInboxParams {
    project: String,
    #[serde(rename = "for")]
    for_session: String,
    #[serde(default)]
    since: Option<String>,
    #[serde(default)]
    limit: Option<u32>,
}

fn invalid_params(msg: impl Into<String>) -> JsonRpcError {
    JsonRpcError::new(code::INVALID_PARAMS, msg)
}

pub async fn send_message(store: &MessageStore, params: Value) -> Result<Value, JsonRpcError> {
    let params: SendMessageParams =
        serde_json::from_value(params).map_err(|e| invalid_params(format!("bad params: {e}")))?;
    let input = NewMessage {
        from_session: params.from,
        to_session: params.to,
        message_type: params
            .message_type
            .unwrap_or_else(|| DEFAULT_MESSAGE_TYPE.to_owned()),
        payload: params.payload,
        correlation_id: params.correlation_id,
        reply_to: params.reply_to,
    };
    let message = store
        .send(&params.project, input)
        .await
        .map_err(|e| messaging_error_to_jsonrpc(&e))?;
    Ok(serde_json::to_value(message).unwrap_or(Value::Null))
}

pub async fn read_inbox(
    store: &MessageStore,
    sessions: &SessionStore,
    caller: &Caller,
    params: Value,
) -> Result<Value, JsonRpcError> {
    let params: ReadInboxParams =
        serde_json::from_value(params).map_err(|e| invalid_params(format!("bad params: {e}")))?;
    // An inbox address that names a real session is private to that
    // session's owner: AFG dispatches every `task` to a session id, so
    // this is where the sensitive traffic lives, and without the check
    // any credential could read another developer's.
    //
    // An address that names no session is a free-form label — the
    // deliberate design of this bus, which predates the session
    // registry — and a label has no owner to check against. It is a
    // shared channel, and documented as one.
    //
    // Someone else's session reads as empty rather than as an error.
    // Distinguishing "not yours" from "no such session" would answer
    // whether a session id exists, to anyone who asks.
    if !may_read_inbox(sessions, &params.for_session, caller).await? {
        return Ok(serde_json::json!([]));
    }

    let since = params.since.map(atlas_messaging::api::MessageId);
    let messages = store
        .read_inbox(
            &params.project,
            &params.for_session,
            since.as_ref(),
            params.limit,
        )
        .await
        .map_err(|e| messaging_error_to_jsonrpc(&e))?;
    Ok(serde_json::to_value(messages).unwrap_or(Value::Null))
}

/// Whether this caller may read the inbox at `address`.
///
/// True for an address that names no session (a free-form label), and
/// for one naming a session the caller owns. False only for a session
/// belonging to somebody else.
async fn may_read_inbox(
    sessions: &SessionStore,
    address: &str,
    caller: &Caller,
) -> Result<bool, JsonRpcError> {
    let id = atlas_sessions::api::SessionId(address.to_owned());
    match sessions.get_unscoped(&id).await {
        Ok(session) => Ok(session.owner_id == caller.owner_id()),
        Err(atlas_sessions::api::SessionsError::NotFound) => Ok(true),
        Err(e) => Err(sessions_error_to_jsonrpc(&e)),
    }
}
