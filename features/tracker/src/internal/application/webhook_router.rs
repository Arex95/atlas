//! `POST /api/webhooks/tracker/gitlab` — the HTTP side of the webhook.
//!
//! Four jobs and only four: authenticate the sender, work out which
//! issue the event is about, refresh that one issue, acknowledge.
//!
//! **The response never waits on the upstream fetch.** GitLab expects
//! a prompt answer and disables a hook that times out, so the refresh
//! runs off the response path and its failures are logged rather than
//! returned. There is nothing useful to tell the tracker about a
//! failure on our side anyway — it cannot fix it, and a non-2xx would
//! teach it to stop sending.
//!
//! That the mirror might therefore miss one refresh is acceptable, and
//! is why the polling loop stays: a webhook shortens the gap, it does
//! not become the only way the mirror is correct.

use std::sync::Arc;

use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use serde_json::json;

use super::webhook::{changed_issue, refresh_issue, token_matches};
use crate::internal::domain::IssueTracker;
use crate::internal::infrastructure::mirror::MirrorStore;

/// The header GitLab echoes back with the secret chosen when the hook
/// was registered.
const TOKEN_HEADER: &str = "x-gitlab-token";

#[derive(Clone)]
struct WebhookState {
    upstream: Arc<dyn IssueTracker>,
    store: MirrorStore,
    secret: String,
}

/// Mounts the receiver, or nothing at all when no secret is set.
///
/// An unauthenticated webhook endpoint is a way for anyone to make
/// this server fetch from the tracker on demand, so a deployment that
/// configured no secret gets no route rather than an open one —
/// `404`, the same posture the OAuth routes take.
pub fn webhook_router(
    upstream: Arc<dyn IssueTracker>,
    store: MirrorStore,
    secret: Option<String>,
) -> Router {
    let Some(secret) = secret.filter(|s| !s.trim().is_empty()) else {
        return Router::new();
    };
    Router::new()
        .route("/gitlab", post(receive_gitlab))
        .with_state(Arc::new(WebhookState {
            upstream,
            store,
            secret,
        }))
}

async fn receive_gitlab(
    State(state): State<Arc<WebhookState>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let provided = headers.get(TOKEN_HEADER).and_then(|v| v.to_str().ok());
    if !token_matches(provided, &state.secret) {
        // Nothing is read and nothing is stored. An unauthenticated
        // event is not ours to act on, and acting on it would let
        // anyone drive requests to the tracker through us.
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({ "error": "token does not match" })),
        )
            .into_response();
    }

    let Ok(text) = std::str::from_utf8(&body) else {
        return accepted(0);
    };
    let Some(changed) = changed_issue(text) else {
        // An event this does not handle — a push, a comment, a
        // malformed body. Acknowledged rather than refused: a non-2xx
        // teaches GitLab to disable the hook, and there is nothing
        // wrong on its side.
        return accepted(0);
    };

    let state = Arc::clone(&state);
    tokio::spawn(async move {
        if let Err(e) = refresh_issue(&state.upstream, &state.store, &changed).await {
            // Logged, not returned. The tracker cannot act on this,
            // and the polling loop will pick the issue up regardless.
            tracing::warn!(
                project = %changed.project,
                issue = %changed.id,
                error = %e,
                "webhook: refresh failed; the polling loop will catch it"
            );
        }
    });

    accepted(1)
}

/// Always 200, with how many issues the event scheduled a refresh for.
///
/// A count rather than a bare acknowledgement so that a delivery log
/// on the tracker's side distinguishes "received and acted on" from
/// "received and ignored", which is otherwise invisible.
fn accepted(scheduled: u8) -> Response {
    (StatusCode::OK, Json(json!({ "scheduled": scheduled }))).into_response()
}
