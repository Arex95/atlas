//! Integration tests for AFG slice 4 — the live run view over SSE.
//!
//! Uses `tower::ServiceExt::oneshot` and then polls the response body
//! frame by frame: an SSE response's headers arrive immediately and
//! its body is produced lazily, so events published *after* the
//! request are observed exactly as a real watcher would see them.

use std::collections::VecDeque;
use std::time::Duration;

/// The credential the session a node was dispatched to would present.
///
/// A result is accepted only from that session, so every one of these
/// tests has to say who is answering — which is the point.
fn agent(session_id: &str, owner_id: &str) -> Caller {
    Caller::Session {
        session_id: session_id.to_owned(),
        owner_id: owner_id.to_owned(),
    }
}

use atlas_afg::api::{
    AfgRuntime, AfgStore, RunStatus, WorkflowId, router, run_migrations as run_afg_migrations,
};
use atlas_auth::api::{AuthStore, run_migrations as run_auth_migrations};
use atlas_messaging::api::{MessageStore, SqlitePool, run_migrations as run_messaging_migrations};
use atlas_sessions::api::{
    Caller, NewSession, SessionStore, run_migrations as run_sessions_migrations,
};
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tempfile::TempDir;
use tower::ServiceExt;

struct Harness {
    app: axum::Router,
    runtime: AfgRuntime,
    /// Bearer for the user who owns the run.
    token: String,
    session_id: String,
    /// The real user the session belongs to. Every runtime call is
    /// made as them, because acting on a session now requires owning
    /// it.
    owner_id: String,
    auth: AuthStore,
}

async fn harness() -> Harness {
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    run_afg_migrations(&pool).await.unwrap();
    run_messaging_migrations(&pool).await.unwrap();
    run_sessions_migrations(&pool).await.unwrap();
    run_auth_migrations(&pool).await.unwrap();

    let sessions = SessionStore::new(pool.clone());
    let messages = MessageStore::new(pool.clone());
    let auth = AuthStore::new(pool.clone());
    // One store instance shared by the runtime and the router: the
    // live-view hub lives inside it, so two instances would leave a
    // watcher subscribed to a hub nobody publishes to.
    let store = AfgStore::new(pool);

    let issued = auth
        .register("dev@example.com", "correct horse battery", "Dev")
        .await
        .unwrap();

    let session = sessions
        .create(NewSession {
            project: "your-org/your-project".to_owned(),
            owner_id: issued.user.id.0.clone(),
            remote_url: "git@github.com:you/your-project.git".to_owned(),
            branch: "main".to_owned(),
            relative_path: "your-org/your-project".to_owned(),
            agent_kind: None,
            title: None,
            resume_command: None,
        })
        .await
        .unwrap();

    Harness {
        app: router(store.clone(), auth.clone(), sessions.clone()),
        runtime: AfgRuntime::new(store, sessions, messages),
        token: issued.token,
        session_id: session.id.0,
        owner_id: issued.user.id.0.clone(),
        auth,
    }
}

/// A second real account on the *same* auth store. `register` is
/// bootstrap-only, so a teammate arrives by invitation — and changes
/// the temporary password, so this test turns on ownership alone
/// rather than on the invited-account restriction.
async fn second_user(auth: &AuthStore) -> String {
    let invited = auth
        .invite_user("intruder@example.com", "Nope")
        .await
        .unwrap();
    let session = auth
        .login("intruder@example.com", &invited.temporary_password)
        .await
        .unwrap();
    auth.change_password(
        &session.user.id,
        &invited.temporary_password,
        "a settled password",
    )
    .await
    .unwrap();
    auth.login("intruder@example.com", "a settled password")
        .await
        .unwrap()
        .token
}

const TWO_NODE_YAML: &str = "
name: two-step
nodes:
  - id: write
    title: Write the file
    instructions: create hello.txt containing 'hi'
  - id: verify
    title: Verify the file
    instructions: report back once done
    dependsOn: [write]
";

async fn register_workflow(runtime: &AfgRuntime, project_root: &str) -> WorkflowId {
    runtime
        .register_workflow("your-org/your-project", project_root, None, Some(TWO_NODE_YAML))
        .await
        .unwrap()
        .id
}

