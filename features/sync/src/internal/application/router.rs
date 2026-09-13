//! Axum wiring for `/api/sync/**` — server side of every sync
//! mode. Authenticated with an `atlas-auth` bearer session token (the
//! same identity `/api/auth/me` resolves), never the shared
//! `ATLAS_MCP_TOKEN` — a sync client is acting as a specific human,
//! not as an agent.
//!
//! Mounted by the binary at `/api/sync`, so the effective paths are
//! `POST /api/sync/sessions/push`, `GET /api/sync/sessions/pull`,
//! `POST /api/sync/memory/push`, `GET /api/sync/memory/pull`,
//! `POST /api/sync/notes/push`, `GET /api/sync/notes/pull`, and
//! `GET /api/sync/events` (the `live`-mode SSE stream).
//!
//! The two memory routes carry both of buckets in one
//! payload, and treat them differently on purpose: project entries
//! (Type 1) replicate freely, while personal ones (Type 2) are forced
//! under the authenticated caller's own owner id. A client
//! cannot push into someone else's personal memory, or pull it.
//!
//! Notes are Type 2 throughout, so their two routes have no bucket to
//! route between — every note on both legs is stored under the
//! authenticated caller, never the owner the row claims.

use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;

use atlas_auth::api::{AuthError, AuthStore, User};
use atlas_memory::api::{MemoryEntry, MemoryError, MemoryScope, MemoryStore};
use atlas_notes::api::{Note, NoteStore, NotesError};
use atlas_sessions::api::{Session, SessionStore, SessionsError};
use axum::extract::{Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::json;
use tokio_stream::StreamExt;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::wrappers::errors::BroadcastStreamRecvError;

use crate::internal::domain::{Audience, ChangeKind};
use crate::internal::infrastructure::ChangeHub;

/// How often an idle stream emits a comment so proxies and NAT
/// tables don't reap a connection that is merely quiet.
const KEEP_ALIVE_SECS: u64 = 15;

struct SyncState {
    auth: AuthStore,
    sessions: SessionStore,
    memory: MemoryStore,
    notes: NoteStore,
    hub: ChangeHub,
}

pub fn router(
    auth: AuthStore,
    sessions: SessionStore,
    memory: MemoryStore,
    notes: NoteStore,
) -> Router {
    // The hub is created here rather than passed in because nothing
    // outside this router touches it: the only publisher is the push
    // handler below, and the only subscribers are its own streams.
    let hub = ChangeHub::new();
    Router::new()
        .route("/sessions/push", post(push_sessions))
        .route("/sessions/pull", get(pull_sessions))
        .route("/memory/push", post(push_memory))
        .route("/memory/pull", get(pull_memory))
        .route("/notes/push", post(push_notes))
        .route("/notes/pull", get(pull_notes))
        .route("/events", get(events))
        .with_state(Arc::new(SyncState {
            auth,
            sessions,
            memory,
            notes,
            hub,
        }))
}

#[derive(Deserialize)]
struct PushBody {
    sessions: Vec<Session>,
}

#[derive(Deserialize)]
struct PullQuery {
    since: Option<String>,
}

async fn push_sessions(
    State(state): State<Arc<SyncState>>,
    headers: HeaderMap,
    Json(body): Json<PushBody>,
) -> Response {
    let user = match authenticate(&state.auth, &headers).await {
        Ok(user) => user,
        Err(resp) => return resp,
    };

    let mut results = Vec::with_capacity(body.sessions.len());
    let mut moved = false;
    for incoming in body.sessions {
        match state.sessions.upsert_for_sync(incoming, &user.id.0).await {
            Ok((session, changed)) => {
                moved |= changed;
                results.push(session);
            }
            Err(e) => return sessions_error_response(&e),
        }
    }

    // One notification per push, not per session: a `live` subscriber
    // answers any notification with a full pass, so telling it five
    // times that five sessions moved would buy five identical passes.
    //
    // And only when something *moved*. A push whose rows all lost
    // last-write-wins changed nothing, and announcing it anyway is what
    // made `live` spin: the subscriber answered with a pass, the pass
    // pushed the same rows again, the push announced again. Measured at
    // 256 events a second between two idle machines.
    if moved {
        state.hub.publish(&user.id.0, ChangeKind::Sessions);
    }

    (StatusCode::OK, Json(json!({ "sessions": results }))).into_response()
}

#[derive(Deserialize)]
struct MemoryPushBody {
    entries: Vec<MemoryEntry>,
}

/// Applies incoming memory, each entry routed by its own scope.
///
/// Project entries go through as-is; personal ones are forced under
/// the caller's own owner id, so a client that lies about `owner_id`
/// writes to its own bucket rather than being refused — there is no
/// path here that touches another developer's personal memory at all.
async fn push_memory(
    State(state): State<Arc<SyncState>>,
    headers: HeaderMap,
    Json(body): Json<MemoryPushBody>,
) -> Response {
    let user = match authenticate(&state.auth, &headers).await {
        Ok(user) => user,
        Err(resp) => return resp,
    };

    let mut results = Vec::with_capacity(body.entries.len());
    let mut moved_personal = false;
    let mut moved_project = false;
    for incoming in &body.entries {
        let applied = match incoming.scope {
            MemoryScope::Project => state.memory.upsert_project_for_sync(incoming).await,
            MemoryScope::Personal => {
                state
                    .memory
                    .upsert_personal_for_sync(incoming, &user.id.0)
                    .await
            }
        };
        match applied {
            Ok((entry, changed)) => {
                match incoming.scope {
                    MemoryScope::Personal => moved_personal |= changed,
                    MemoryScope::Project => moved_project |= changed,
                }
                results.push(entry);
            }
            Err(e) => return memory_error_response(&e),
        }
    }

    // Two audiences, because the two scopes have two.
    //
    // Personal memory is one developer's event. Project memory is
    // everyone's — and until this was here, it reached nobody in `live`
    // mode: the channel was addressed by owner and Type 1 state has no
    // owner, so the shared thing was the one that did not propagate.
    if moved_personal {
        state.hub.publish(&user.id.0, ChangeKind::Memory);
    }
    if moved_project {
        state.hub.publish_to(Audience::Everyone, ChangeKind::Memory);
    }

    (StatusCode::OK, Json(json!({ "entries": results }))).into_response()
}

/// Returns all project memory plus the caller's own personal memory —
/// never anyone else's.
async fn pull_memory(
    State(state): State<Arc<SyncState>>,
    headers: HeaderMap,
    Query(query): Query<PullQuery>,
) -> Response {
    let user = match authenticate(&state.auth, &headers).await {
        Ok(user) => user,
        Err(resp) => return resp,
    };

    let since = match query.since.as_deref().map(parse_cursor).transpose() {
        Ok(since) => since,
        Err(msg) => {
            return (StatusCode::BAD_REQUEST, Json(json!({ "error": msg }))).into_response();
        }
    };

    let mut entries = match state.memory.list_project_since(since).await {
        Ok(entries) => entries,
        Err(e) => return memory_error_response(&e),
    };
    match state.memory.list_personal_since(&user.id.0, since).await {
        Ok(mine) => entries.extend(mine),
        Err(e) => return memory_error_response(&e),
    }

    (StatusCode::OK, Json(json!({ "entries": entries }))).into_response()
}

#[derive(Deserialize)]
struct NotesPushBody {
    notes: Vec<Note>,
}

/// Applies incoming notes under the caller's own owner id.
///
/// A client that claims another owner writes into its own notes rather
/// than being refused: there is no path here that touches a different
/// developer's, so there is nothing to refuse *from*.
async fn push_notes(
    State(state): State<Arc<SyncState>>,
    headers: HeaderMap,
    Json(body): Json<NotesPushBody>,
) -> Response {
    let user = match authenticate(&state.auth, &headers).await {
        Ok(user) => user,
        Err(resp) => return resp,
    };

    let mut results = Vec::with_capacity(body.notes.len());
    let mut moved = false;
    for incoming in body.notes {
        match state.notes.upsert_for_sync(incoming, &user.id.0).await {
            Ok((note, changed)) => {
                moved |= changed;
                results.push(note);
            }
            Err(e) => return notes_error_response(&e),
        }
    }

    // One notification per push, not per note: a `live` subscriber
    // answers any notification with a full pass.
    // Only when something moved — see push_sessions.
    if moved {
        state.hub.publish(&user.id.0, ChangeKind::Notes);
    }

    (StatusCode::OK, Json(json!({ "notes": results }))).into_response()
}

/// Returns the caller's own notes and nobody else's.
async fn pull_notes(
    State(state): State<Arc<SyncState>>,
    headers: HeaderMap,
    Query(query): Query<PullQuery>,
) -> Response {
    let user = match authenticate(&state.auth, &headers).await {
        Ok(user) => user,
        Err(resp) => return resp,
    };

    let since = match query.since.as_deref().map(parse_cursor).transpose() {
        Ok(since) => since,
        Err(msg) => {
            return (StatusCode::BAD_REQUEST, Json(json!({ "error": msg }))).into_response();
        }
    };

    match state.notes.list_since(&user.id.0, since).await {
        Ok(notes) => (StatusCode::OK, Json(json!({ "notes": notes }))).into_response(),
        Err(e) => notes_error_response(&e),
    }
}

fn notes_error_response(err: &NotesError) -> Response {
    let status = match err {
        NotesError::EmptyName | NotesError::NameTooLong(_) | NotesError::EmptyOwnerId => {
            StatusCode::BAD_REQUEST
        }
        NotesError::NotFound => StatusCode::NOT_FOUND,
        NotesError::Storage(_) => StatusCode::INTERNAL_SERVER_ERROR,
    };
    (status, Json(json!({ "error": err.to_string() }))).into_response()
}

fn memory_error_response(err: &MemoryError) -> Response {
    match err {
        MemoryError::EmptyKey | MemoryError::EmptyProject | MemoryError::EmptyOwner => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": err.to_string() })),
        )
            .into_response(),
        MemoryError::NotFound => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": err.to_string() })),
        )
            .into_response(),
        MemoryError::Corrupt(_) | MemoryError::Storage(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": "internal error" })),
        )
            .into_response(),
    }
}

