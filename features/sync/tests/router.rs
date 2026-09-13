//! End-to-end tests over `/api/sync/**`, same `tower::ServiceExt::oneshot`
//! pattern as `atlas-mcp`'s `mcp_endpoint.rs` and `atlas-auth`'s
//! `router.rs`.

use atlas_auth::api::{AuthStore, SqlitePool as AuthPool, run_migrations as run_auth_migrations};
use atlas_sessions::api::{NewSession, SessionStore, run_migrations as run_sessions_migrations};
use atlas_sync::api::router;
use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode, header};
use serde_json::{Value, json};
use tower::ServiceExt;

async fn fresh_note_store() -> atlas_notes::api::NoteStore {
    let pool = atlas_notes::api::SqlitePool::connect("sqlite::memory:")
        .await
        .unwrap();
    atlas_notes::api::run_migrations(&pool).await.unwrap();
    atlas_notes::api::NoteStore::new(pool)
}

/// A memory store for the sync engine to carry alongside sessions.
/// Its own in-memory database: memory rows share no tables with
/// sessions, so nothing here needs the same pool.
async fn fresh_memory_store() -> atlas_memory::api::MemoryStore {
    let pool = atlas_memory::api::SqlitePool::connect("sqlite::memory:")
        .await
        .unwrap();
    atlas_memory::api::run_migrations(&pool).await.unwrap();
    atlas_memory::api::MemoryStore::new(pool)
}

async fn app() -> (axum::Router, AuthStore, SessionStore) {
    let pool = AuthPool::connect("sqlite::memory:").await.unwrap();
    run_auth_migrations(&pool).await.unwrap();
    run_sessions_migrations(&pool).await.unwrap();
    let auth = AuthStore::new(pool.clone());
    let sessions = SessionStore::new(pool);
    (
        router(
            auth.clone(),
            sessions.clone(),
            fresh_memory_store().await,
            fresh_note_store().await,
        ),
        auth,
        sessions,
    )
}

/// The app plus the note store behind it, so a test can seed rows the
/// way another developer's would arrive.
async fn app_with_notes() -> (axum::Router, AuthStore, atlas_notes::api::NoteStore) {
    let pool = AuthPool::connect("sqlite::memory:").await.unwrap();
    run_auth_migrations(&pool).await.unwrap();
    run_sessions_migrations(&pool).await.unwrap();
    let auth = AuthStore::new(pool.clone());
    let sessions = SessionStore::new(pool);
    let notes = fresh_note_store().await;
    (
        router(
            auth.clone(),
            sessions,
            fresh_memory_store().await,
            notes.clone(),
        ),
        auth,
        notes,
    )
}

async fn bootstrap_user(auth: &AuthStore) -> (String, String) {
    let session = auth
        .register("owner@example.com", "correct horse battery", "Owner")
        .await
        .unwrap();
    (session.token, session.user.id.0)
}