/// What one poll of a watcher's stream produced.
#[derive(Debug)]
enum Next {
    Event {
        name: String,
        data: Value,
    },
    /// The server closed the stream.
    Closed,
    /// Still open, nothing arrived in the window.
    Idle,
}

struct SseReader {
    body: Body,
    buffer: String,
    pending: VecDeque<(String, String)>,
}

impl SseReader {
    fn new(body: Body) -> Self {
        Self {
            body,
            buffer: String::new(),
            pending: VecDeque::new(),
        }
    }

    async fn next(&mut self, within: Duration) -> Next {
        loop {
            if let Some((name, data)) = self.pending.pop_front() {
                return Next::Event {
                    name,
                    data: serde_json::from_str(&data).unwrap_or(Value::String(data)),
                };
            }

            let Ok(frame) = tokio::time::timeout(within, self.body.frame()).await else {
                return Next::Idle;
            };
            let Some(frame) = frame else {
                return Next::Closed;
            };
            let Ok(frame) = frame else {
                return Next::Closed;
            };
            let Ok(bytes) = frame.into_data() else {
                continue;
            };
            self.buffer.push_str(std::str::from_utf8(&bytes).unwrap());

            while let Some(idx) = self.buffer.find("\n\n") {
                let raw: String = self.buffer.drain(..idx + 2).collect();
                let mut name = String::new();
                let mut data: Vec<String> = Vec::new();
                for line in raw.lines() {
                    if let Some(v) = line.strip_prefix("event:") {
                        v.trim().clone_into(&mut name);
                    } else if let Some(v) = line.strip_prefix("data:") {
                        data.push(v.strip_prefix(' ').unwrap_or(v).to_owned());
                    }
                }
                if !data.is_empty() {
                    self.pending.push_back((name, data.join("\n")));
                }
            }
        }
    }
}

