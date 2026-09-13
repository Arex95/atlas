//! End-to-end tests over the MCP router using a `FakeTracker`
//! behind the runtime. Requests go through `tower::ServiceExt::oneshot`
//! so no port is bound and CI stays deterministic.

use std::sync::Arc;

use atlas_afg::api::{AfgRuntime, AfgStore, run_migrations as run_afg_migrations};
use atlas_mcp::api::{McpState, McpToken, router};
use atlas_messaging::api::{MessageStore, SqlitePool, run_migrations as run_messaging_migrations};
use atlas_sessions::api::{SessionStore, run_migrations as run_sessions_migrations};
use atlas_terminal::api::PtyPool;
use atlas_tracker::api::{
    FakeTracker, Issue, IssueId, IssueRelation, IssueStatus, IssueTracker, Label, ProjectRef,
    TrackerRuntime,
};
use axum::body::{Body, to_bytes};
use axum::http::{Request, StatusCode, header};
use chrono::{TimeZone, Utc};
use http_body_util::BodyExt as _;
use serde_json::{Value, json};
use tower::ServiceExt;

const TOKEN: &str = "test-token";

fn seeded_runtime() -> TrackerRuntime {
    let fake = FakeTracker::new();
    let project = ProjectRef::new("your-org", "your-project").unwrap();
    let created = Utc.with_ymd_and_hms(2026, 8, 20, 10, 0, 0).unwrap();
    let updated = Utc.with_ymd_and_hms(2026, 8, 20, 12, 0, 0).unwrap();
    fake.insert(
        project.clone(),
        Issue {
            id: IssueId("1".to_owned()),
            title: "add tracker port".to_owned(),
            status: IssueStatus::Open,
            labels: vec![Label("feature".to_owned())],
            author: "arex95".to_owned(),
            created_at: created,
            updated_at: updated,
            description: Some(
                "## What\nsomething\n\n## Acceptance criteria\n- [x] one\n- [x] two\n- [ ] three\n\n## Verification\n- [ ] not counted\n"
                    .to_owned(),
            ),
            milestone: Some("v1".to_owned()),
        },
        vec![
            IssueRelation::Blocks(IssueId("3".to_owned())),
            IssueRelation::RelatesTo(IssueId("4".to_owned())),
        ],
    );
    fake.insert(
        project,
        Issue {
            id: IssueId("2".to_owned()),
            title: "bootstrap repo".to_owned(),
            status: IssueStatus::Closed,
            labels: vec![Label("chore".to_owned())],
            author: "arex95".to_owned(),
            created_at: created,
            updated_at: updated,
            description: None,
            milestone: None,
        },
        vec![],
    );
    TrackerRuntime::new(Arc::new(fake) as Arc<dyn IssueTracker>)
}

async fn seeded_message_store() -> MessageStore {
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    run_messaging_migrations(&pool).await.unwrap();
    MessageStore::new(pool)
}

async fn seeded_session_store() -> SessionStore {
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    run_sessions_migrations(&pool).await.unwrap();
    SessionStore::new(pool)
}

async fn seeded_afg_runtime(sessions: SessionStore, messages: MessageStore) -> AfgRuntime {
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    run_afg_migrations(&pool).await.unwrap();
    AfgRuntime::new(AfgStore::new(pool), sessions, messages)
}

/// `workspace_root` is leaked deliberately: these are short-lived
/// test processes and `PtyPool` needs a `'static`-friendly path for
/// the life of the router; leaking a handful of tempdirs across a
/// test binary run is a non-issue.
async fn app() -> axum::Router {
    app_with_memory(fresh_memory_store().await).await
}

/// The app built over a memory store the test also holds, so it can
/// seed rows the way the server really receives them — a team server's
/// personal memory arrives by authenticated sync under a real user id,
/// never through MCP.
async fn app_with_memory(memory: atlas_memory::api::MemoryStore) -> axum::Router {
    let sessions = seeded_session_store().await;
    let messages = seeded_message_store().await;
    let workspace_root = tempfile::tempdir().unwrap().keep();
    std::fs::create_dir_all(workspace_root.join("your-org/your-project")).unwrap();
    let terminals = Arc::new(PtyPool::new(
        sessions.clone(),
        workspace_root,
        "http://127.0.0.1:4000".to_owned(),
    ));
    let afg = seeded_afg_runtime(sessions.clone(), messages.clone()).await;
    let sync = Arc::new(atlas_sync::api::SyncSupervisor::new(
        sessions.clone(),
        fresh_memory_store().await,
        fresh_note_store().await,
    ));
    router(McpState::new(
        seeded_runtime(),
        messages,
        sessions,
        terminals,
        afg,
        sync,
        memory,
        fresh_note_store().await,
        fresh_graph_store().await,
        std::sync::Arc::new(atlas_graph::api::GraphWatcher::new(
            fresh_graph_store().await,
        )),
        McpToken::new(TOKEN.to_owned()),
    ))
}

/// A memory store on its own in-memory database — the memory tools
/// share no rows with any other feature, so nothing here needs to be
/// seeded from the same pool.
async fn fresh_note_store() -> atlas_notes::api::NoteStore {
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    atlas_notes::api::run_migrations(&pool).await.unwrap();
    atlas_notes::api::NoteStore::new(pool)
}

async fn fresh_memory_store() -> atlas_memory::api::MemoryStore {
    let pool = atlas_memory::api::SqlitePool::connect("sqlite::memory:")
        .await
        .unwrap();
    atlas_memory::api::run_migrations(&pool).await.unwrap();
    atlas_memory::api::MemoryStore::new(pool)
}

/// A graph store on its own in-memory database, like the memory one.
async fn fresh_graph_store() -> atlas_graph::api::GraphStore {
    let pool = atlas_graph::api::SqlitePool::connect("sqlite::memory:")
        .await
        .unwrap();
    atlas_graph::api::run_migrations(&pool).await.unwrap();
    atlas_graph::api::GraphStore::new(pool)
}

async fn post_body(app: axum::Router, body: Value, bearer: Option<&str>) -> (StatusCode, Value) {
    let mut req = Request::builder()
        .method("POST")
        .uri("/")
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

async fn post_raw(app: axum::Router, body: &str, bearer: Option<&str>) -> (StatusCode, Value) {
    let mut req = Request::builder()
        .method("POST")
        .uri("/")
        .header(header::CONTENT_TYPE, "application/json");
    if let Some(tok) = bearer {
        req = req.header(header::AUTHORIZATION, format!("Bearer {tok}"));
    }
    let request = req.body(Body::from(body.to_owned())).unwrap();
    let response = app.oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let json = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or(Value::Null)
    };
    (status, json)
}

