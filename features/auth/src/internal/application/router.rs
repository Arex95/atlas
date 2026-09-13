//! Axum wiring for `/api/auth/**` — human/dashboard authentication,
//! orthogonal to `ATLAS_MCP_TOKEN` (the agent-facing MCP bearer).
//!
//! Mounted by the binary at `/api/auth`, so the effective paths are
//! `POST /api/auth/register`, `POST /api/auth/login`,
//! `GET /api/auth/me`, `POST /api/auth/invite`,
//! `POST /api/auth/change-password`,
//! `POST /api/auth/users/{id}/disable`,
//! `POST /api/auth/users/{id}/enable`, and — only when GitLab OAuth
//! is configured — `GET /api/auth/oauth/gitlab/start` /
//! `GET .../oauth/gitlab/callback`.
//!
//! There is no separate admin role (single-tenant): any
//! caller holding a valid session bearer may disable or enable any
//! other account, or invite a new one. A caller may not disable the
//! account they're authenticated as — see
//! `AuthError::CannotDisableSelf`. `invite` is the only way to create
//! an account after the one-time bootstrap (`register`) — GitLab
//! OAuth deliberately never creates one — and hands back a
//! server-generated temporary password the invitee must replace via
//! `change-password` before any route but `/me` will serve them; see
//! `AuthError::PasswordChangeRequired`.

use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::Utc;
use serde::Deserialize;
use serde_json::json;

use crate::internal::domain::OAuthProvider;
use crate::internal::domain::{AuthError, InvitedAccount, IssuedSession, User};
use crate::internal::infrastructure::{
    GithubOAuthClient, GithubOAuthConfig, GitlabOAuthClient, GitlabOAuthConfig,
};

use super::AuthStore;

struct AppState {
    store: AuthStore,
    gitlab: Option<GitlabOAuthClient>,
    github: Option<GithubOAuthClient>,
}

/// `gitlab_oauth: None` mounts only `/register`, `/login`, `/me` —
/// the OAuth routes don't exist at all, rather than existing and
/// returning some "not configured" error, so a Mode 1 install that
/// never touches Mode 2 has nothing extra to probe.
pub fn router(
    store: AuthStore,
    gitlab_oauth: Option<GitlabOAuthConfig>,
    github_oauth: Option<GithubOAuthConfig>,
) -> Router {
    let state = Arc::new(AppState {
        store,
        gitlab: gitlab_oauth.map(GitlabOAuthClient::new),
        github: github_oauth.map(GithubOAuthClient::new),
    });
    let mut router = Router::new()
        .route("/register", post(register))
        .route("/login", post(login))
        .route("/me", get(me))
        .route("/invite", post(invite))
        .route("/change-password", post(change_password))
        .route("/users/{id}/disable", post(disable_user))
        .route("/users/{id}/enable", post(enable_user));
    if state.github.is_some() {
        router = router
            .route("/oauth/github/start", get(github_oauth_start))
            .route("/oauth/github/callback", get(github_oauth_callback));
    }
    if state.gitlab.is_some() {
        router = router
            .route("/oauth/gitlab/start", get(gitlab_oauth_start))
            .route("/oauth/gitlab/callback", get(gitlab_oauth_callback));
    }
    router.with_state(state)
}

#[derive(Deserialize)]
struct RegisterBody {
    email: String,
    password: String,
    display_name: String,
}

#[derive(Deserialize)]
struct LoginBody {
    email: String,
    password: String,
}

async fn register(State(state): State<Arc<AppState>>, Json(body): Json<RegisterBody>) -> Response {
    match state
        .store
        .register(&body.email, &body.password, &body.display_name)
        .await
    {
        Ok(session) => (StatusCode::CREATED, Json(session_response(&session))).into_response(),
        Err(e) => error_response(&e),
    }
}

async fn login(State(state): State<Arc<AppState>>, Json(body): Json<LoginBody>) -> Response {
    match state.store.login(&body.email, &body.password).await {
        Ok(session) => (StatusCode::OK, Json(session_response(&session))).into_response(),
        Err(e) => error_response(&e),
    }
}