async fn open_stream(app: axum::Router, run_id: &str, bearer: Option<&str>) -> (StatusCode, Body) {
    let mut req = Request::builder()
        .method("GET")
        .uri(format!("/runs/{run_id}/events"));
    if let Some(token) = bearer {
        req = req.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
    let response = app.oneshot(req.body(Body::empty()).unwrap()).await.unwrap();
    (response.status(), response.into_body())
}

#[tokio::test]
async fn replays_the_timeline_then_delivers_new_events_on_the_same_connection() {
    let h = harness().await;
    let dir = TempDir::new().unwrap();
    let project_root = dir.path().to_str().unwrap();
    let workflow_id = register_workflow(&h.runtime, project_root).await;

    // Starting the run records an `enter` before anyone is watching.
    let run = h
        .runtime
        .start_run(
            &workflow_id,
            &h.session_id,
            None,
            &agent(&h.session_id, &h.owner_id),
        )
        .await
        .unwrap();

    let (status, body) = open_stream(h.app.clone(), &run.id.0, Some(&h.token)).await;
    assert_eq!(status, StatusCode::OK);
    let mut stream = SseReader::new(body);

    // Replayed history: the watcher arrived late and still sees it.
    let Next::Event { name, data } = stream.next(Duration::from_secs(5)).await else {
        panic!("the stream delivered no history");
    };
    assert_eq!(name, "enter");
    assert_eq!(data["node_id"], "write");

    // Now advance the run — these must arrive live, on this same
    // connection, with no second request.
    h.runtime
        .submit_task_result(
            &run.id,
            "write",
            &agent(&h.session_id, &h.owner_id),
            Some(&json!({"done": true})),
        )
        .await
        .unwrap();

    let mut kinds = Vec::new();
    for _ in 0..4 {
        match stream.next(Duration::from_secs(5)).await {
            Next::Event { name, .. } => kinds.push(name),
            other => panic!("expected a live event, got {other:?}"),
        }
        if kinds.len() >= 3 {
            break;
        }
    }
    assert_eq!(kinds, vec!["exec", "advance", "enter"]);
}

#[tokio::test]
async fn every_event_carries_its_payload_so_a_viewer_needs_no_second_call() {
    let h = harness().await;
    let dir = TempDir::new().unwrap();
    let project_root = dir.path().to_str().unwrap();
    let workflow_id = register_workflow(&h.runtime, project_root).await;
    let run = h
        .runtime
        .start_run(
            &workflow_id,
            &h.session_id,
            None,
            &agent(&h.session_id, &h.owner_id),
        )
        .await
        .unwrap();

    let (_, body) = open_stream(h.app.clone(), &run.id.0, Some(&h.token)).await;
    let mut stream = SseReader::new(body);
    stream.next(Duration::from_secs(5)).await; // the replayed `enter`

    h.runtime
        .submit_task_result(
            &run.id,
            "write",
            &agent(&h.session_id, &h.owner_id),
            Some(&json!({"wrote": "hello.txt"})),
        )
        .await
        .unwrap();

    let Next::Event { name, data } = stream.next(Duration::from_secs(5)).await else {
        panic!("no exec event arrived");
    };
    assert_eq!(name, "exec");
    assert_eq!(data["kind"], "exec");
    assert_eq!(data["node_id"], "write");
    assert_eq!(data["payload"]["wrote"], "hello.txt");
    assert!(data["id"].as_str().is_some());
    assert!(data["at"].as_str().is_some());
}

#[tokio::test]
async fn the_stream_closes_once_the_run_reaches_a_terminal_event() {
    let h = harness().await;
    let dir = TempDir::new().unwrap();
    let project_root = dir.path().to_str().unwrap();
    let workflow_id = register_workflow(&h.runtime, project_root).await;
    let run = h
        .runtime
        .start_run(
            &workflow_id,
            &h.session_id,
            None,
            &agent(&h.session_id, &h.owner_id),
        )
        .await
        .unwrap();

    let (_, body) = open_stream(h.app.clone(), &run.id.0, Some(&h.token)).await;
    let mut stream = SseReader::new(body);

    // Drive the run to completion.
    h.runtime
        .submit_task_result(
            &run.id,
            "write",
            &agent(&h.session_id, &h.owner_id),
            Some(&json!({"done": true})),
        )
        .await
        .unwrap();
    let finished = h
        .runtime
        .submit_task_result(
            &run.id,
            "verify",
            &agent(&h.session_id, &h.owner_id),
            Some(&json!({"done": true})),
        )
        .await
        .unwrap();
    assert_eq!(finished.status, RunStatus::Completed);

    // Read until the terminal event, then the stream must end by
    // itself rather than hang open on a run that will never move.
    let mut saw_complete = false;
    for _ in 0..12 {
        match stream.next(Duration::from_secs(5)).await {
            Next::Event { name, .. } => {
                if name == "complete" {
                    saw_complete = true;
                }
            }
            Next::Closed => break,
            Next::Idle => panic!("the stream stayed open after the run completed"),
        }
    }
    assert!(saw_complete, "never saw the terminal event");
    assert!(
        matches!(stream.next(Duration::from_secs(2)).await, Next::Closed),
        "the stream did not close after the terminal event"
    );
}

#[tokio::test]
async fn a_run_owned_by_another_user_is_not_found() {
    let h = harness().await;
    let dir = TempDir::new().unwrap();
    let project_root = dir.path().to_str().unwrap();
    let workflow_id = register_workflow(&h.runtime, project_root).await;
    let run = h
        .runtime
        .start_run(
            &workflow_id,
            &h.session_id,
            None,
            &agent(&h.session_id, &h.owner_id),
        )
        .await
        .unwrap();

    let intruder_token = second_user(&h.auth).await;

    let (status, _) = open_stream(h.app.clone(), &run.id.0, Some(&intruder_token)).await;
    // 404, not 403 — the endpoint must not confirm the run exists.
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn an_unknown_run_is_not_found() {
    let h = harness().await;
    let (status, _) = open_stream(h.app.clone(), "no-such-run", Some(&h.token)).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn the_stream_requires_a_valid_bearer() {
    let h = harness().await;
    let dir = TempDir::new().unwrap();
    let project_root = dir.path().to_str().unwrap();
    let workflow_id = register_workflow(&h.runtime, project_root).await;
    let run = h
        .runtime
        .start_run(
            &workflow_id,
            &h.session_id,
            None,
            &agent(&h.session_id, &h.owner_id),
        )
        .await
        .unwrap();

    let (missing, _) = open_stream(h.app.clone(), &run.id.0, None).await;
    assert_eq!(missing, StatusCode::UNAUTHORIZED);

    let (garbage, _) = open_stream(h.app.clone(), &run.id.0, Some("not-a-token")).await;
    assert_eq!(garbage, StatusCode::UNAUTHORIZED);
}