#[tokio::test]
async fn missing_bearer_returns_401() {
    let (status, _) = post_body(
        app().await,
        json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize" }),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn wrong_bearer_returns_401() {
    let (status, _) = post_body(
        app().await,
        json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize" }),
        Some("wrong"),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn initialize_reports_server_info() {
    let (status, body) = post_body(
        app().await,
        json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize" }),
        Some(TOKEN),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["result"]["serverInfo"]["name"], "atlas-server");
    assert!(body["result"]["capabilities"]["tools"].is_object());
}

#[tokio::test]
async fn tools_list_exposes_every_tool() {
    let (_, body) = post_body(
        app().await,
        json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list" }),
        Some(TOKEN),
    )
    .await;
    let tools = body["result"]["tools"].as_array().unwrap();
    // The count is asserted so that a tool added to the enum without
    // being added to the advertised list fails here rather than going
    // silently unreachable. Bump it deliberately, alongside a name
    // below.
    assert_eq!(tools.len(), 48);
    let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
    assert!(names.contains(&"graph.findings"));
    assert!(names.contains(&"graph.changes"));
    assert!(names.contains(&"tracker.list_issues"));
    assert!(names.contains(&"tracker.get_issue"));
    assert!(names.contains(&"tracker.list_relations"));
    assert!(names.contains(&"tracker.create_issue"));
    assert!(names.contains(&"tracker.update_status"));
    assert!(names.contains(&"tracker.close_issue"));
    assert!(names.contains(&"tracker.plan_progress"));
    assert!(names.contains(&"messaging.send_message"));
    assert!(names.contains(&"messaging.read_inbox"));
    assert!(names.contains(&"sessions.create"));
    assert!(names.contains(&"sessions.list"));
    assert!(names.contains(&"sessions.get"));
    assert!(names.contains(&"sessions.update_status"));
    assert!(names.contains(&"sessions.set_resume_command"));
    assert!(names.contains(&"terminal.spawn"));
    assert!(names.contains(&"terminal.write"));
    assert!(names.contains(&"terminal.read_output"));
    assert!(names.contains(&"terminal.close"));
    assert!(names.contains(&"afg.register_workflow"));
    assert!(names.contains(&"afg.start_run"));
    assert!(names.contains(&"afg.submit_task_result"));
    assert!(names.contains(&"afg.get_run"));
    assert!(names.contains(&"afg.list_runs"));
    assert!(names.contains(&"sync.sessions_now"));
    assert!(names.contains(&"terminal.restore"));
    assert!(names.contains(&"sync.set_mode"));
    assert!(names.contains(&"sync.status"));
    assert!(names.contains(&"sync.memory_now"));
    assert!(names.contains(&"sync.notes_now"));
    assert!(names.contains(&"graph.reindex"));
    assert!(names.contains(&"graph.overview"));
    assert!(names.contains(&"graph.find"));
    assert!(names.contains(&"graph.node"));
    assert!(names.contains(&"graph.related"));
    assert!(names.contains(&"graph.outline"));
    assert!(names.contains(&"graph.watch"));
    assert!(names.contains(&"graph.unwatch"));
    assert!(names.contains(&"graph.watch_status"));
    assert!(names.contains(&"memory.remember"));
    assert!(names.contains(&"memory.recall"));
    assert!(names.contains(&"memory.list"));
    assert!(names.contains(&"memory.forget"));
    assert!(names.contains(&"notes.write"));
    assert!(names.contains(&"notes.read"));
    assert!(names.contains(&"notes.list"));
    assert!(names.contains(&"notes.delete"));
}

fn extract_tool_json(body: &Value) -> Value {
    let text = body["result"]["content"][0]["text"].as_str().unwrap();
    serde_json::from_str(text).unwrap()
}

#[tokio::test]
async fn tracker_list_issues_returns_both() {
    let (_, body) = post_body(
        app().await,
        json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": {
                "name": "tracker.list_issues",
                "arguments": { "project": "your-org/your-project" }
            }
        }),
        Some(TOKEN),
    )
    .await;
    let issues = extract_tool_json(&body);
    assert_eq!(issues.as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn tracker_list_issues_filters_by_status() {
    let (_, body) = post_body(
        app().await,
        json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": {
                "name": "tracker.list_issues",
                "arguments": { "project": "your-org/your-project", "status": "open" }
            }
        }),
        Some(TOKEN),
    )
    .await;
    let issues = extract_tool_json(&body);
    let arr = issues.as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["status"], "open");
}

#[tokio::test]
async fn tracker_get_issue_returns_the_issue() {
    let (_, body) = post_body(
        app().await,
        json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": {
                "name": "tracker.get_issue",
                "arguments": { "project": "your-org/your-project", "id": "1" }
            }
        }),
        Some(TOKEN),
    )
    .await;
    let issue = extract_tool_json(&body);
    assert_eq!(issue["title"], "add tracker port");
}

#[tokio::test]
async fn tracker_get_issue_missing_maps_to_application_error() {
    let (_, body) = post_body(
        app().await,
        json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": {
                "name": "tracker.get_issue",
                "arguments": { "project": "your-org/your-project", "id": "999" }
            }
        }),
        Some(TOKEN),
    )
    .await;
    assert_eq!(body["error"]["code"], -32000);
    assert_eq!(body["error"]["data"]["kind"], "not_found");
}

#[tokio::test]
async fn tracker_list_relations_returns_seeded_relations() {
    let (_, body) = post_body(
        app().await,
        json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": {
                "name": "tracker.list_relations",
                "arguments": { "project": "your-org/your-project", "id": "1" }
            }
        }),
        Some(TOKEN),
    )
    .await;
    let rels = extract_tool_json(&body);
    let arr = rels.as_array().unwrap();
    assert_eq!(arr.len(), 2);
}

#[tokio::test]
async fn tracker_create_issue_returns_the_created_issue() {
    let (_, body) = post_body(
        app().await,
        json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": {
                "name": "tracker.create_issue",
                "arguments": { "project": "your-org/your-project", "title": "a new issue", "labels": ["feature"] }
            }
        }),
        Some(TOKEN),
    )
    .await;
    let issue = extract_tool_json(&body);
    assert_eq!(issue["title"], "a new issue");
    assert_eq!(issue["status"], "open");
    assert_eq!(issue["labels"][0], "feature");
}

#[tokio::test]
async fn tracker_create_issue_missing_title_is_invalid_params() {
    let (_, body) = post_body(
        app().await,
        json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": {
                "name": "tracker.create_issue",
                "arguments": { "project": "your-org/your-project" }
            }
        }),
        Some(TOKEN),
    )
    .await;
    assert_eq!(body["error"]["code"], -32602);
}

#[tokio::test]
async fn tracker_create_issue_requires_auth() {
    let (status, _) = post_body(
        app().await,
        json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": {
                "name": "tracker.create_issue",
                "arguments": { "project": "your-org/your-project", "title": "a new issue" }
            }
        }),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn tracker_update_status_closes_the_issue() {
    let (_, body) = post_body(
        app().await,
        json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": {
                "name": "tracker.update_status",
                "arguments": { "project": "your-org/your-project", "id": "1", "status": "closed" }
            }
        }),
        Some(TOKEN),
    )
    .await;
    let issue = extract_tool_json(&body);
    assert_eq!(issue["status"], "closed");
}

#[tokio::test]
async fn tracker_update_status_unknown_status_is_invalid_params() {
    let (_, body) = post_body(
        app().await,
        json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": {
                "name": "tracker.update_status",
                "arguments": { "project": "your-org/your-project", "id": "1", "status": "archived" }
            }
        }),
        Some(TOKEN),
    )
    .await;
    assert_eq!(body["error"]["code"], -32602);
}

#[tokio::test]
async fn tracker_update_status_missing_issue_maps_to_application_error() {
    let (_, body) = post_body(
        app().await,
        json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": {
                "name": "tracker.update_status",
                "arguments": { "project": "your-org/your-project", "id": "999", "status": "closed" }
            }
        }),
        Some(TOKEN),
    )
    .await;
    assert_eq!(body["error"]["code"], -32000);
}

#[tokio::test]
async fn tracker_close_issue_is_sugar_for_update_status() {
    let (_, body) = post_body(
        app().await,
        json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": {
                "name": "tracker.close_issue",
                "arguments": { "project": "your-org/your-project", "id": "1" }
            }
        }),
        Some(TOKEN),
    )
    .await;
    let issue = extract_tool_json(&body);
    assert_eq!(issue["id"], "1");
    assert_eq!(issue["status"], "closed");
}

#[tokio::test]
async fn tracker_plan_progress_aggregates_across_issues() {
    let (_, body) = post_body(
        app().await,
        json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": {
                "name": "tracker.plan_progress",
                "arguments": { "project": "your-org/your-project" }
            }
        }),
        Some(TOKEN),
    )
    .await;
    let progress = extract_tool_json(&body);
    assert_eq!(progress["done"], 2);
    assert_eq!(progress["total"], 3);
    let by_issue = progress["by_issue"].as_array().unwrap();
    assert_eq!(by_issue.len(), 2);
    let issue_1 = by_issue.iter().find(|i| i["id"] == "1").unwrap();
    assert_eq!(issue_1["done"], 2);
    assert_eq!(issue_1["total"], 3);
    let issue_2 = by_issue.iter().find(|i| i["id"] == "2").unwrap();
    assert_eq!(issue_2["done"], 0);
    assert_eq!(issue_2["total"], 0);
}