/// The `live`-mode stream. Emits a notification — never
/// state — when something belonging to the authenticated caller is
/// upserted, and nothing at all when it belongs to anybody else.
async fn events(State(state): State<Arc<SyncState>>, headers: HeaderMap) -> Response {
    let user = match authenticate(&state.auth, &headers).await {
        Ok(user) => user,
        Err(resp) => return resp,
    };
    let owner_id = user.id.0;

    let stream = BroadcastStream::new(state.hub.subscribe()).filter_map(move |item| match item {
        // Addressed to this developer, or to everyone because it is
        // Type 1 state that is shared by design.
        Ok(change)
            if matches!(&change.audience, Audience::Owner(id) if *id == owner_id)
                || change.audience == Audience::Everyone =>
        {
            Some(Ok::<Event, Infallible>(
                Event::default().event("change").data(change.kind.as_str()),
            ))
        }
        Ok(_) => None,
        // This subscriber fell behind and missed notifications. Not
        // worth closing the stream over, and not worth reporting
        // precisely: one nudge makes it pull, and that pull collects
        // everything it missed in a single pass.
        Err(BroadcastStreamRecvError::Lagged(_)) => Some(Ok(Event::default()
            .event("change")
            .data(ChangeKind::Sessions.as_str()))),
    });

    Sse::new(stream)
        .keep_alive(KeepAlive::default().interval(Duration::from_secs(KEEP_ALIVE_SECS)))
        .into_response()
}