async fn me(State(state): State<Arc<AppState>>, headers: HeaderMap) -> Response {
    let Some(token) = bearer_token(&headers) else {
        return unauthorized();
    };
    match authenticate(&state, token).await {
        Ok(user) => (StatusCode::OK, Json(user_response(&user))).into_response(),
        Err(()) => unauthorized(),
    }
}

#[derive(Deserialize)]
struct InviteBody {
    email: String,
    display_name: String,
}

async fn invite(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(body): Json<InviteBody>,
) -> Response {
    let Some(token) = bearer_token(&headers) else {
        return unauthorized();
    };
    let Ok(caller) = authenticate(&state, token).await else {
        return unauthorized();
    };
    if !password_already_changed(&caller) {
        return error_response(&AuthError::PasswordChangeRequired);
    }
    match state
        .store
        .invite_user(&body.email, &body.display_name)
        .await
    {
        Ok(invited) => (StatusCode::CREATED, Json(invited_response(&invited))).into_response(),
        Err(e) => error_response(&e),
    }
}

#[derive(Deserialize)]
struct ChangePasswordBody {
    current_password: String,
    new_password: String,
}

/// Deliberately exempt from `password_already_changed` — an
/// invited account with a temporary password needs exactly this
/// route to stop being restricted.
async fn change_password(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(body): Json<ChangePasswordBody>,
) -> Response {
    let Some(token) = bearer_token(&headers) else {
        return unauthorized();
    };
    let Ok(caller) = authenticate(&state, token).await else {
        return unauthorized();
    };
    match state
        .store
        .change_password(&caller.id, &body.current_password, &body.new_password)
        .await
    {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => error_response(&e),
    }
}

async fn disable_user(
    State(state): State<Arc<AppState>>,
    Path(target_id): Path<String>,
    headers: HeaderMap,
) -> Response {
    let Some(token) = bearer_token(&headers) else {
        return unauthorized();
    };
    let Ok(caller) = authenticate(&state, token).await else {
        return unauthorized();
    };
    if !password_already_changed(&caller) {
        return error_response(&AuthError::PasswordChangeRequired);
    }
    match state.store.disable_user(&target_id, &caller.id).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => error_response(&e),
    }
}

async fn enable_user(
    State(state): State<Arc<AppState>>,
    Path(target_id): Path<String>,
    headers: HeaderMap,
) -> Response {
    let Some(token) = bearer_token(&headers) else {
        return unauthorized();
    };
    let Ok(caller) = authenticate(&state, token).await else {
        return unauthorized();
    };
    if !password_already_changed(&caller) {
        return error_response(&AuthError::PasswordChangeRequired);
    }
    match state.store.enable_user(&target_id).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => error_response(&e),
    }
}

fn bearer_token(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
}

/// Shared session resolution for every route that requires an
/// authenticated caller.
async fn authenticate(state: &AppState, token: &str) -> Result<User, ()> {
    state.store.resolve_session(token).await.map_err(|_| ())
}

/// Blocks every route but `/me` and `/change-password` until an
/// invited account has replaced its server-generated temporary
/// password.
fn password_already_changed(user: &User) -> bool {
    !user.must_change_password
}

#[derive(Deserialize)]
struct OAuthCallbackQuery {
    code: String,
    state: String,
}

async fn gitlab_oauth_start(State(state): State<Arc<AppState>>) -> Response {
    let Some(client) = &state.gitlab else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let signed_state =
        crate::internal::infrastructure::sign_oauth_state(client.state_secret(), Utc::now());
    Redirect::to(client.authorize_url(&signed_state).as_str()).into_response()
}

async fn github_oauth_start(State(state): State<Arc<AppState>>) -> Response {
    let Some(client) = &state.github else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let signed_state =
        crate::internal::infrastructure::sign_oauth_state(client.state_secret(), Utc::now());
    Redirect::to(client.authorize_url(&signed_state).as_str()).into_response()
}