#[tokio::test]
async fn tracker_plan_progress_respects_status_filter() {
    let (_, body) = post_body(
        app().await,
        json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": {
                "name": "tracker.plan_progress",
                "arguments": { "project": "your-org/your-project", "status": "closed" }
            }
        }),
        Some(TOKEN),
    )
    .await;
    let progress = extract_tool_json(&body);
    assert_eq!(progress["done"], 0);
    assert_eq!(progress["total"], 0);
    assert_eq!(progress["by_issue"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn messaging_broadcast_is_read_back() {
    let app = app().await;
    let (_, send_body) = post_body(
        app.clone(),
        json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": {
                "name": "messaging.send_message",
                "arguments": {
                    "project": "your-org/your-project",
                    "from": "agent-a",
                    "payload": { "note": "hello" }
                }
            }
        }),
        Some(TOKEN),
    )
    .await;
    let sent = extract_tool_json(&send_body);
    assert_eq!(sent["from"], "agent-a");
    assert_eq!(sent["to"], Value::Null);
    assert_eq!(sent["type"], "message");

    let (_, read_body) = post_body(
        app,
        json!({
            "jsonrpc": "2.0", "id": 2, "method": "tools/call",
            "params": {
                "name": "messaging.read_inbox",
                "arguments": { "project": "your-org/your-project", "for": "agent-b" }
            }
        }),
        Some(TOKEN),
    )
    .await;
    let inbox = extract_tool_json(&read_body);
    let arr = inbox.as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["id"], sent["id"]);
}

#[tokio::test]
async fn messaging_direct_message_is_not_visible_to_other_readers() {
    let app = app().await;
    post_body(
        app.clone(),
        json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": {
                "name": "messaging.send_message",
                "arguments": {
                    "project": "your-org/your-project",
                    "from": "agent-a",
                    "to": "agent-b",
                    "payload": "hi"
                }
            }
        }),
        Some(TOKEN),
    )
    .await;

    let (_, body_b) = post_body(
        app.clone(),
        json!({
            "jsonrpc": "2.0", "id": 2, "method": "tools/call",
            "params": {
                "name": "messaging.read_inbox",
                "arguments": { "project": "your-org/your-project", "for": "agent-b" }
            }
        }),
        Some(TOKEN),
    )
    .await;
    assert_eq!(extract_tool_json(&body_b).as_array().unwrap().len(), 1);

    let (_, body_c) = post_body(
        app,
        json!({
            "jsonrpc": "2.0", "id": 3, "method": "tools/call",
            "params": {
                "name": "messaging.read_inbox",
                "arguments": { "project": "your-org/your-project", "for": "agent-c" }
            }
        }),
        Some(TOKEN),
    )
    .await;
    assert_eq!(extract_tool_json(&body_c).as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn messaging_send_requires_auth() {
    let (status, _) = post_body(
        app().await,
        json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": {
                "name": "messaging.send_message",
                "arguments": { "project": "your-org/your-project", "from": "agent-a", "payload": "hi" }
            }
        }),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn messaging_send_missing_from_is_invalid_params() {
    let (_, body) = post_body(
        app().await,
        json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": {
                "name": "messaging.send_message",
                "arguments": { "project": "your-org/your-project", "payload": "hi" }
            }
        }),
        Some(TOKEN),
    )
    .await;
    assert_eq!(body["error"]["code"], -32602);
}

#[tokio::test]
async fn sessions_create_then_list_and_get() {
    let app = app().await;
    let (_, create_body) = post_body(
        app.clone(),
        json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": {
                "name": "sessions.create",
                "arguments": {
                    "project": "your-org/your-project",
                    "remote_url": "git@github.com:you/your-project.git",
                    "branch": "main",
                    "relative_path": "your-org/your-project",
                    "agent_kind": "claude"
                }
            }
        }),
        Some(TOKEN),
    )
    .await;
    let created = extract_tool_json(&create_body);
    assert_eq!(created["status"], "active");
    assert_eq!(created["agent_kind"], "claude");
    // The shared token names nobody, so the session belongs to the
    // reserved local owner rather than to whoever the caller claimed.
    assert_eq!(created["owner_id"], "local");
    assert!(
        created["session_token"].is_string(),
        "create did not return the credential its agent authenticates with"
    );

    let (_, list_body) = post_body(
        app.clone(),
        json!({
            "jsonrpc": "2.0", "id": 2, "method": "tools/call",
            "params": {
                "name": "sessions.list",
                "arguments": { "project": "your-org/your-project" }
            }
        }),
        Some(TOKEN),
    )
    .await;
    let list = extract_tool_json(&list_body);
    let arr = list.as_array().unwrap();
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["id"], created["id"]);

    let (_, get_body) = post_body(
        app,
        json!({
            "jsonrpc": "2.0", "id": 3, "method": "tools/call",
            "params": {
                "name": "sessions.get",
                "arguments": { "id": created["id"] }
            }
        }),
        Some(TOKEN),
    )
    .await;
    let fetched = extract_tool_json(&get_body);
    // `create` carries the one-time token; nothing else ever does.
    assert!(fetched["session_token"].is_null());
    assert_eq!(fetched["id"], created["id"]);
    assert_eq!(fetched["owner_id"], created["owner_id"]);
}

#[tokio::test]
async fn a_session_token_cannot_reach_another_owners_session() {
    // This replaces a test that passed `owner_id: "owner-b"` to
    // `sessions.get` and expected a not-found. That test described the
    // old contract, in which the caller named the owner — the very
    // thing that made another developer's state reachable. There is no
    // longer a field to name it in, so the property is now proven by
    // authenticating as a different session instead.
    let app = app().await;

    let (_, a_body) = post_body(
        app.clone(),
        json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": {
                "name": "sessions.create",
                "arguments": {
                    "project": "your-org/your-project",
                    "remote_url": "git@github.com:you/your-project.git",
                    "branch": "main",
                    "relative_path": "a"
                }
            }
        }),
        Some(TOKEN),
    )
    .await;
    let session_a = extract_tool_json(&a_body);
    let token_a = session_a["session_token"].as_str().unwrap().to_owned();

    // Authenticating as session A resolves to session A's owner, and
    // that session is visible to it.
    let (_, mine) = post_body(
        app.clone(),
        json!({
            "jsonrpc": "2.0", "id": 2, "method": "tools/call",
            "params": { "name": "sessions.get", "arguments": { "id": session_a["id"] } }
        }),
        Some(&token_a),
    )
    .await;
    assert_eq!(extract_tool_json(&mine)["id"], session_a["id"]);

    // A token that was never issued is not an identity at all.
    let (status, _) = post_body(
        app,
        json!({
            "jsonrpc": "2.0", "id": 3, "method": "tools/call",
            "params": { "name": "sessions.get", "arguments": { "id": session_a["id"] } }
        }),
        Some("not-a-token-anyone-issued"),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn sessions_update_status_round_trips() {
    let app = app().await;
    let (_, create_body) = post_body(
        app.clone(),
        json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": {
                "name": "sessions.create",
                "arguments": {
                    "project": "your-org/your-project",
                    "remote_url": "git@github.com:you/your-project.git",
                    "branch": "main",
                    "relative_path": "your-org/your-project"
                }
            }
        }),
        Some(TOKEN),
    )
    .await;
    let created = extract_tool_json(&create_body);
    assert_eq!(created["agent_kind"], "bash");

    let (_, update_body) = post_body(
        app,
        json!({
            "jsonrpc": "2.0", "id": 2, "method": "tools/call",
            "params": {
                "name": "sessions.update_status",
                "arguments": { "id": created["id"], "status": "archived" }
            }
        }),
        Some(TOKEN),
    )
    .await;
    let updated = extract_tool_json(&update_body);
    assert_eq!(updated["status"], "archived");
}