async fn pull_sessions(
    State(state): State<Arc<SyncState>>,
    headers: HeaderMap,
    Query(query): Query<PullQuery>,
) -> Response {
    let user = match authenticate(&state.auth, &headers).await {
        Ok(user) => user,
        Err(resp) => return resp,
    };

    let since = match query.since.as_deref().map(parse_cursor).transpose() {
        Ok(since) => since,
        Err(msg) => {
            return (StatusCode::BAD_REQUEST, Json(json!({ "error": msg }))).into_response();
        }
    };

    match state.sessions.list_since(&user.id.0, since).await {
        Ok(sessions) => (StatusCode::OK, Json(json!({ "sessions": sessions }))).into_response(),
        Err(e) => sessions_error_response(&e),
    }
}

fn parse_cursor(raw: &str) -> Result<DateTime<Utc>, String> {
    DateTime::parse_from_rfc3339(raw)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|e| format!("since must be RFC3339: {e}"))
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

fn sessions_error_response(err: &SessionsError) -> Response {
    match err {
        SessionsError::EmptyProject | SessionsError::EmptyRemoteUrl => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": err.to_string() })),
        )
            .into_response(),
        SessionsError::OwnerMismatch => (
            StatusCode::CONFLICT,
            Json(json!({ "error": err.to_string() })),
        )
            .into_response(),
        _ => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": "internal error" })),
        )
            .into_response(),
    }
}
