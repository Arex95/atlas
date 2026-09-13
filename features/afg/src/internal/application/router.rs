//! Axum wiring for `/api/afg/**` — the live run view (AFG slice 4).
//!
//! Authenticated with an `atlas-auth` bearer session token, not the
//! shared `ATLAS_MCP_TOKEN`: this is a surface a person watches, not
//! one an agent drives. Everything an agent does with AFG still goes
//! through the MCP tools, unchanged.
//!
//! Mounted by the binary at `/api/afg`, so the effective path is
//! `GET /api/afg/runs/{run_id}/events`.

use std::collections::HashSet;
use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;
use std::vec;

use atlas_auth::api::{AuthError, AuthStore, User};
use atlas_sessions::api::{SessionId, SessionStore};
use axum::extract::{Path, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use futures_util::StreamExt;
use serde_json::json;
use tokio::sync::broadcast;

use crate::internal::domain::{AfgError, NodeEventKind, RunId, WorkflowNodeEvent};
use crate::internal::infrastructure::AfgStore;

/// How often an idle stream emits a comment so proxies and NAT
/// tables don't reap a run that is simply thinking.
const KEEP_ALIVE_SECS: u64 = 15;

struct AfgState {
    auth: AuthStore,
    sessions: SessionStore,
    store: AfgStore,
}

pub fn router(store: AfgStore, auth: AuthStore, sessions: SessionStore) -> Router {
    Router::new()
        .route("/runs/{run_id}/events", get(run_events))
        .with_state(Arc::new(AfgState {
            auth,
            sessions,
            store,
        }))
}

/// Everything one watcher's stream carries between polls.
struct WatchState {
    history: vec::IntoIter<WorkflowNodeEvent>,
    live: broadcast::Receiver<WorkflowNodeEvent>,
    run_id: RunId,
    /// Ids already delivered from history, so an event caught by both
    /// the history read and the subscription is sent exactly once.
    replayed: HashSet<String>,
    /// Set when a terminal event has been yielded. Checked before the
    /// next receive, so the connection closes immediately after the
    /// run ends rather than waiting for an event that will never come.
    finished: bool,
}

/// Replays the run's timeline, then continues with events as they are
/// recorded, on one connection.
///
/// Unlike `/api/sync/events`, each event carries its payload — see
/// Workflow node events are append-only and immutable, so a
/// payload-carrying stream creates no second consistency path, and a
/// viewer would otherwise re-fetch the entire run once per event.
async fn run_events(
    State(state): State<Arc<AfgState>>,
    Path(run_id): Path<String>,
    headers: HeaderMap,
) -> Response {
    let user = match authenticate(&state.auth, &headers).await {
        Ok(user) => user,
        Err(resp) => return resp,
    };
    let run_id = RunId(run_id);

    // Subscribe *before* reading history. An event recorded in the
    // window between the two would otherwise fall in the gap: too
    // late for the history read, too early for the subscription.
    let live = state.store.subscribe_events();

    let detail = match state.store.get_run_detail(&run_id).await {
        Ok(detail) => detail,
        Err(AfgError::NotFound(_)) => return not_found(),
        Err(e) => return afg_error_response(&e),
    };

    if !owns_run(&state, &user, &detail.run.initiator_session_id).await {
        // 404, not 403: a run this caller may not see must not be
        // confirmed to exist.
        return not_found();
    }

    let replayed = detail.events.iter().map(|e| e.id.clone()).collect();
    let watch = WatchState {
        history: detail.events.into_iter(),
        live,
        run_id,
        replayed,
        finished: false,
    };

    // `.fuse()` is load-bearing, not decoration: `unfold` panics if
    // polled again after it has ended, and the SSE body does exactly
    // that once the terminal event closes the stream.
    let stream = futures_util::stream::unfold(watch, |mut watch| async move {
        if watch.finished {
            return None;
        }

        // History first, in order, before anything live.
        if let Some(event) = watch.history.next() {
            watch.finished = is_terminal(event.kind);
            return Some((Ok::<Event, Infallible>(sse_event(&event)), watch));
        }

        loop {
            match watch.live.recv().await {
                Ok(event) => {
                    if event.run_id != watch.run_id || watch.replayed.contains(&event.id) {
                        continue;
                    }
                    watch.finished = is_terminal(event.kind);
                    return Some((Ok(sse_event(&event)), watch));
                }
                // A watcher that fell behind has a hole in its
                // timeline and no pull to repair it. Say
                // so plainly rather than let it render an incomplete
                // run as if it were whole.
                Err(broadcast::error::RecvError::Lagged(missed)) => {
                    return Some((
                        Ok(Event::default()
                            .event("lagged")
                            .data(json!({ "missed": missed }).to_string())),
                        watch,
                    ));
                }
                Err(broadcast::error::RecvError::Closed) => return None,
            }
        }
    })
    .fuse();

    Sse::new(stream)
        .keep_alive(KeepAlive::default().interval(Duration::from_secs(KEEP_ALIVE_SECS)))
        .into_response()
}

fn is_terminal(kind: NodeEventKind) -> bool {
    matches!(kind, NodeEventKind::Complete | NodeEventKind::Error)
}

fn sse_event(event: &WorkflowNodeEvent) -> Event {
    Event::default().event(event.kind.as_str()).data(
        json!({
            "id": event.id,
            "run_id": event.run_id,
            "node_id": event.node_id,
            "kind": event.kind,
            "payload": event.payload,
            "message_id": event.message_id,
            "at": event.at,
        })
        .to_string(),
    )
}

async fn owns_run(state: &AfgState, user: &User, initiator_session_id: &str) -> bool {
    let session_id = SessionId(initiator_session_id.to_owned());
    match state.sessions.get_unscoped(&session_id).await {
        Ok(session) => session.owner_id == user.id.0,
        Err(_) => false,
    }
}

async fn authenticate(auth: &AuthStore, headers: &HeaderMap) -> Result<User, Response> {
    let token = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "));

    let Some(token) = token else {
        return Err(unauthorized());
    };

    auth.resolve_session(token).await.map_err(|e| match e {
        AuthError::InvalidSession => unauthorized(),
        other => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": other.to_string() })),
        )
            .into_response(),
    })
}

fn unauthorized() -> Response {
    let mut resp = (
        StatusCode::UNAUTHORIZED,
        Json(json!({ "error": "invalid or missing session token" })),
    )
        .into_response();
    resp.headers_mut()
        .insert(header::WWW_AUTHENTICATE, HeaderValue::from_static("Bearer"));
    resp
}

fn not_found() -> Response {
    (
        StatusCode::NOT_FOUND,
        Json(json!({ "error": "no such run" })),
    )
        .into_response()
}

fn afg_error_response(err: &AfgError) -> Response {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({ "error": err.to_string() })),
    )
        .into_response()
}