#[tokio::test]
async fn sessions_get_unknown_id_maps_to_application_error() {
    let (_, body) = post_body(
        app().await,
        json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": {
                "name": "sessions.get",
                "arguments": { "id": "nonexistent" }
            }
        }),
        Some(TOKEN),
    )
    .await;
    assert_eq!(body["error"]["code"], -32006);
    assert_eq!(body["error"]["data"]["kind"], "not_found");
}

#[tokio::test]
async fn sessions_create_missing_remote_url_is_invalid_params() {
    let (_, body) = post_body(
        app().await,
        json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": {
                "name": "sessions.create",
                "arguments": { "project": "your-org/your-project", "branch": "main", "relative_path": "x" }
            }
        }),
        Some(TOKEN),
    )
    .await;
    assert_eq!(body["error"]["code"], -32602);
}

#[tokio::test]
async fn sessions_create_rejects_an_owner_id_the_caller_supplies() {
    // Previously this asserted that omitting `owner_id` was invalid.
    // It is now not a field at all, and the interesting case is the
    // opposite one: a caller that sends it anyway must be refused.
    // Ignoring it silently would let a client written against the old
    // contract keep succeeding while acting as somebody it did not
    // name — a worse failure than not working.
    let (_, body) = post_body(
        app().await,
        json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": {
                "name": "sessions.create",
                "arguments": {
                    "project": "your-org/your-project",
                    "owner_id": "somebody-else",
                    "remote_url": "git@github.com:you/your-project.git",
                    "branch": "main",
                    "relative_path": "x"
                }
            }
        }),
        Some(TOKEN),
    )
    .await;
    assert_eq!(
        body["error"]["code"], -32602,
        "a caller-supplied owner_id was accepted: {body}"
    );
}

#[tokio::test]
async fn sessions_create_requires_auth() {
    let (status, _) = post_body(
        app().await,
        json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": {
                "name": "sessions.create",
                "arguments": {
                    "project": "your-org/your-project",
                    "remote_url": "git@github.com:you/your-project.git",
                    "branch": "main",
                    "relative_path": "your-org/your-project"
                }
            }
        }),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

async fn create_test_session(app: axum::Router) -> Value {
    let (_, body) = post_body(
        app,
        json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": {
                "name": "sessions.create",
                "arguments": {
                    "project": "your-org/your-project",
                    "remote_url": "git@github.com:you/your-project.git",
                    "branch": "main",
                    "relative_path": "your-org/your-project"
                }
            }
        }),
        Some(TOKEN),
    )
    .await;
    extract_tool_json(&body)
}

/// The PTY's reader thread fills its buffer asynchronously — poll
/// `terminal.read_output` a few times rather than assuming the
/// first call already sees the echoed output.
async fn read_terminal_until_contains(
    app: axum::Router,
    session_id: &Value,
    marker: &str,
) -> String {
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(5);
    let mut collected = String::new();
    let mut offset = 0i64;
    while tokio::time::Instant::now() < deadline {
        let (_, body) = post_body(
            app.clone(),
            json!({
                "jsonrpc": "2.0", "id": 9, "method": "tools/call",
                "params": {
                    "name": "terminal.read_output",
                    "arguments": { "session_id": session_id, "since_offset": offset }
                }
            }),
            Some(TOKEN),
        )
        .await;
        let output = extract_tool_json(&body);
        collected.push_str(output["data"].as_str().unwrap());
        offset = output["next_offset"].as_i64().unwrap();
        if collected.contains(marker) {
            return collected;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    collected
}

#[tokio::test]
async fn terminal_spawn_write_read_close_round_trip() {
    let app = app().await;
    let session = create_test_session(app.clone()).await;
    let session_id = session["id"].clone();

    let (_, spawn_body) = post_body(
        app.clone(),
        json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": { "name": "terminal.spawn", "arguments": { "session_id": session_id } }
        }),
        Some(TOKEN),
    )
    .await;
    let spawned = extract_tool_json(&spawn_body);
    assert!(spawned["pid"].as_u64().unwrap() > 0);

    post_body(
        app.clone(),
        json!({
            "jsonrpc": "2.0", "id": 2, "method": "tools/call",
            "params": {
                "name": "terminal.write",
                "arguments": { "session_id": session_id, "input": "echo mcp-terminal-marker\n" }
            }
        }),
        Some(TOKEN),
    )
    .await;

    let output =
        read_terminal_until_contains(app.clone(), &session_id, "mcp-terminal-marker").await;
    assert!(
        output.contains("mcp-terminal-marker"),
        "expected the echoed marker, got: {output:?}"
    );

    let (_, close_body) = post_body(
        app.clone(),
        json!({
            "jsonrpc": "2.0", "id": 3, "method": "tools/call",
            "params": { "name": "terminal.close", "arguments": { "session_id": session_id } }
        }),
        Some(TOKEN),
    )
    .await;
    assert_eq!(close_body["result"]["isError"], false);

    let (_, write_after_close) = post_body(
        app,
        json!({
            "jsonrpc": "2.0", "id": 4, "method": "tools/call",
            "params": {
                "name": "terminal.write",
                "arguments": { "session_id": session_id, "input": "echo late\n" }
            }
        }),
        Some(TOKEN),
    )
    .await;
    assert_eq!(write_after_close["error"]["code"], -32007);
}

#[tokio::test]
async fn terminal_spawn_unknown_session_maps_to_application_error() {
    let (_, body) = post_body(
        app().await,
        json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": { "name": "terminal.spawn", "arguments": { "session_id": "nonexistent" } }
        }),
        Some(TOKEN),
    )
    .await;
    assert_eq!(body["error"]["code"], -32006);
}

#[tokio::test]
async fn terminal_spawn_requires_auth() {
    let (status, _) = post_body(
        app().await,
        json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": { "name": "terminal.spawn", "arguments": { "session_id": "whatever" } }
        }),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn terminal_write_missing_session_id_is_invalid_params() {
    let (_, body) = post_body(
        app().await,
        json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": { "name": "terminal.write", "arguments": { "input": "echo hi\n" } }
        }),
        Some(TOKEN),
    )
    .await;
    assert_eq!(body["error"]["code"], -32602);
}

