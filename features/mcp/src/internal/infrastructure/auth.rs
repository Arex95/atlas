//! Bearer-token auth middleware for the MCP endpoint.
//!
//! Two credentials are accepted, and the difference between them is
//! the identity they carry:
//!
//! * A **session token**, minted when a session is created. Resolves
//!   to `Caller::Session` — a session, and through it an owner.
//! * The **shared token** from `ATLAS_MCP_TOKEN`. Resolves to
//!   `Caller::Local`, which owns nothing on a team server.
//!
//! The resolved caller is put into the request extensions, and every
//! tool that touches personal state reads its owner from there. It
//! used to come from an `owner_id` argument the caller wrote itself,
//! which meant anyone holding the shared token could name another
//! developer and read, list or delete their personal state.
//!
//! The shared token is compared in constant time
//! (`subtle::ConstantTimeEq`). The session token is not, and does not
//! need to be: it is looked up by SHA-256 hash, so a timing
//! difference leaks something about the *hash* of a guess, and
//! inverting that to recover a 256-bit token is the thing SHA-256
//! exists to prevent.
//!
//! A missing header, or a value matching neither credential, yields
//! HTTP 401 with a `WWW-Authenticate: Bearer` challenge.

use atlas_sessions::api::{Caller, SessionStore};
use axum::body::Body;
use axum::extract::State;
use axum::http::{HeaderValue, Request, StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use subtle::ConstantTimeEq;

/// Wrapper so the token cannot be accidentally logged with `{:?}`
/// or `{}` — Debug is opaque, Display is not implemented.
#[derive(Clone)]
pub struct McpToken(String);

impl McpToken {
    #[must_use]
    pub fn new(raw: String) -> Self {
        Self(raw)
    }

    #[must_use]
    fn matches(&self, candidate: &str) -> bool {
        self.0.as_bytes().ct_eq(candidate.as_bytes()).into()
    }
}

impl std::fmt::Debug for McpToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("McpToken(<redacted>)")
    }
}

/// What the middleware needs to resolve a bearer.
#[derive(Clone)]
pub struct AuthContext {
    pub token: McpToken,
    pub sessions: SessionStore,
}

pub async fn bearer_auth(
    State(context): State<AuthContext>,
    mut request: Request<Body>,
    next: Next,
) -> Response {
    let candidate = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .map(ToOwned::to_owned);

    let Some(bearer) = candidate else {
        return unauthorized();
    };

    // The shared token first: it is a constant-time comparison against
    // one value, where the session lookup is a database round trip.
    let caller = if context.token.matches(&bearer) {
        Caller::Local
    } else {
        match context.sessions.resolve_token(&bearer).await {
            Ok(Some(caller)) => caller,
            Ok(None) => return unauthorized(),
            Err(e) => {
                // A storage failure is not an authentication failure,
                // and reporting it as one sends whoever debugs it to
                // check their token.
                tracing::error!(error = %e, "could not resolve a session token");
                return (StatusCode::SERVICE_UNAVAILABLE, "auth store unavailable").into_response();
            }
        }
    };

    // Extensions rather than a header: this is decided here and must
    // not be settable by the client.
    request.extensions_mut().insert(caller);
    next.run(request).await
}

fn unauthorized() -> Response {
    let mut resp = (StatusCode::UNAUTHORIZED, "unauthorized").into_response();
    resp.headers_mut()
        .insert(header::WWW_AUTHENTICATE, HeaderValue::from_static("Bearer"));
    resp
}
