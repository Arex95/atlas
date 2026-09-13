//! Axum wiring for the MCP endpoint.
//!
//! One route: `POST /`. The binary mounts this router under
//! `/api/mcp`, so the effective path a client hits is
//! `POST /api/mcp` — plus whatever the auth layer decides.

use std::sync::Arc;

use atlas_afg::api::AfgRuntime;
use atlas_graph::api::{GraphStore, GraphWatcher};
use atlas_memory::api::MemoryStore;
use atlas_messaging::api::MessageStore;
use atlas_notes::api::NoteStore;
use atlas_sessions::api::{Caller, SessionStore};
use atlas_sync::api::SyncSupervisor;
use atlas_terminal::api::PtyPool;
use atlas_tracker::api::TrackerRuntime;
use axum::Extension;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router, middleware};
use serde_json::Value;

use super::handle_request;
use crate::internal::infrastructure::auth::{AuthContext, McpToken, bearer_auth};
use crate::internal::infrastructure::jsonrpc::{
    JsonRpcError, JsonRpcRequest, code, error, success,
};

/// Everything the router needs to serve a request.
#[derive(Clone)]
pub struct McpState {
    pub tracker: TrackerRuntime,
    pub messages: MessageStore,
    pub sessions: SessionStore,
    pub terminals: Arc<PtyPool>,
    pub afg: AfgRuntime,
    pub sync: Arc<SyncSupervisor>,
    pub memory: MemoryStore,
    pub notes: NoteStore,
    pub graph: GraphStore,
    pub graph_watcher: Arc<GraphWatcher>,
    pub token: McpToken,
}

impl McpState {
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        tracker: TrackerRuntime,
        messages: MessageStore,
        sessions: SessionStore,
        terminals: Arc<PtyPool>,
        afg: AfgRuntime,
        sync: Arc<SyncSupervisor>,
        memory: MemoryStore,
        notes: NoteStore,
        graph: GraphStore,
        graph_watcher: Arc<GraphWatcher>,
        token: McpToken,
    ) -> Self {
        Self {
            tracker,
            messages,
            sessions,
            terminals,
            afg,
            sync,
            memory,
            notes,
            graph,
            graph_watcher,
            token,
        }
    }
}

#[derive(Clone)]
struct InnerState {
    tracker: TrackerRuntime,
    messages: MessageStore,
    sessions: SessionStore,
    terminals: Arc<PtyPool>,
    afg: AfgRuntime,
    sync: Arc<SyncSupervisor>,
    memory: MemoryStore,
    notes: NoteStore,
    graph: GraphStore,
    graph_watcher: Arc<GraphWatcher>,
}

pub fn router(state: McpState) -> Router {
    let inner = InnerState {
        tracker: state.tracker,
        messages: state.messages,
        sessions: state.sessions.clone(),
        terminals: state.terminals,
        afg: state.afg,
        sync: state.sync,
        memory: state.memory,
        notes: state.notes,
        graph: state.graph,
        graph_watcher: state.graph_watcher,
    };
    // The middleware needs the session store to resolve a session
    // token, so it gets its own context rather than the bare token.
    let auth_context = AuthContext {
        token: state.token,
        sessions: state.sessions,
    };
    Router::new()
        .route("/", post(handle_post))
        .route_layer(middleware::from_fn_with_state(auth_context, bearer_auth))
        .with_state(Arc::new(inner))
}

async fn handle_post(
    State(inner): State<Arc<InnerState>>,
    // Put there by `bearer_auth`, which is the only thing that can:
    // a client cannot set a request extension.
    Extension(caller): Extension<Caller>,
    body: axum::body::Bytes,
) -> Response {
    // Parse the JSON *once* so we can classify batches, notifications
    // and malformed input independently of the request shape.
    let raw: Value = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(e) => {
            return (
                StatusCode::OK,
                Json(error(
                    Value::Null,
                    JsonRpcError::new(code::PARSE_ERROR, format!("parse error: {e}")),
                )),
            )
                .into_response();
        }
    };

    if raw.is_array() {
        return (
            StatusCode::OK,
            Json(error(
                Value::Null,
                JsonRpcError::new(code::INVALID_REQUEST, "batches are not supported"),
            )),
        )
            .into_response();
    }

    let request: JsonRpcRequest = match serde_json::from_value(raw) {
        Ok(r) => r,
        Err(e) => {
            return (
                StatusCode::OK,
                Json(error(
                    Value::Null,
                    JsonRpcError::new(code::INVALID_REQUEST, format!("invalid request: {e}")),
                )),
            )
                .into_response();
        }
    };

    if request.jsonrpc != "2.0" {
        return (
            StatusCode::OK,
            Json(error(
                request.id.unwrap_or(Value::Null),
                JsonRpcError::new(code::INVALID_REQUEST, "jsonrpc must be \"2.0\""),
            )),
        )
            .into_response();
    }

    let is_notification = request.id.is_none();
    let id_for_response = request.id.clone().unwrap_or(Value::Null);

    let outcome = handle_request(
        &inner.tracker,
        &inner.messages,
        &inner.sessions,
        &inner.terminals,
        &inner.afg,
        &inner.sync,
        &inner.memory,
        &inner.notes,
        &inner.graph,
        &inner.graph_watcher,
        &caller,
        &request.method,
        request.params,
    )
    .await;

    if is_notification {
        return StatusCode::NO_CONTENT.into_response();
    }

    match outcome {
        Ok(result) => (StatusCode::OK, Json(success(id_for_response, result))).into_response(),
        Err(err) => (StatusCode::OK, Json(error(id_for_response, err))).into_response(),
    }
}