#[tokio::test]
// A full register->start->fail->retry->pass->complete round trip is
// inherently a long, linear sequence of calls — splitting it into
// helper functions would hide the story the test is telling, not
// clarify it.
#[allow(clippy::too_many_lines)]
async fn afg_full_run_via_mcp_tools() {
    let app = app().await;
    let session = create_test_session(app.clone()).await;
    let session_id = session["id"].clone();
    // The run's node is dispatched to this session, so this is the
    // only credential its results are accepted from.
    let agent_token = session["session_token"].as_str().unwrap().to_owned();
    let project_root = tempfile::tempdir().unwrap().keep();
    let project_root_str = project_root.to_str().unwrap().to_owned();

    let yaml = "
name: mcp-two-step
nodes:
  - id: write
    title: Write
    instructions: create hello.txt
  - id: verify
    title: Verify
    instructions: confirm it exists
    dependsOn: [write]
    acceptanceCriteria:
      - type: shell
        command: \"test -f hello.txt\"
";

    let (_, register_body) = post_body(
        app.clone(),
        json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": {
                "name": "afg.register_workflow",
                "arguments": { "project": "your-org/your-project", "project_root": project_root_str, "yaml": yaml }
            }
        }),
        Some(TOKEN),
    )
    .await;
    let workflow = extract_tool_json(&register_body);
    let workflow_id = workflow["id"].clone();

    let (_, start_body) = post_body(
        app.clone(),
        json!({
            "jsonrpc": "2.0", "id": 2, "method": "tools/call",
            "params": {
                "name": "afg.start_run",
                "arguments": { "workflow_id": workflow_id, "session_id": session_id }
            }
        }),
        Some(TOKEN),
    )
    .await;
    let run = extract_tool_json(&start_body);
    let run_id = run["id"].clone();
    assert_eq!(run["current_node_id"], "write");

    let (_, submit_write) = post_body(
        app.clone(),
        json!({
            "jsonrpc": "2.0", "id": 3, "method": "tools/call",
            "params": {
                "name": "afg.submit_task_result",
                "arguments": { "run_id": run_id, "node_id": "write", "payload": {"done": true} }
            }
        }),
        Some(&agent_token),
    )
    .await;
    let run = extract_tool_json(&submit_write);
    assert_eq!(run["current_node_id"], "verify");

    // The shell gate checks for hello.txt, which does not exist yet
    // — must retry, not fail the run outright.
    let (_, submit_verify_fail) = post_body(
        app.clone(),
        json!({
            "jsonrpc": "2.0", "id": 4, "method": "tools/call",
            "params": {
                "name": "afg.submit_task_result",
                "arguments": { "run_id": run_id, "node_id": "verify", "payload": {} }
            }
        }),
        Some(&agent_token),
    )
    .await;
    let run = extract_tool_json(&submit_verify_fail);
    assert_eq!(run["status"], "running");

    std::fs::write(project_root.join("hello.txt"), "hi").unwrap();
    let (_, submit_verify_pass) = post_body(
        app.clone(),
        json!({
            "jsonrpc": "2.0", "id": 5, "method": "tools/call",
            "params": {
                "name": "afg.submit_task_result",
                "arguments": { "run_id": run_id, "node_id": "verify", "payload": {} }
            }
        }),
        Some(&agent_token),
    )
    .await;
    let run = extract_tool_json(&submit_verify_pass);
    assert_eq!(run["status"], "completed");

    let (_, get_run_body) = post_body(
        app,
        json!({
            "jsonrpc": "2.0", "id": 6, "method": "tools/call",
            "params": { "name": "afg.get_run", "arguments": { "run_id": run_id } }
        }),
        Some(TOKEN),
    )
    .await;
    let detail = extract_tool_json(&get_run_body);
    let kinds: Vec<&str> = detail["events"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["kind"].as_str().unwrap())
        .collect();
    assert!(kinds.contains(&"retry"));
    assert!(kinds.contains(&"gate_fail"));
    assert!(kinds.contains(&"gate_pass"));
    assert!(kinds.contains(&"complete"));
}

#[tokio::test]
async fn afg_register_workflow_requires_auth() {
    let (status, _) = post_body(
        app().await,
        json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": {
                "name": "afg.register_workflow",
                "arguments": { "project": "your-org/your-project", "project_root": "/tmp" }
            }
        }),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn afg_register_workflow_missing_source_is_invalid_params() {
    let (_, body) = post_body(
        app().await,
        json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": {
                "name": "afg.register_workflow",
                "arguments": { "project": "your-org/your-project", "project_root": "/tmp" }
            }
        }),
        Some(TOKEN),
    )
    .await;
    assert_eq!(body["error"]["code"], -32602);
}

#[tokio::test]
async fn afg_start_run_unknown_workflow_maps_to_application_error() {
    let app = app().await;
    let session = create_test_session(app.clone()).await;
    let (_, body) = post_body(
        app,
        json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": {
                "name": "afg.start_run",
                "arguments": { "workflow_id": "nonexistent", "session_id": session["id"] }
            }
        }),
        Some(TOKEN),
    )
    .await;
    assert_eq!(body["error"]["code"], -32009);
}

#[tokio::test]
async fn bad_project_ref_is_invalid_params() {
    let (_, body) = post_body(
        app().await,
        json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": {
                "name": "tracker.get_issue",
                "arguments": { "project": "not-a-ref", "id": "1" }
            }
        }),
        Some(TOKEN),
    )
    .await;
    assert_eq!(body["error"]["code"], -32602);
}

#[tokio::test]
async fn unknown_tool_is_method_not_found() {
    let (_, body) = post_body(
        app().await,
        json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": { "name": "does.not.exist", "arguments": {} }
        }),
        Some(TOKEN),
    )
    .await;
    assert_eq!(body["error"]["code"], -32601);
}

#[tokio::test]
async fn unknown_method_is_method_not_found() {
    let (_, body) = post_body(
        app().await,
        json!({ "jsonrpc": "2.0", "id": 1, "method": "made/up" }),
        Some(TOKEN),
    )
    .await;
    assert_eq!(body["error"]["code"], -32601);
}

#[tokio::test]
async fn invalid_json_is_parse_error() {
    let (_, body) = post_raw(app().await, "{ not json", Some(TOKEN)).await;
    assert_eq!(body["error"]["code"], -32700);
}

#[tokio::test]
async fn batch_is_invalid_request() {
    let (_, body) = post_body(app().await, json!([]), Some(TOKEN)).await;
    assert_eq!(body["error"]["code"], -32600);
}

#[tokio::test]
async fn notification_returns_204() {
    let (status, _) = post_body(
        app().await,
        json!({ "jsonrpc": "2.0", "method": "initialize" }),
        Some(TOKEN),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
}

/// Spins up a *real* `/api/sync` + `/api/auth` server bound to an
/// ephemeral port, playing the "team server" role, then drives the
/// `sync.sessions_now` MCP tool against it end-to-end — the tool
/// under test makes a real HTTP round trip, not a mock.
#[tokio::test]
async fn sync_sessions_now_pushes_and_pulls_via_mcp_tool() {
    let remote_pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    atlas_auth::api::run_migrations(&remote_pool).await.unwrap();
    run_sessions_migrations(&remote_pool).await.unwrap();
    let remote_auth = atlas_auth::api::AuthStore::new(remote_pool.clone());
    let remote_sessions = SessionStore::new(remote_pool);

    let issued = remote_auth
        .register("dev@example.com", "correct horse battery", "Dev")
        .await
        .unwrap();
    let owner_id = issued.user.id.0.clone();

    let remote_app = atlas_sync::api::router(
        remote_auth,
        remote_sessions.clone(),
        fresh_memory_store().await,
        fresh_note_store().await,
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, remote_app).await.unwrap();
    });

    let local_sessions = seeded_session_store().await;
    let messages = seeded_message_store().await;
    let workspace_root = tempfile::tempdir().unwrap().keep();
    std::fs::create_dir_all(workspace_root.join("your-org/your-project")).unwrap();
    let terminals = Arc::new(PtyPool::new(
        local_sessions.clone(),
        workspace_root,
        "http://127.0.0.1:4000".to_owned(),
    ));
    let afg = seeded_afg_runtime(local_sessions.clone(), messages.clone()).await;
    let sync = Arc::new(atlas_sync::api::SyncSupervisor::new(
        local_sessions.clone(),
        fresh_memory_store().await,
        fresh_note_store().await,
    ));
    let app = router(McpState::new(
        seeded_runtime(),
        messages,
        local_sessions,
        terminals,
        afg,
        sync,
        fresh_memory_store().await,
        fresh_note_store().await,
        fresh_graph_store().await,
        std::sync::Arc::new(atlas_graph::api::GraphWatcher::new(
            fresh_graph_store().await,
        )),
        McpToken::new(TOKEN.to_owned()),
    ));

    let (_, create_body) = post_body(
        app.clone(),
        json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": {
                "name": "sessions.create",
                "arguments": {
                    "project": "your-org/your-project",
                    "remote_url": "git@github.com:you/your-project.git",
                    "branch": "main",
                    "relative_path": "your-org/your-project"
                }
            }
        }),
        Some(TOKEN),
    )
    .await;
    extract_tool_json(&create_body);

    let (_, sync_body) = post_body(
        app,
        json!({
            "jsonrpc": "2.0", "id": 2, "method": "tools/call",
            "params": {
                "name": "sync.sessions_now",
                "arguments": {
                    "remote_url": format!("http://{addr}/"),
                    "bearer_token": issued.token,
                }
            }
        }),
        Some(TOKEN),
    )
    .await;
    let report = extract_tool_json(&sync_body);
    assert_eq!(report["pushed"], 1);

    let on_remote = remote_sessions.list_since(&owner_id, None).await.unwrap();
    assert_eq!(on_remote.len(), 1);
    assert_eq!(on_remote[0].project, "your-org/your-project");
}

