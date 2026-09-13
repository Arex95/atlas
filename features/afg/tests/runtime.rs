//! Integration tests exercising a full AFG run against real
//! `SQLite` databases (tracker/messaging/sessions-shaped stack) —
//! no mocks for storage. The `shell` gate spawns a real subprocess.

use atlas_afg::api::{
    AfgError, AfgRuntime, AfgStore, RunStatus, run_migrations as run_afg_migrations,
};
use atlas_messaging::api::{MessageStore, SqlitePool, run_migrations as run_messaging_migrations};
use atlas_sessions::api::{
    Caller, NewSession, SessionStore, run_migrations as run_sessions_migrations,
};
use serde_json::json;
use tempfile::TempDir;

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

async fn fresh_runtime() -> (AfgRuntime, MessageStore, String) {
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    run_afg_migrations(&pool).await.unwrap();
    run_messaging_migrations(&pool).await.unwrap();
    run_sessions_migrations(&pool).await.unwrap();

    let sessions = SessionStore::new(pool.clone());
    let messages = MessageStore::new(pool.clone());
    let store = AfgStore::new(pool);

    let session = sessions
        .create(NewSession {
            project: "your-org/your-project".to_owned(),
            owner_id: "owner-1".to_owned(),
            remote_url: "git@github.com:you/your-project.git".to_owned(),
            branch: "main".to_owned(),
            relative_path: "your-org/your-project".to_owned(),
            agent_kind: None,
            title: None,
            resume_command: None,
        })
        .await
        .unwrap();

    let runtime = AfgRuntime::new(store, sessions, messages.clone());
    (runtime, messages, session.id.0)
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
    acceptanceCriteria:
      - type: shell
        command: \"test -f hello.txt\"
";

#[tokio::test]
async fn full_run_advances_through_two_nodes_and_completes() {
    let (runtime, messages, session_id) = fresh_runtime().await;
    let dir = TempDir::new().unwrap();
    let project_root = dir.path().to_str().unwrap();

    let workflow = runtime
        .register_workflow(
            "your-org/your-project",
            project_root,
            None,
            Some(TWO_NODE_YAML),
        )
        .await
        .unwrap();

    let run = runtime
        .start_run(
            &workflow.id,
            &session_id,
            None,
            &agent(&session_id, "owner-1"),
        )
        .await
        .unwrap();
    assert_eq!(run.status, RunStatus::Running);
    assert_eq!(run.current_node_id.as_deref(), Some("write"));

    // The runtime dispatched a "task" message for the first node —
    // confirm it actually landed on the bus, same transport as any
    // other coordination message.
    let inbox = messages
        .read_inbox("your-org/your-project", &session_id, None, None)
        .await
        .unwrap();
    assert_eq!(inbox.len(), 1);
    assert_eq!(inbox[0].message_type, "task");
    assert_eq!(inbox[0].payload["nodeId"], "write");

    // Agent reports the first node done (no gates on it) — advances
    // to "verify".
    let run = runtime
        .submit_task_result(
            &run.id,
            "write",
            &agent(&session_id, "owner-1"),
            Some(&json!({"done": true})),
        )
        .await
        .unwrap();
    assert_eq!(run.current_node_id.as_deref(), Some("verify"));
    assert_eq!(run.status, RunStatus::Running);

    // "verify"'s shell gate checks for hello.txt, which doesn't
    // exist yet — must fail and retry, not silently pass.
    let run = runtime
        .submit_task_result(
            &run.id,
            "verify",
            &agent(&session_id, "owner-1"),
            Some(&json!({"done": true})),
        )
        .await
        .unwrap();
    assert_eq!(
        run.status,
        RunStatus::Running,
        "gate failure retries, does not fail the run immediately"
    );
    assert_eq!(run.current_node_id.as_deref(), Some("verify"));

    let detail = runtime.get_run(&run.id).await.unwrap();
    let retry_events: Vec<_> = detail
        .events
        .iter()
        .filter(|e| e.kind == atlas_afg::api::NodeEventKind::Retry)
        .collect();
    assert_eq!(retry_events.len(), 1, "expected exactly one retry event");

    // Now create the file the gate checks for, and resubmit — this
    // time the gate passes and the run completes (no more nodes).
    std::fs::write(dir.path().join("hello.txt"), "hi").unwrap();
    let run = runtime
        .submit_task_result(
            &run.id,
            "verify",
            &agent(&session_id, "owner-1"),
            Some(&json!({"done": true})),
        )
        .await
        .unwrap();
    assert_eq!(run.status, RunStatus::Completed);
    assert!(run.completed_at.is_some());

    // The full timeline reads back in chronological order and
    // contains every kind of event this run produced.
    let detail = runtime.get_run(&run.id).await.unwrap();
    let kinds: Vec<_> = detail.events.iter().map(|e| e.kind).collect();
    assert!(kinds.contains(&atlas_afg::api::NodeEventKind::Enter));
    assert!(kinds.contains(&atlas_afg::api::NodeEventKind::Exec));
    assert!(kinds.contains(&atlas_afg::api::NodeEventKind::GateFail));
    assert!(kinds.contains(&atlas_afg::api::NodeEventKind::GatePass));
    assert!(kinds.contains(&atlas_afg::api::NodeEventKind::Retry));
    assert!(kinds.contains(&atlas_afg::api::NodeEventKind::Advance));
    assert!(kinds.contains(&atlas_afg::api::NodeEventKind::Complete));
}

#[tokio::test]
async fn exhausting_retries_fails_the_run() {
    let (runtime, _messages, session_id) = fresh_runtime().await;
    let dir = TempDir::new().unwrap();
    let project_root = dir.path().to_str().unwrap();

    let yaml = "
name: always-fails
nodes:
  - id: only
    title: Only node
    instructions: this will never satisfy the gate
    maxRetries: 1
    acceptanceCriteria:
      - type: shell
        command: \"exit 1\"
";
    let workflow = runtime
        .register_workflow("your-org/your-project", project_root, None, Some(yaml))
        .await
        .unwrap();
    let run = runtime
        .start_run(
            &workflow.id,
            &session_id,
            None,
            &agent(&session_id, "owner-1"),
        )
        .await
        .unwrap();

    let run = runtime
        .submit_task_result(
            &run.id,
            "only",
            &agent(&session_id, "owner-1"),
            Some(&json!({})),
        )
        .await
        .unwrap();
    assert_eq!(run.status, RunStatus::Running, "first failure retries");

    let run = runtime
        .submit_task_result(
            &run.id,
            "only",
            &agent(&session_id, "owner-1"),
            Some(&json!({})),
        )
        .await
        .unwrap();
    assert_eq!(
        run.status,
        RunStatus::Failed,
        "max_retries exhausted must fail the run"
    );
}

#[tokio::test]
async fn register_workflow_upserts_by_name() {
    let (runtime, _messages, _session_id) = fresh_runtime().await;
    let dir = TempDir::new().unwrap();
    let project_root = dir.path().to_str().unwrap();

    let v1 = runtime
        .register_workflow(
            "your-org/your-project",
            project_root,
            None,
            Some("name: same\nnodes:\n  - id: a\n    title: A\n    instructions: v1\n"),
        )
        .await
        .unwrap();
    assert_eq!(v1.version, 1);

    let v2 = runtime
        .register_workflow(
            "your-org/your-project",
            project_root,
            None,
            Some("name: same\nnodes:\n  - id: a\n    title: A\n    instructions: v2\n"),
        )
        .await
        .unwrap();
    assert_eq!(v2.id, v1.id, "same (project, name) updates in place");
    assert_eq!(v2.version, 2);
    assert_eq!(v2.spec.nodes[0].instructions, "v2");
}

#[tokio::test]
async fn start_run_with_unknown_session_is_not_found() {
    let (runtime, _messages, _session_id) = fresh_runtime().await;
    let dir = TempDir::new().unwrap();
    let project_root = dir.path().to_str().unwrap();

    let workflow = runtime
        .register_workflow(
            "your-org/your-project",
            project_root,
            None,
            Some("name: t\nnodes:\n  - id: a\n    title: A\n    instructions: a\n"),
        )
        .await
        .unwrap();

    let err = runtime
        .start_run(
            &workflow.id,
            "nonexistent-session",
            None,
            &agent("nonexistent-session", "owner-1"),
        )
        .await
        .unwrap_err();
    assert!(matches!(err, atlas_afg::api::AfgError::NotFound("session")));
}

/// The defect this authorisation exists for.
///
/// `submit_task_result` used to take `run_id`, `node_id` and
/// `project_root` and check none of them against anything. Verified
/// against a real container before the fix: an unrelated session
/// completed another's run, and the `project_root` it named became the
/// working directory of the shell that run's acceptance gate executed
/// in.
#[tokio::test]
async fn a_result_from_a_session_the_node_was_not_sent_to_is_refused() {
    let (runtime, _messages, session_id) = fresh_runtime().await;
    let dir = TempDir::new().unwrap();
    let project_root = dir.path().to_str().unwrap();

    let workflow = runtime
        .register_workflow(
            "your-org/your-project",
            project_root,
            None,
            Some(TWO_NODE_YAML),
        )
        .await
        .unwrap();
    let run = runtime
        .start_run(
            &workflow.id,
            &session_id,
            None,
            &agent(&session_id, "owner-1"),
        )
        .await
        .unwrap();

    let err = runtime
        .submit_task_result(
            &run.id,
            "write",
            &agent("01SOMEONEELSESESSION", "owner-1"),
            Some(&json!({"done": true})),
        )
        .await
        .unwrap_err();

    // Not-found rather than forbidden: a caller who may not touch this
    // run must not learn that it exists. Same rule as the live view.
    assert!(
        matches!(err, AfgError::NotFound("run")),
        "expected a not-found, got {err:?}"
    );

    // And the run did not move.
    let unchanged = runtime.get_run(&run.id).await.unwrap();
    assert_eq!(unchanged.run.current_node_id.as_deref(), Some("write"));
    assert_eq!(unchanged.run.status, RunStatus::Running);
}

#[tokio::test]
async fn a_caller_with_no_session_cannot_submit_at_all() {
    let (runtime, _messages, session_id) = fresh_runtime().await;
    let dir = TempDir::new().unwrap();
    let project_root = dir.path().to_str().unwrap();

    let workflow = runtime
        .register_workflow(
            "your-org/your-project",
            project_root,
            None,
            Some(TWO_NODE_YAML),
        )
        .await
        .unwrap();
    let run = runtime
        .start_run(
            &workflow.id,
            &session_id,
            None,
            &agent(&session_id, "owner-1"),
        )
        .await
        .unwrap();

    // The shared MCP token identifies a permitted client, not a
    // session. It has no business answering for one.
    let err = runtime
        .submit_task_result(&run.id, "write", &Caller::Local, Some(&json!({})))
        .await
        .unwrap_err();
    assert!(matches!(err, AfgError::NotFound("run")), "{err:?}");
}

#[tokio::test]
async fn a_gate_runs_where_the_workflow_was_registered_not_where_a_caller_says() {
    let (runtime, _messages, session_id) = fresh_runtime().await;
    let registered = TempDir::new().unwrap();
    let elsewhere = TempDir::new().unwrap();

    // A gate that passes only in the directory the workflow was
    // registered against. There is no longer any parameter that could
    // point it at `elsewhere`, which is the point of the test: the
    // caller has lost the ability to choose.
    std::fs::write(registered.path().join("marker"), "here").unwrap();
    let yaml = r#"
name: rooted
nodes:
  - id: only
    title: Only
    instructions: go
    acceptanceCriteria:
      - type: shell
        command: "test -f marker"
"#;

    let workflow = runtime
        .register_workflow(
            "your-org/your-project",
            registered.path().to_str().unwrap(),
            None,
            Some(yaml),
        )
        .await
        .unwrap();
    let run = runtime
        .start_run(
            &workflow.id,
            &session_id,
            None,
            &agent(&session_id, "owner-1"),
        )
        .await
        .unwrap();

    let run = runtime
        .submit_task_result(
            &run.id,
            "only",
            &agent(&session_id, "owner-1"),
            Some(&json!({})),
        )
        .await
        .unwrap();

    assert_eq!(
        run.status,
        RunStatus::Completed,
        "the gate did not run in the registered root"
    );
    assert!(
        !elsewhere.path().join("marker").exists(),
        "the fixture is wrong: the other directory has the marker too"
    );
}

const SCOPED_YAML: &str = r"
name: scoped
nodes:
  - id: only
    title: Run the tests
    instructions: run them
    allowedTools:
      - tracker.*
      - notes.write
";

#[tokio::test]
async fn a_node_that_declares_tools_scopes_the_session_executing_it() {
    let (runtime, _messages, session_id) = fresh_runtime().await;
    let dir = TempDir::new().unwrap();

    let workflow = runtime
        .register_workflow(
            "your-org/your-project",
            dir.path().to_str().unwrap(),
            None,
            Some(SCOPED_YAML),
        )
        .await
        .unwrap();
    runtime
        .start_run(
            &workflow.id,
            &session_id,
            None,
            &agent(&session_id, "owner-1"),
        )
        .await
        .unwrap();

    let scope = runtime
        .tool_scope_for_session(&session_id)
        .await
        .unwrap()
        .expect("the session is mid-node and should be scoped");

    assert!(scope.permits("tracker.close_issue"));
    assert!(scope.permits("notes.write"));
    assert!(!scope.permits("terminal.write"));
    assert!(!scope.permits("notes.delete"));
    // Otherwise the agent cannot say it finished and the run stalls.
    assert!(scope.permits("afg.submit_task_result"));
}

#[tokio::test]
async fn a_node_that_declares_nothing_scopes_nothing() {
    // A workflow written before tool scoping existed must behave
    // exactly as it did.
    let (runtime, _messages, session_id) = fresh_runtime().await;
    let dir = TempDir::new().unwrap();

    let workflow = runtime
        .register_workflow(
            "your-org/your-project",
            dir.path().to_str().unwrap(),
            None,
            Some(TWO_NODE_YAML),
        )
        .await
        .unwrap();
    runtime
        .start_run(
            &workflow.id,
            &session_id,
            None,
            &agent(&session_id, "owner-1"),
        )
        .await
        .unwrap();

    let scope = runtime
        .tool_scope_for_session(&session_id)
        .await
        .unwrap()
        .expect("mid-node");
    assert!(scope.is_unrestricted());
    assert!(scope.permits("terminal.write"));
}

#[tokio::test]
async fn a_session_executing_nothing_has_no_scope() {
    let (runtime, _messages, session_id) = fresh_runtime().await;
    assert!(
        runtime
            .tool_scope_for_session(&session_id)
            .await
            .unwrap()
            .is_none(),
        "a session with no run was scoped"
    );
}

#[tokio::test]
async fn a_finished_run_stops_scoping_its_session() {
    // The scope belongs to the node being executed, not to the
    // session; when the run completes the agent gets its tools back.
    let (runtime, _messages, session_id) = fresh_runtime().await;
    let dir = TempDir::new().unwrap();

    let workflow = runtime
        .register_workflow(
            "your-org/your-project",
            dir.path().to_str().unwrap(),
            None,
            Some(SCOPED_YAML),
        )
        .await
        .unwrap();
    let run = runtime
        .start_run(
            &workflow.id,
            &session_id,
            None,
            &agent(&session_id, "owner-1"),
        )
        .await
        .unwrap();

    assert!(
        runtime
            .tool_scope_for_session(&session_id)
            .await
            .unwrap()
            .is_some()
    );

    runtime
        .submit_task_result(
            &run.id,
            "only",
            &agent(&session_id, "owner-1"),
            Some(&json!({})),
        )
        .await
        .unwrap();

    assert!(
        runtime
            .tool_scope_for_session(&session_id)
            .await
            .unwrap()
            .is_none(),
        "a completed run kept scoping its session"
    );
}

/// A result may only answer the node that was actually dispatched.
///
/// The run records which node it sent out. Reading the node from the
/// caller's parameter instead means an agent asked for one step can
/// report on another — running that other node's gates, and recording
/// its `advance` — which is the definition of skipping a step.
#[tokio::test]
async fn a_result_for_a_node_that_was_not_dispatched_is_refused() {
    let (runtime, _messages, session_id) = fresh_runtime().await;
    let dir = TempDir::new().unwrap();
    let project_root = dir.path().to_str().unwrap();

    let workflow = runtime
        .register_workflow(
            "your-org/your-project",
            project_root,
            None,
            Some(TWO_NODE_YAML),
        )
        .await
        .unwrap();
    let run = runtime
        .start_run(
            &workflow.id,
            &session_id,
            None,
            &agent(&session_id, "owner-1"),
        )
        .await
        .unwrap();

    // "write" is what was dispatched. Answer for "verify" instead.
    assert_eq!(run.current_node_id.as_deref(), Some("write"));
    let err = runtime
        .submit_task_result(
            &run.id,
            "verify",
            &agent(&session_id, "owner-1"),
            Some(&json!({"done": true})),
        )
        .await
        .unwrap_err();

    assert!(
        matches!(err, AfgError::Validation(_)),
        "a result for an undispatched node was accepted: {err:?}"
    );

    // And the run did not move.
    let after = runtime.get_run(&run.id).await.unwrap().run;
    assert_eq!(after.current_node_id.as_deref(), Some("write"));
}