fn sample(owner_id: &str) -> NewSession {
    NewSession {
        project: "your-org/your-project".to_owned(),
        owner_id: owner_id.to_owned(),
        remote_url: "git@github.com:you/your-project.git".to_owned(),
        branch: "main".to_owned(),
        relative_path: "your-org/your-project".to_owned(),
        agent_kind: None,
        title: None,
        resume_command: None,
    }
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
async fn push_without_a_token_is_unauthorized() {
    let (app, _auth, _sessions) = app().await;
    let (status, _) = post(app, "/sessions/push", json!({ "sessions": [] }), None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn pull_without_a_token_is_unauthorized() {
    let (app, _auth, _sessions) = app().await;
    let (status, _) = get(app, "/sessions/pull", None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn push_then_pull_round_trips_for_the_authenticated_owner() {
    let (app, auth, sessions) = app().await;
    let (token, owner_id) = bootstrap_user(&auth).await;
    let created = sessions.create(sample(&owner_id)).await.unwrap();

    let (status, push_body) = post(
        app.clone(),
        "/sessions/push",
        json!({ "sessions": [created] }),
        Some(&token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let pushed = push_body["sessions"].as_array().unwrap();
    assert_eq!(pushed.len(), 1);
    assert_eq!(pushed[0]["id"], created.id.0);
    assert_eq!(pushed[0]["owner_id"], owner_id);

    let (status, pull_body) = get(app, "/sessions/pull", Some(&token)).await;
    assert_eq!(status, StatusCode::OK);
    let pulled = pull_body["sessions"].as_array().unwrap();
    assert_eq!(pulled.len(), 1);
    assert_eq!(pulled[0]["id"], created.id.0);
}

#[tokio::test]
async fn push_ignores_a_client_supplied_owner_id_and_forces_the_authenticated_one() {
    let (app, auth, sessions) = app().await;
    let (token, owner_id) = bootstrap_user(&auth).await;
    let mut created = sessions.create(sample(&owner_id)).await.unwrap();
    created.owner_id = "someone-else".to_owned();

    let (status, body) = post(
        app,
        "/sessions/push",
        json!({ "sessions": [created] }),
        Some(&token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["sessions"][0]["owner_id"], owner_id);
}

#[tokio::test]
async fn pull_only_returns_the_authenticated_owners_sessions() {
    let (app, auth, sessions) = app().await;
    let (token_a, owner_a) = bootstrap_user(&auth).await;

    // A second user, registered directly against the store (the
    // router only allows one bootstrap registration over HTTP).
    let owner_b = "owner-b-id".to_owned();
    sessions.create(sample(&owner_a)).await.unwrap();
    sessions
        .create(NewSession {
            owner_id: owner_b,
            ..sample(&owner_a)
        })
        .await
        .unwrap();

    let (_, body) = get(app, "/sessions/pull", Some(&token_a)).await;
    let pulled = body["sessions"].as_array().unwrap();
    assert_eq!(pulled.len(), 1);
    assert_eq!(pulled[0]["owner_id"], owner_a);
}

#[tokio::test]
async fn pull_since_excludes_rows_at_or_before_the_cursor() {
    let (app, auth, sessions) = app().await;
    let (token, owner_id) = bootstrap_user(&auth).await;
    let created = sessions.create(sample(&owner_id)).await.unwrap();
    let cursor = urlencoding::encode(&created.updated_at.to_rfc3339()).into_owned();

    let (_, body) = get(app, &format!("/sessions/pull?since={cursor}"), Some(&token)).await;
    assert_eq!(body["sessions"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn push_a_stale_row_is_a_noop_last_write_wins() {
    let (app, auth, sessions) = app().await;
    let (token, owner_id) = bootstrap_user(&auth).await;
    let created = sessions.create(sample(&owner_id)).await.unwrap();
    let newer = sessions
        .update_status(
            &created.id,
            &owner_id,
            atlas_sessions::api::SessionStatus::Archived,
        )
        .await
        .unwrap();

    // Push the *original* (now stale) snapshot back.
    let (status, body) = post(
        app,
        "/sessions/push",
        json!({ "sessions": [created] }),
        Some(&token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    // The server reports the current authoritative row, not the
    // stale one that was pushed.
    assert_eq!(body["sessions"][0]["status"], "archived");
    let got_updated_at: chrono::DateTime<chrono::Utc> = body["sessions"][0]["updated_at"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    assert_eq!(got_updated_at, newer.updated_at);
}

#[tokio::test]
async fn notes_push_then_pull_round_trips_for_the_authenticated_owner() {
    let (app, auth, _) = app_with_notes().await;
    let (token, owner_id) = bootstrap_user(&auth).await;

    let (status, body) = post(
        app.clone(),
        "/notes/push",
        json!({ "notes": [{
            "id": "n1",
            "owner_id": owner_id,
            "name": "scratchpad",
            "body": "half an idea",
            "created_at": "2026-09-11T00:00:00Z",
            "updated_at": "2026-09-11T00:00:00Z",
        }]}),
        Some(&token),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");

    let (status, pulled) = get(app, "/notes/pull", Some(&token)).await;
    assert_eq!(status, StatusCode::OK);
    let notes = pulled["notes"].as_array().unwrap();
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0]["body"], "half an idea");
}

/// The property personal state exists for, over the wire.
#[tokio::test]
async fn a_pushed_note_claiming_another_owner_is_stored_under_the_caller() {
    let (app, auth, notes) = app_with_notes().await;
    let (token, owner_id) = bootstrap_user(&auth).await;

    let (status, _) = post(
        app,
        "/notes/push",
        json!({ "notes": [{
            "id": "n1",
            // A client claiming somebody else's notes.
            "owner_id": "01SOMEONEELSE",
            "name": "theirs",
            "body": "pushed",
            "created_at": "2026-09-11T00:00:00Z",
            "updated_at": "2026-09-11T00:00:00Z",
        }]}),
        Some(&token),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    assert_eq!(notes.list(&owner_id).await.unwrap().len(), 1);
    assert_eq!(
        notes.list("01SOMEONEELSE").await.unwrap().len(),
        0,
        "a client wrote into another developer's notes"
    );
}

#[tokio::test]
async fn pulling_never_returns_another_developers_notes() {
    let (app, auth, notes) = app_with_notes().await;
    let (token, _) = bootstrap_user(&auth).await;

    // Seeded directly, the way a teammate's rows sit on a shared
    // server.
    notes
        .write("01SOMEONEELSE", "salary", "asking for more")
        .await
        .unwrap();

    let (status, pulled) = get(app, "/notes/pull", Some(&token)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        pulled["notes"].as_array().unwrap().len(),
        0,
        "a pull returned somebody else's notes"
    );
}

#[tokio::test]
async fn notes_routes_require_a_token() {
    let (app, _, _) = app_with_notes().await;
    let (status, _) = post(app.clone(), "/notes/push", json!({ "notes": [] }), None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _) = get(app, "/notes/pull", None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn notes_pull_pages_forward_from_a_cursor() {
    let (app, auth, notes) = app_with_notes().await;
    let (token, owner_id) = bootstrap_user(&auth).await;

    notes.write(&owner_id, "first", "1").await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(1100)).await;
    let second = notes.write(&owner_id, "second", "2").await.unwrap();

    let (_, all) = get(app.clone(), "/notes/pull", Some(&token)).await;
    assert_eq!(all["notes"].as_array().unwrap().len(), 2);

    // Strictly after: a cursor at the newest row returns nothing, so a
    // poll loop does not re-deliver what it just saw.
    let cursor = second.updated_at.to_rfc3339();
    let (_, after) = get(
        app,
        &format!("/notes/pull?since={}", urlencoding(&cursor)),
        Some(&token),
    )
    .await;
    assert_eq!(after["notes"].as_array().unwrap().len(), 0);
}

fn urlencoding(raw: &str) -> String {
    raw.replace(':', "%3A").replace('+', "%2B")
}