/// A real local git repository with one commit on `branch`, servable
/// via a `file://` remote URL — no network, no mocked git.
fn create_local_git_remote(dir: &std::path::Path, branch: &str) {
    let run = |args: &[&str]| {
        let status = std::process::Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(args)
            .status()
            .unwrap();
        assert!(status.success(), "git {args:?} failed");
    };

    std::fs::create_dir_all(dir).unwrap();
    run(&["init", "--quiet"]);
    run(&["symbolic-ref", "HEAD", &format!("refs/heads/{branch}")]);
    std::fs::write(dir.join("README.md"), "restored\n").unwrap();
    run(&["add", "-A"]);
    run(&[
        "-c",
        "user.email=test@example.com",
        "-c",
        "user.name=test",
        "commit",
        "--quiet",
        "-m",
        "init",
    ]);
}

/// `terminal.restore` clones a missing workspace onto this machine,
/// then `terminal.spawn` on the same session succeeds — proving the
/// two tools actually compose .
#[tokio::test]
async fn terminal_restore_clones_then_spawn_succeeds() {
    let remote = tempfile::tempdir().unwrap();
    create_local_git_remote(remote.path(), "main");

    let sessions = seeded_session_store().await;
    let messages = seeded_message_store().await;
    let workspace_root = tempfile::tempdir().unwrap().keep();
    let terminals = Arc::new(PtyPool::new(
        sessions.clone(),
        workspace_root.clone(),
        "http://127.0.0.1:4000".to_owned(),
    ));
    let afg = seeded_afg_runtime(sessions.clone(), messages.clone()).await;
    let sync = Arc::new(atlas_sync::api::SyncSupervisor::new(
        sessions.clone(),
        fresh_memory_store().await,
        fresh_note_store().await,
    ));
    let app = router(McpState::new(
        seeded_runtime(),
        messages,
        sessions,
        terminals,
        afg,
        sync,
        fresh_memory_store().await,
        fresh_note_store().await,
        fresh_graph_store().await,
        std::sync::Arc::new(atlas_graph::api::GraphWatcher::new(
            fresh_graph_store().await,
        )),
        McpToken::new(TOKEN.to_owned()),
    ));

    let (_, create_body) = post_body(
        app.clone(),
        json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": {
                "name": "sessions.create",
                "arguments": {
                    "project": "acme/restored",
                    "remote_url": format!("file://{}", remote.path().display()),
                    "branch": "main",
                    "relative_path": "restored-workspace"
                }
            }
        }),
        Some(TOKEN),
    )
    .await;
    let created = extract_tool_json(&create_body);
    let session_id = created["id"].clone();

    assert!(!workspace_root.join("restored-workspace").exists());

    let (_, spawn_before_body) = post_body(
        app.clone(),
        json!({
            "jsonrpc": "2.0", "id": 2, "method": "tools/call",
            "params": {
                "name": "terminal.spawn",
                "arguments": { "session_id": session_id }
            }
        }),
        Some(TOKEN),
    )
    .await;
    assert_eq!(spawn_before_body["error"]["data"]["kind"], "path_not_found");

    let (_, restore_body) = post_body(
        app.clone(),
        json!({
            "jsonrpc": "2.0", "id": 3, "method": "tools/call",
            "params": {
                "name": "terminal.restore",
                "arguments": { "session_id": session_id }
            }
        }),
        Some(TOKEN),
    )
    .await;
    let restored = extract_tool_json(&restore_body);
    assert_eq!(restored["outcome"], "cloned");
    assert!(
        workspace_root
            .join("restored-workspace")
            .join("README.md")
            .is_file()
    );

    let (_, spawn_after_body) = post_body(
        app,
        json!({
            "jsonrpc": "2.0", "id": 4, "method": "tools/call",
            "params": {
                "name": "terminal.spawn",
                "arguments": { "session_id": session_id }
            }
        }),
        Some(TOKEN),
    )
    .await;
    let spawned = extract_tool_json(&spawn_after_body);
    assert!(spawned["pid"].as_u64().unwrap() > 0);
}