async fn github_oauth_callback(
    State(state): State<Arc<AppState>>,
    Query(query): Query<OAuthCallbackQuery>,
) -> Response {
    let Some(client) = &state.github else {
        return StatusCode::NOT_FOUND.into_response();
    };

    // Before any network call: an unverified `state` means this
    // callback was not started here, and acting on it would let a
    // third party spend our client credentials.
    if let Err(e) = crate::internal::infrastructure::verify_oauth_state(
        client.state_secret(),
        &query.state,
        Utc::now(),
    ) {
        return error_response(&e);
    }

    let (access_token, scopes) = match client.exchange_code(&query.code).await {
        Ok(v) => v,
        Err(e) => return error_response(&e),
    };
    // Used to ask the provider who this is, then dropped. It is not
    // stored: see the migration that removed the column.
    let profile = match client.fetch_profile(&access_token).await {
        Ok(p) => p,
        Err(e) => return error_response(&e),
    };

    match state
        .store
        .login_via_oauth(OAuthProvider::Github, profile, &scopes)
        .await
    {
        Ok(session) => (StatusCode::OK, Json(session_response(&session))).into_response(),
        Err(e) => error_response(&e),
    }
}

async fn gitlab_oauth_callback(
    State(state): State<Arc<AppState>>,
    Query(query): Query<OAuthCallbackQuery>,
) -> Response {
    let Some(client) = &state.gitlab else {
        return StatusCode::NOT_FOUND.into_response();
    };

    if let Err(e) = crate::internal::infrastructure::verify_oauth_state(
        client.state_secret(),
        &query.state,
        Utc::now(),
    ) {
        return error_response(&e);
    }

    let (access_token, scopes) = match client.exchange_code(&query.code).await {
        Ok(v) => v,
        Err(e) => return error_response(&e),
    };
    // Used to ask the provider who this is, then dropped. It is not
    // stored: see the migration that removed the column.
    let profile = match client.fetch_profile(&access_token).await {
        Ok(p) => p,
        Err(e) => return error_response(&e),
    };

    match state
        .store
        .login_via_oauth(OAuthProvider::Gitlab, profile, &scopes)
        .await
    {
        Ok(session) => (StatusCode::OK, Json(session_response(&session))).into_response(),
        Err(e) => error_response(&e),
    }
}

fn session_response(session: &IssuedSession) -> serde_json::Value {
    json!({
        "token": session.token,
        "expires_at": session.expires_at,
        "user": user_response(&session.user),
    })
}

fn user_response(user: &User) -> serde_json::Value {
    json!({
        "id": user.id.0,
        "email": user.email,
        "display_name": user.display_name,
        "created_at": user.created_at,
        "updated_at": user.updated_at,
        "must_change_password": user.must_change_password,
    })
}

fn invited_response(invited: &InvitedAccount) -> serde_json::Value {
    json!({
        "user": user_response(&invited.user),
        "temporary_password": invited.temporary_password,
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

fn error_response(err: &AuthError) -> Response {
    match err {
        AuthError::AlreadyBootstrapped
        | AuthError::OAuthAccountNotLinked
        | AuthError::PasswordChangeRequired => (
            StatusCode::FORBIDDEN,
            Json(json!({ "error": err.to_string() })),
        )
            .into_response(),
        AuthError::InvalidEmail
        | AuthError::PasswordTooShort
        | AuthError::OAuthStateInvalid
        | AuthError::CannotDisableSelf => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": err.to_string() })),
        )
            .into_response(),
        AuthError::InvalidCredentials | AuthError::InvalidSession => unauthorized(),
        AuthError::OAuthProviderError(_) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({ "error": "the OAuth provider is unavailable or rejected the request" })),
        )
            .into_response(),
        AuthError::UserNotFound => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": err.to_string() })),
        )
            .into_response(),
        AuthError::EmailAlreadyRegistered => (
            StatusCode::CONFLICT,
            Json(json!({ "error": err.to_string() })),
        )
            .into_response(),
        AuthError::Storage(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": "internal error" })),
        )
            .into_response(),
    }
}
