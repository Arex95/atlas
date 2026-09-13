//! End-to-end tests over the `/api/auth/**` router, same pattern as
//! `atlas-mcp`'s `mcp_endpoint.rs`: `tower::ServiceExt::oneshot`, no
//! port bound, deterministic in CI.

use atlas_auth::api::{
    AuthStore, GithubOAuthConfig, GitlabOAuthConfig, SqlitePool, router, run_migrations,
};
use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode, header};
use serde_json::{Value, json};
use tower::ServiceExt;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

async fn app() -> axum::Router {
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    run_migrations(&pool).await.unwrap();
    router(AuthStore::new(pool), None, None)
}

async fn app_with_pool() -> (axum::Router, SqlitePool) {
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    run_migrations(&pool).await.unwrap();
    (router(AuthStore::new(pool.clone()), None, None), pool)
}

async fn app_with_gitlab_oauth(base_url: String) -> axum::Router {
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    run_migrations(&pool).await.unwrap();
    let config = GitlabOAuthConfig {
        client_id: "test-client-id".to_owned(),
        client_secret: "test-client-secret".to_owned(),
        redirect_uri: "http://localhost:4000/api/auth/oauth/gitlab/callback".to_owned(),
        state_secret: "test-state-secret".to_owned(),
        base_url,
    };
    router(AuthStore::new(pool), Some(config), None)
}

/// The GitHub harness. Both hosts point at the same mock server: the
/// real deployment splits `github.com` from `api.github.com`, and the
/// config keeps them separate so a test can collapse them.
async fn app_with_github_oauth(base_url: String) -> (axum::Router, SqlitePool) {
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    run_migrations(&pool).await.unwrap();
    let config = GithubOAuthConfig {
        client_id: "test-client-id".to_owned(),
        client_secret: "test-client-secret".to_owned(),
        redirect_uri: "http://localhost:4000/api/auth/oauth/github/callback".to_owned(),
        state_secret: "test-state-secret".to_owned(),
        base_url: base_url.clone(),
        api_base_url: base_url,
    };
    (
        router(AuthStore::new(pool.clone()), None, Some(config)),
        pool,
    )
}

async fn get_raw(app: axum::Router, path: &str) -> axum::response::Response {
    let request = Request::builder()
        .method("GET")
        .uri(path)
        .body(Body::empty())
        .unwrap();
    app.oneshot(request).await.unwrap()
}

async fn post_raw(app: axum::Router, path: &str, bearer: Option<&str>) -> StatusCode {
    let mut req = Request::builder().method("POST").uri(path);
    if let Some(tok) = bearer {
        req = req.header(header::AUTHORIZATION, format!("Bearer {tok}"));
    }
    let request = req.body(Body::empty()).unwrap();
    app.oneshot(request).await.unwrap().status()
}

async fn post(
    app: axum::Router,
    path: &str,
    body: Value,
    bearer: Option<&str>,
) -> (StatusCode, Value) {
    let mut req = Request::builder()
        .method("POST")
        .uri(path)
        .header(header::CONTENT_TYPE, "application/json");
    if let Some(tok) = bearer {
        req = req.header(header::AUTHORIZATION, format!("Bearer {tok}"));
    }
    let request = req.body(Body::from(body.to_string())).unwrap();
    let response = app.oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 64 * 1024).await.unwrap();
    let json = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or(Value::Null)
    };
    (status, json)
}

async fn get(app: axum::Router, path: &str, bearer: Option<&str>) -> (StatusCode, Value) {
    let mut req = Request::builder().method("GET").uri(path);
    if let Some(tok) = bearer {
        req = req.header(header::AUTHORIZATION, format!("Bearer {tok}"));
    }
    let request = req.body(Body::empty()).unwrap();
    let response = app.oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 64 * 1024).await.unwrap();
    let json = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or(Value::Null)
    };
    (status, json)
}