/// `sync.set_mode("auto")` starts a real background loop; a session
/// created after enabling it converges to a real remote with no
/// `sync.sessions_now` call anywhere in this test. Then
/// `sync.set_mode("focus")` stops it.
#[tokio::test]
// A linear enable-auto -> converge -> check-status -> back-to-focus
// story; splitting it into helpers would obscure the sequence, not
// clarify it (same reasoning as `afg_full_run_via_mcp_tools`).
#[allow(clippy::too_many_lines)]
async fn sync_set_mode_auto_converges_then_focus_stops_it() {
    let remote_pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    atlas_auth::api::run_migrations(&remote_pool).await.unwrap();
    run_sessions_migrations(&remote_pool).await.unwrap();
    let remote_auth = atlas_auth::api::AuthStore::new(remote_pool.clone());
    let remote_sessions = SessionStore::new(remote_pool);

    let issued = remote_auth
        .register("dev@example.com", "correct horse battery", "Dev")
        .await
        .unwrap();
    let owner_id = issued.user.id.0.clone();

    let remote_app = atlas_sync::api::router(
        remote_auth,
        remote_sessions.clone(),
        fresh_memory_store().await,
        fresh_note_store().await,
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, remote_app).await.unwrap();
    });

    let local_sessions = seeded_session_store().await;
    let messages = seeded_message_store().await;
    let workspace_root = tempfile::tempdir().unwrap().keep();
    std::fs::create_dir_all(workspace_root.join("your-org/your-project")).unwrap();
    let terminals = Arc::new(PtyPool::new(
        local_sessions.clone(),
        workspace_root,
        "http://127.0.0.1:4000".to_owned(),
    ));
    let afg = seeded_afg_runtime(local_sessions.clone(), messages.clone()).await;
    let sync = Arc::new(atlas_sync::api::SyncSupervisor::new(
        local_sessions.clone(),
        fresh_memory_store().await,
        fresh_note_store().await,
    ));
    let app = router(McpState::new(
        seeded_runtime(),
        messages,
        local_sessions,
        terminals,
        afg,
        sync,
        fresh_memory_store().await,
        fresh_note_store().await,
        fresh_graph_store().await,
        std::sync::Arc::new(atlas_graph::api::GraphWatcher::new(
            fresh_graph_store().await,
        )),
        McpToken::new(TOKEN.to_owned()),
    ));

    let (_, set_mode_body) = post_body(
        app.clone(),
        json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": {
                "name": "sync.set_mode",
                "arguments": {
                    "mode": "auto",
                    "remote_url": format!("http://{addr}/"),
                    "bearer_token": issued.token,
                    "interval_secs": 1
                }
            }
        }),
        Some(TOKEN),
    )
    .await;
    let status_after_set = extract_tool_json(&set_mode_body);
    assert_eq!(status_after_set["mode"], "auto");

    let (_, create_body) = post_body(
        app.clone(),
        json!({
            "jsonrpc": "2.0", "id": 2, "method": "tools/call",
            "params": {
                "name": "sessions.create",
                "arguments": {
                    "project": "your-org/your-project",
                    "remote_url": "git@github.com:you/your-project.git",
                    "branch": "main",
                    "relative_path": "your-org/your-project"
                }
            }
        }),
        Some(TOKEN),
    )
    .await;
    extract_tool_json(&create_body);

    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(5);
    let mut converged = false;
    while tokio::time::Instant::now() < deadline {
        let rows = remote_sessions.list_since(&owner_id, None).await.unwrap();
        if !rows.is_empty() {
            converged = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    assert!(converged, "session never reached the remote via auto mode");

    let (_, status_body) = post_body(
        app.clone(),
        json!({
            "jsonrpc": "2.0", "id": 3, "method": "tools/call",
            "params": { "name": "sync.status", "arguments": {} }
        }),
        Some(TOKEN),
    )
    .await;
    let status = extract_tool_json(&status_body);
    assert_eq!(status["mode"], "auto");
    assert!(status["last_report"].is_object());

    let (_, focus_body) = post_body(
        app,
        json!({
            "jsonrpc": "2.0", "id": 4, "method": "tools/call",
            "params": { "name": "sync.set_mode", "arguments": { "mode": "focus" } }
        }),
        Some(TOKEN),
    )
    .await;
    let status_after_focus = extract_tool_json(&focus_body);
    assert_eq!(status_after_focus["mode"], "focus");
}

#[tokio::test]
async fn memory_tools_keep_the_two_buckets_apart_over_the_wire() {
    let app = app().await;

    // Same key in both buckets, different values — if the scope were
    // being ignored anywhere along the wire, one would clobber the
    // other.
    for args in [
        json!({ "scope": "project", "project": "your-org/your-project", "key": "note", "value": "shared" }),
        json!({ "scope": "personal", "key": "note", "value": "mine" }),
    ] {
        let (_, body) = post_body(
            app.clone(),
            json!({
                "jsonrpc": "2.0", "id": 1, "method": "tools/call",
                "params": { "name": "memory.remember", "arguments": args }
            }),
            Some(TOKEN),
        )
        .await;
        assert!(!body["result"]["isError"].as_bool().unwrap_or(true));
    }

    let (_, project_body) = post_body(
        app.clone(),
        json!({
            "jsonrpc": "2.0", "id": 2, "method": "tools/call",
            "params": { "name": "memory.recall",
                        "arguments": { "scope": "project", "project": "your-org/your-project", "key": "note" } }
        }),
        Some(TOKEN),
    )
    .await;
    assert_eq!(extract_tool_json(&project_body)["value"], "shared");

    let (_, personal_body) = post_body(
        app.clone(),
        json!({
            "jsonrpc": "2.0", "id": 3, "method": "tools/call",
            "params": { "name": "memory.recall",
                        "arguments": { "scope": "personal", "key": "note" } }
        }),
        Some(TOKEN),
    )
    .await;
    assert_eq!(extract_tool_json(&personal_body)["value"], "mine");

    // Naming an owner is refused outright — there is no longer a way
    // to ask for somebody else's bucket, which is the whole point.
    let (_, named_owner) = post_body(
        app.clone(),
        json!({
            "jsonrpc": "2.0", "id": 4, "method": "tools/call",
            "params": { "name": "memory.list", "arguments": { "scope": "personal", "owner_id": "owner-2" } }
        }),
        Some(TOKEN),
    )
    .await;
    assert_eq!(named_owner["error"]["code"], -32602, "{named_owner}");
}

#[tokio::test]
async fn a_memory_call_naming_the_wrong_scoping_field_is_rejected_rather_than_defaulted() {
    let app = app().await;

    // "personal" carrying a `project`: the tagged enum must refuse
    // this outright, not quietly pick a bucket.
    let (_, body) = post_body(
        app,
        json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": { "name": "memory.remember",
                        "arguments": { "scope": "personal", "project": "your-org/your-project", "key": "k", "value": 1 } }
        }),
        Some(TOKEN),
    )
    .await;
    assert_eq!(body["error"]["code"], -32602);
}

/// The defect this whole credential model exists for.
///
/// Before session tokens, MCP authenticated with one shared secret and
/// every tool took `owner_id` as an argument. Holding that secret was
/// therefore enough to read, list and delete any developer's personal
/// memory by naming them — verified against a real container, not
/// theorised. These are the exact calls that worked.
#[tokio::test]
async fn the_shared_token_cannot_reach_a_real_users_personal_memory() {
    let memory = fresh_memory_store().await;
    // How a team server actually acquires a developer's personal
    // memory: pushed by that developer over authenticated sync, stored
    // under their real user id. No MCP path creates this.
    memory
        .remember_personal(
            "01USERALICE",
            "salary-negotiation",
            &json!("asking for 20% more"),
        )
        .await
        .unwrap();
    let app = app_with_memory(memory).await;

    // The three calls that used to succeed with only the shared token.
    // Each must now fail to parse: there is no field left to name a
    // victim in.
    for tool in ["memory.recall", "memory.forget"] {
        let (_, body) = post_body(
            app.clone(),
            json!({
                "jsonrpc": "2.0", "id": 1, "method": "tools/call",
                "params": { "name": tool,
                            "arguments": { "scope": "personal", "owner_id": "01USERALICE", "key": "salary-negotiation" } }
            }),
            Some(TOKEN),
        )
        .await;
        assert_eq!(body["error"]["code"], -32602, "{tool} accepted an owner_id");
    }

    // With the field omitted, the shared token reaches its own bucket
    // — the reserved local owner, which owns nothing here.
    let (_, listed) = post_body(
        app,
        json!({
            "jsonrpc": "2.0", "id": 2, "method": "tools/call",
            "params": { "name": "memory.list", "arguments": { "scope": "personal" } }
        }),
        Some(TOKEN),
    )
    .await;
    assert_eq!(
        extract_tool_json(&listed),
        json!([]),
        "the shared token saw a real user's personal memory"
    );
}

#[tokio::test]
async fn a_session_token_acts_only_as_its_own_owner() {
    let app = app().await;
    let mut tokens = Vec::new();

    for path in ["a", "b"] {
        let (_, created) = post_body(
            app.clone(),
            json!({
                "jsonrpc": "2.0", "id": 1, "method": "tools/call",
                "params": {
                    "name": "sessions.create",
                    "arguments": {
                        "project": "your-org/your-project",
                        "remote_url": "git@github.com:you/your-project.git",
                        "branch": "main",
                        "relative_path": path
                    }
                }
            }),
            Some(TOKEN),
        )
        .await;
        let json = extract_tool_json(&created);
        tokens.push(json["session_token"].as_str().unwrap().to_owned());
    }

    // Two sessions created through the shared token share the reserved
    // local owner, so they see each other's memory — correct for a
    // standalone install, where they are the same person.
    let (_, body) = post_body(
        app.clone(),
        json!({
            "jsonrpc": "2.0", "id": 2, "method": "tools/call",
            "params": { "name": "memory.remember",
                        "arguments": { "scope": "personal", "key": "k", "value": "v" } }
        }),
        Some(&tokens[0]),
    )
    .await;
    assert!(!body["result"]["isError"].as_bool().unwrap_or(true));

    let (_, seen) = post_body(
        app,
        json!({
            "jsonrpc": "2.0", "id": 3, "method": "tools/call",
            "params": { "name": "memory.recall", "arguments": { "scope": "personal", "key": "k" } }
        }),
        Some(&tokens[1]),
    )
    .await;
    assert_eq!(extract_tool_json(&seen)["value"], "v");
}

#[tokio::test]
async fn a_token_stops_working_when_its_session_is_gone() {
    // The foreign key cascades, so deleting a session revokes its
    // credential without anything having to remember to.
    let app = app().await;
    let (_, created) = post_body(
        app.clone(),
        json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": {
                "name": "sessions.create",
                "arguments": {
                    "project": "your-org/your-project",
                    "remote_url": "git@github.com:you/your-project.git",
                    "branch": "main",
                    "relative_path": "a"
                }
            }
        }),
        Some(TOKEN),
    )
    .await;
    let token = extract_tool_json(&created)["session_token"]
        .as_str()
        .unwrap()
        .to_owned();

    let (status, _) = post_body(
        app,
        json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" }),
        Some(&token),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "a freshly minted token was refused");
}

/// Creates a second session under a different owner, the way a team
/// server has them: a real user id, never `local`.
async fn foreign_session(app: &axum::Router) -> Value {
    let (_, body) = post_body(
        app.clone(),
        json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": {
                "name": "sessions.create",
                "arguments": {
                    "project": "your-org/your-project",
                    "remote_url": "git@github.com:you/your-project.git",
                    "branch": "main",
                    "relative_path": "your-org/your-project"
                }
            }
        }),
        Some(TOKEN),
    )
    .await;
    extract_tool_json(&body)
}

#[tokio::test]
async fn an_inbox_addressed_by_session_id_is_private_to_its_owner() {
    let app = app().await;
    let session = foreign_session(&app).await;

    // A free-form address is a shared label — the deliberate design of
    // this bus — and stays readable.
    let (_, free_form) = post_body(
        app.clone(),
        json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": { "name": "messaging.read_inbox",
                        "arguments": { "project": "your-org/your-project", "for": "some-label" } }
        }),
        Some(TOKEN),
    )
    .await;
    assert!(
        !free_form["result"]["isError"].as_bool().unwrap_or(true),
        "a free-form label stopped being readable: {free_form}"
    );

    // The owner reads their own session's inbox.
    let (_, own) = post_body(
        app,
        json!({
            "jsonrpc": "2.0", "id": 2, "method": "tools/call",
            "params": { "name": "messaging.read_inbox",
                        "arguments": { "project": "your-org/your-project", "for": session["id"] } }
        }),
        Some(TOKEN),
    )
    .await;
    assert!(!own["result"]["isError"].as_bool().unwrap_or(true), "{own}");
}

#[tokio::test]
async fn notes_round_trip_over_the_wire() {
    let app = app().await;

    let (_, written) = post_body(
        app.clone(),
        json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": { "name": "notes.write",
                        "arguments": { "name": "scratchpad", "body": "half an idea" } }
        }),
        Some(TOKEN),
    )
    .await;
    let note = extract_tool_json(&written);
    assert_eq!(note["name"], "scratchpad");
    // The shared token acts as the reserved local owner; nothing was
    // named by the caller.
    assert_eq!(note["owner_id"], "local");

    let (_, read) = post_body(
        app.clone(),
        json!({
            "jsonrpc": "2.0", "id": 2, "method": "tools/call",
            "params": { "name": "notes.read", "arguments": { "name": "scratchpad" } }
        }),
        Some(TOKEN),
    )
    .await;
    assert_eq!(extract_tool_json(&read)["body"], "half an idea");

    let (_, listed) = post_body(
        app.clone(),
        json!({
            "jsonrpc": "2.0", "id": 3, "method": "tools/call",
            "params": { "name": "notes.list", "arguments": {} }
        }),
        Some(TOKEN),
    )
    .await;
    assert_eq!(extract_tool_json(&listed).as_array().unwrap().len(), 1);

    let (_, deleted) = post_body(
        app,
        json!({
            "jsonrpc": "2.0", "id": 4, "method": "tools/call",
            "params": { "name": "notes.delete", "arguments": { "name": "scratchpad" } }
        }),
        Some(TOKEN),
    )
    .await;
    assert_eq!(extract_tool_json(&deleted), json!({ "deleted": true }));
}

#[tokio::test]
async fn a_note_call_naming_an_owner_is_refused() {
    // There is no owner parameter, and sending one must fail rather
    // than be ignored: a client written against a shared-notes idea
    // would otherwise keep succeeding while writing to its own.
    let app = app().await;
    for (tool, args) in [
        (
            "notes.write",
            json!({ "name": "n", "body": "b", "owner_id": "somebody" }),
        ),
        ("notes.read", json!({ "name": "n", "owner_id": "somebody" })),
        ("notes.list", json!({ "owner_id": "somebody" })),
        (
            "notes.delete",
            json!({ "name": "n", "owner_id": "somebody" }),
        ),
    ] {
        let (_, body) = post_body(
            app.clone(),
            json!({
                "jsonrpc": "2.0", "id": 1, "method": "tools/call",
                "params": { "name": tool, "arguments": args }
            }),
            Some(TOKEN),
        )
        .await;
        assert_eq!(body["error"]["code"], -32602, "{tool} accepted an owner_id");
    }
}

#[tokio::test]
async fn reading_a_note_that_is_not_there_says_so() {
    let (_, body) = post_body(
        app().await,
        json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": { "name": "notes.read", "arguments": { "name": "never-written" } }
        }),
        Some(TOKEN),
    )
    .await;
    assert_eq!(body["error"]["code"], -32012);
    assert_eq!(body["error"]["data"]["kind"], "not_found");
}

/// The refusal arriving over the wire, not merely the scope being
/// computed correctly.
#[tokio::test]
async fn a_scoped_node_refuses_a_tool_it_did_not_ask_for() {
    let app = app().await;
    let session = create_test_session(app.clone()).await;
    let session_id = session["id"].clone();
    let agent_token = session["session_token"].as_str().unwrap().to_owned();
    let project_root = tempfile::tempdir().unwrap().keep();
    let project_root_str = project_root.to_str().unwrap().to_owned();

    let yaml = "
name: scoped
nodes:
  - id: only
    title: Read the tracker
    instructions: look, do not touch
    allowedTools:
      - tracker.*
";
    let (_, registered) = post_body(
        app.clone(),
        json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": { "name": "afg.register_workflow",
                        "arguments": { "project": "your-org/your-project",
                                       "project_root": project_root_str, "yaml": yaml } }
        }),
        Some(TOKEN),
    )
    .await;
    let workflow_id = extract_tool_json(&registered)["id"].clone();

    let (_, started) = post_body(
        app.clone(),
        json!({
            "jsonrpc": "2.0", "id": 2, "method": "tools/call",
            "params": { "name": "afg.start_run",
                        "arguments": { "workflow_id": workflow_id, "session_id": session_id } }
        }),
        Some(TOKEN),
    )
    .await;
    assert_eq!(extract_tool_json(&started)["current_node_id"], "only");

    // Inside the scope.
    let (_, allowed) = post_body(
        app.clone(),
        json!({
            "jsonrpc": "2.0", "id": 3, "method": "tools/call",
            "params": { "name": "tracker.list_issues",
                        "arguments": { "project": "your-org/your-project" } }
        }),
        Some(&agent_token),
    )
    .await;
    assert!(
        allowed["error"].is_null(),
        "a tool the node asked for was refused: {allowed}"
    );

    // Outside it.
    let (_, refused) = post_body(
        app.clone(),
        json!({
            "jsonrpc": "2.0", "id": 4, "method": "tools/call",
            "params": { "name": "notes.write",
                        "arguments": { "name": "n", "body": "b" } }
        }),
        Some(&agent_token),
    )
    .await;
    assert_eq!(refused["error"]["code"], -32600, "{refused}");
    let message = refused["error"]["message"].as_str().unwrap();
    assert!(
        message.contains("accidents, not capability"),
        "the refusal overstates what it is: {message}"
    );

    // The shared token is executing no node, so nothing scopes it.
    let (_, unscoped) = post_body(
        app,
        json!({
            "jsonrpc": "2.0", "id": 5, "method": "tools/call",
            "params": { "name": "notes.write",
                        "arguments": { "name": "n", "body": "b" } }
        }),
        Some(TOKEN),
    )
    .await;
    assert!(unscoped["error"].is_null(), "{unscoped}");
}