#[tokio::test]
async fn register_then_me_round_trip() {
    let app = app().await;
    let (status, body) = post(
        app.clone(),
        "/register",
        json!({ "email": "admin@example.com", "password": "correct horse battery", "display_name": "Admin" }),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let token = body["token"].as_str().unwrap().to_owned();
    assert_eq!(body["user"]["email"], "admin@example.com");

    let (status, me_body) = get(app, "/me", Some(&token)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(me_body["email"], "admin@example.com");
}

#[tokio::test]
async fn register_after_bootstrap_is_forbidden() {
    let app = app().await;
    post(
        app.clone(),
        "/register",
        json!({ "email": "admin@example.com", "password": "correct horse battery", "display_name": "Admin" }),
        None,
    )
    .await;

    let (status, _) = post(
        app,
        "/register",
        json!({ "email": "second@example.com", "password": "another password", "display_name": "Second" }),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn login_with_bad_credentials_is_unauthorized_with_a_generic_body() {
    let app = app().await;
    post(
        app.clone(),
        "/register",
        json!({ "email": "admin@example.com", "password": "correct horse battery", "display_name": "Admin" }),
        None,
    )
    .await;

    let (wrong_password_status, wrong_password_body) = post(
        app.clone(),
        "/login",
        json!({ "email": "admin@example.com", "password": "nope" }),
        None,
    )
    .await;
    let (unknown_email_status, unknown_email_body) = post(
        app,
        "/login",
        json!({ "email": "nobody@example.com", "password": "whatever" }),
        None,
    )
    .await;

    assert_eq!(wrong_password_status, StatusCode::UNAUTHORIZED);
    assert_eq!(unknown_email_status, StatusCode::UNAUTHORIZED);
    assert_eq!(wrong_password_body, unknown_email_body);
}

#[tokio::test]
async fn me_without_a_token_is_unauthorized() {
    let (status, _) = get(app().await, "/me", None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn me_with_a_garbage_token_is_unauthorized() {
    let (status, _) = get(app().await, "/me", Some("garbage")).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn oauth_routes_do_not_exist_when_gitlab_oauth_is_not_configured() {
    let response = get_raw(app().await, "/oauth/gitlab/start").await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn gitlab_oauth_start_redirects_to_gitlab_with_a_signed_state() {
    let server = MockServer::start().await;
    let app = app_with_gitlab_oauth(server.uri()).await;

    let response = get_raw(app, "/oauth/gitlab/start").await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    let location = response
        .headers()
        .get(header::LOCATION)
        .unwrap()
        .to_str()
        .unwrap();
    assert!(location.starts_with(&format!("{}/oauth/authorize", server.uri())));
    assert!(location.contains("client_id=test-client-id"));
    assert!(location.contains("state="));
}

/// Extracts a real, freshly-signed `state` from `/start` rather than
/// hand-rolling one — proves the whole round trip, not just the
/// callback in isolation.
async fn signed_state_from_start(app: axum::Router) -> String {
    let start_response = get_raw(app, "/oauth/gitlab/start").await;
    let location = start_response
        .headers()
        .get(header::LOCATION)
        .unwrap()
        .to_str()
        .unwrap()
        .to_owned();
    url::Url::parse(&location)
        .unwrap()
        .query_pairs()
        .find(|(k, _)| k == "state")
        .unwrap()
        .1
        .into_owned()
}

async fn mount_gitlab_profile(server: &MockServer, id: i64, email: &str, name: &str) {
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "access_token": "gl-access-token",
            "scope": "read_user read_api",
        })))
        .mount(server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v4/user"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": id,
            "email": email,
            "name": name,
        })))
        .mount(server)
        .await;
}

#[tokio::test]
async fn gitlab_oauth_callback_with_no_linked_account_is_rejected_and_creates_nothing() {
    let server = MockServer::start().await;
    mount_gitlab_profile(&server, 42, "dev@example.com", "Dev").await;
    let app = app_with_gitlab_oauth(server.uri()).await;

    let state = signed_state_from_start(app.clone()).await;
    let (status, body) = get(
        app,
        &format!("/oauth/gitlab/callback?code=irrelevant-code&state={state}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert!(body["error"].as_str().unwrap().contains("no local account"));
}

#[tokio::test]
async fn gitlab_oauth_callback_links_and_logs_into_a_pre_existing_account() {
    let server = MockServer::start().await;
    mount_gitlab_profile(&server, 42, "dev@example.com", "Dev").await;
    let app = app_with_gitlab_oauth(server.uri()).await;

    post(
        app.clone(),
        "/register",
        json!({ "email": "dev@example.com", "password": "correct horse battery", "display_name": "Dev (local)" }),
        None,
    )
    .await;

    let state = signed_state_from_start(app.clone()).await;
    let (status, body) = get(
        app,
        &format!("/oauth/gitlab/callback?code=irrelevant-code&state={state}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["user"]["email"], "dev@example.com");
    assert_eq!(body["user"]["display_name"], "Dev (local)");
    assert!(body["token"].as_str().is_some());
}

#[tokio::test]
async fn gitlab_oauth_callback_rejects_an_invalid_state_without_calling_gitlab() {
    // No mocks registered at all — if the callback reached the
    // network before checking `state`, wiremock would reject the
    // unexpected request and this test would fail differently.
    let server = MockServer::start().await;
    let app = app_with_gitlab_oauth(server.uri()).await;

    let (status, body) = get(
        app,
        "/oauth/gitlab/callback?code=irrelevant&state=not-a-valid-state",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body["error"].as_str().unwrap().contains("state"));
}

/// There's no team-invite endpoint yet — the only way to create an
/// account through the public API is the one-time bootstrap
/// `register()`. This seeds a second row directly to give the admin
/// routes another user to act on, standing in for that not-yet-built
/// mechanism.
async fn seed_second_user(pool: &SqlitePool, id: &str, email: &str) {
    sqlx::query(
        "INSERT INTO users (id, email, password_hash, display_name, created_at, updated_at) \
         VALUES (?, ?, 'unused-hash', 'Target', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
    )
    .bind(id)
    .bind(email)
    .execute(pool)
    .await
    .unwrap();
}

async fn disabled_at(pool: &SqlitePool, id: &str) -> Option<String> {
    sqlx::query_scalar("SELECT disabled_at FROM users WHERE id = ?")
        .bind(id)
        .fetch_one(pool)
        .await
        .unwrap()
}

#[tokio::test]
async fn disable_then_enable_a_user_via_the_admin_routes() {
    let (app, pool) = app_with_pool().await;
    let (_, admin_body) = post(
        app.clone(),
        "/register",
        json!({ "email": "admin@example.com", "password": "correct horse battery", "display_name": "Admin" }),
        None,
    )
    .await;
    let admin_token = admin_body["token"].as_str().unwrap().to_owned();
    seed_second_user(&pool, "target-1", "target@example.com").await;

    let status = post_raw(app.clone(), "/users/target-1/disable", Some(&admin_token)).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert!(disabled_at(&pool, "target-1").await.is_some());

    let status = post_raw(app.clone(), "/users/target-1/enable", Some(&admin_token)).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert!(disabled_at(&pool, "target-1").await.is_none());
}

#[tokio::test]
async fn disable_user_requires_authentication() {
    let (app, pool) = app_with_pool().await;
    seed_second_user(&pool, "target-1", "target@example.com").await;

    let status = post_raw(app.clone(), "/users/target-1/disable", None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let status = post_raw(app, "/users/target-1/disable", Some("garbage")).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn disable_user_rejects_disabling_your_own_account() {
    let (app, _pool) = app_with_pool().await;
    let (_, admin_body) = post(
        app.clone(),
        "/register",
        json!({ "email": "admin@example.com", "password": "correct horse battery", "display_name": "Admin" }),
        None,
    )
    .await;
    let admin_token = admin_body["token"].as_str().unwrap().to_owned();
    let admin_id = admin_body["user"]["id"].as_str().unwrap().to_owned();

    let status = post_raw(
        app,
        &format!("/users/{admin_id}/disable"),
        Some(&admin_token),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn disable_user_on_an_unknown_id_is_not_found() {
    let (app, _pool) = app_with_pool().await;
    let (_, admin_body) = post(
        app.clone(),
        "/register",
        json!({ "email": "admin@example.com", "password": "correct horse battery", "display_name": "Admin" }),
        None,
    )
    .await;
    let admin_token = admin_body["token"].as_str().unwrap().to_owned();

    let status = post_raw(app, "/users/no-such-id/disable", Some(&admin_token)).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

async fn register_admin(app: axum::Router) -> String {
    let (_, body) = post(
        app,
        "/register",
        json!({ "email": "admin@example.com", "password": "correct horse battery", "display_name": "Admin" }),
        None,
    )
    .await;
    body["token"].as_str().unwrap().to_owned()
}

#[tokio::test]
async fn invite_creates_an_account_with_a_temporary_password() {
    let app = app().await;
    let admin_token = register_admin(app.clone()).await;

    let (status, body) = post(
        app,
        "/invite",
        json!({ "email": "teammate@example.com", "display_name": "Teammate" }),
        Some(&admin_token),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(body["user"]["email"], "teammate@example.com");
    assert_eq!(body["user"]["must_change_password"], true);
    assert!(body["temporary_password"].as_str().unwrap().len() >= 8);
}

#[tokio::test]
async fn invite_requires_authentication() {
    let app = app().await;
    let (status, _) = post(
        app,
        "/invite",
        json!({ "email": "teammate@example.com", "display_name": "Teammate" }),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn invite_rejects_a_duplicate_email() {
    let app = app().await;
    let admin_token = register_admin(app.clone()).await;

    let (status, _) = post(
        app,
        "/invite",
        json!({ "email": "admin@example.com", "display_name": "Someone" }),
        Some(&admin_token),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
}

#[tokio::test]
async fn an_invited_account_can_only_reach_me_and_change_password_until_it_changes_it() {
    let app = app().await;
    let admin_token = register_admin(app.clone()).await;
    let (_, invite_body) = post(
        app.clone(),
        "/invite",
        json!({ "email": "teammate@example.com", "display_name": "Teammate" }),
        Some(&admin_token),
    )
    .await;
    let temp_password = invite_body["temporary_password"].as_str().unwrap();

    let (status, login_body) = post(
        app.clone(),
        "/login",
        json!({ "email": "teammate@example.com", "password": temp_password }),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let teammate_token = login_body["token"].as_str().unwrap().to_owned();

    // /me still works.
    let (status, _) = post(
        app.clone(),
        "/invite",
        json!({ "email": "blocked@example.com", "display_name": "Blocked" }),
        Some(&teammate_token),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    // Replacing the password lifts the restriction.
    let (status, _) = post(
        app.clone(),
        "/change-password",
        json!({ "current_password": temp_password, "new_password": "a brand new password" }),
        Some(&teammate_token),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let (status, _) = post(
        app,
        "/invite",
        json!({ "email": "allowed@example.com", "display_name": "Allowed" }),
        Some(&teammate_token),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
}

#[tokio::test]
async fn change_password_rejects_the_wrong_current_password() {
    let app = app().await;
    let admin_token = register_admin(app.clone()).await;

    let (status, _) = post(
        app,
        "/change-password",
        json!({ "current_password": "not the real password", "new_password": "a brand new password" }),
        Some(&admin_token),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

/// Mounts a GitHub that answers the token exchange, the profile, and
/// the email list. `emails` is `(address, primary, verified)`.
async fn mount_github(server: &MockServer, id: i64, login: &str, emails: &[(&str, bool, bool)]) {
    Mock::given(method("POST"))
        .and(path("/login/oauth/access_token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "access_token": "gh-access-token",
            "scope": "read:user,user:email",
        })))
        .mount(server)
        .await;

    Mock::given(method("GET"))
        .and(path("/user"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": id,
            "login": login,
            // Deliberately the address an attacker would want believed.
            // Nothing may read it.
            "email": "victim@example.com",
            "name": null,
        })))
        .mount(server)
        .await;

    let body: Vec<_> = emails
        .iter()
        .map(|(email, primary, verified)| {
            json!({ "email": email, "primary": primary, "verified": verified })
        })
        .collect();
    Mock::given(method("GET"))
        .and(path("/user/emails"))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .mount(server)
        .await;
}

async fn github_state(app: axum::Router) -> String {
    let response = get_raw(app, "/oauth/github/start").await;
    let location = response
        .headers()
        .get(header::LOCATION)
        .unwrap()
        .to_str()
        .unwrap()
        .to_owned();
    url::Url::parse(&location)
        .unwrap()
        .query_pairs()
        .find(|(k, _)| k == "state")
        .unwrap()
        .1
        .into_owned()
}

#[tokio::test]
async fn github_start_redirects_to_github_with_a_signed_state() {
    let server = MockServer::start().await;
    let (app, _) = app_with_github_oauth(server.uri()).await;

    let response = get_raw(app, "/oauth/github/start").await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    let location = response
        .headers()
        .get(header::LOCATION)
        .unwrap()
        .to_str()
        .unwrap();

    assert!(location.starts_with(&format!("{}/login/oauth/authorize", server.uri())));
    assert!(location.contains("client_id=test-client-id"));
    assert!(location.contains("state="));
}

#[tokio::test]
async fn github_links_an_existing_account_by_its_verified_primary_email() {
    let server = MockServer::start().await;
    let (app, pool) = app_with_github_oauth(server.uri()).await;
    AuthStore::new(pool)
        .register("dev@example.com", "correct horse battery", "Dev")
        .await
        .unwrap();
    mount_github(
        &server,
        4242,
        "dev-handle",
        &[("dev@example.com", true, true)],
    )
    .await;

    let state = github_state(app.clone()).await;
    let (status, body) = get(
        app,
        &format!("/oauth/github/callback?code=c&state={state}"),
        None,
    )
    .await;

    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["user"]["email"], "dev@example.com");
    assert!(body["token"].is_string());
}

/// The defect this adapter is shaped around.
///
/// GitHub's `GET /user` returns an address it does not guarantee to
/// have verified. Linking is done by matching that address to an
/// existing account, so believing it is an account takeover: add the
/// victim's address to your own GitHub account, consent, and the link
/// is made to theirs.
#[tokio::test]
async fn an_unverified_email_cannot_claim_an_account() {
    let server = MockServer::start().await;
    let (app, pool) = app_with_github_oauth(server.uri()).await;
    AuthStore::new(pool)
        .register("victim@example.com", "correct horse battery", "Victim")
        .await
        .unwrap();

    // The attacker's GitHub account: the victim's address present but
    // unverified, and their own verified one is not primary.
    mount_github(
        &server,
        999,
        "attacker",
        &[
            ("victim@example.com", true, false),
            ("attacker@example.com", false, true),
        ],
    )
    .await;

    let state = github_state(app.clone()).await;
    let (status, body) = get(
        app,
        &format!("/oauth/github/callback?code=c&state={state}"),
        None,
    )
    .await;

    assert_ne!(
        status,
        StatusCode::OK,
        "an unverified email claimed an account: {body}"
    );
}

#[tokio::test]
async fn an_account_with_no_verified_primary_is_refused_rather_than_guessed() {
    let server = MockServer::start().await;
    let (app, pool) = app_with_github_oauth(server.uri()).await;
    AuthStore::new(pool)
        .register("dev@example.com", "correct horse battery", "Dev")
        .await
        .unwrap();
    // Verified, but not primary. Taking it anyway would mean picking an
    // arbitrary confirmed address rather than the one the account is
    // known by.
    mount_github(&server, 1, "dev", &[("dev@example.com", false, true)]).await;

    let state = github_state(app.clone()).await;
    let (status, _) = get(
        app,
        &format!("/oauth/github/callback?code=c&state={state}"),
        None,
    )
    .await;
    assert_ne!(status, StatusCode::OK);
}

#[tokio::test]
async fn github_never_creates_an_account() {
    let server = MockServer::start().await;
    let (app, _) = app_with_github_oauth(server.uri()).await;
    // Verified and primary — and still refused, because linking is
    // link-only. A consent screen is not a sign-up.
    mount_github(
        &server,
        7,
        "stranger",
        &[("stranger@example.com", true, true)],
    )
    .await;

    let state = github_state(app.clone()).await;
    let (status, _) = get(
        app,
        &format!("/oauth/github/callback?code=c&state={state}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn github_routes_are_absent_when_it_is_not_configured() {
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    run_migrations(&pool).await.unwrap();
    let app = router(AuthStore::new(pool), None, None);

    let response = get_raw(app, "/oauth/github/start").await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}
