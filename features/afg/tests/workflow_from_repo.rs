//! The repository is the source of truth for a workflow.
//!
//! A team agrees on how work advances by committing a file. These
//! tests hold the two halves of that promise against each other:
//! starting a run must read the file as it is **now**, and a run
//! already in flight must keep the rules it was dispatched under.
//!
//! Both halves are needed. Without the first, a teammate who pulled a
//! changed workflow silently runs the old one. Without the second,
//! committing a change rewrites the criteria an agent is already being
//! judged against.
//!
//! What the agent was told is read off the coordination bus rather
//! than out of the database, because that is the only copy of the
//! rules the agent ever sees.

use atlas_afg::api::{AfgError, AfgRuntime, AfgStore, RunStatus, run_migrations as run_afg};
use atlas_messaging::api::{MessageStore, SqlitePool, run_migrations as run_messaging};
use atlas_sessions::api::{Caller, NewSession, SessionStore, run_migrations as run_sessions};
use serde_json::json;
use tempfile::TempDir;

const PROJECT: &str = "your-org/your-project";
const OWNER: &str = "owner-1";
const WORKFLOW_PATH: &str = ".atlas/workflows/ship.yaml";

struct Harness {
    runtime: AfgRuntime,
    store: AfgStore,
    messages: MessageStore,
    session_id: String,
}

impl Harness {
    fn caller(&self) -> Caller {
        Caller::Session {
            session_id: self.session_id.clone(),
            owner_id: OWNER.to_owned(),
        }
    }

    /// The title of the node most recently dispatched — what the agent
    /// was actually told to do.
    async fn last_dispatched_title(&self) -> String {
        let inbox = self
            .messages
            .read_inbox(PROJECT, &self.session_id, None, None)
            .await
            .unwrap();
        inbox
            .iter()
            .rev()
            .find(|m| m.message_type == "task")
            .expect("nothing was dispatched")
            .payload["title"]
            .as_str()
            .expect("a task with no title")
            .to_owned()
    }
}

async fn harness() -> Harness {
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    run_afg(&pool).await.unwrap();
    run_messaging(&pool).await.unwrap();
    run_sessions(&pool).await.unwrap();

    let sessions = SessionStore::new(pool.clone());
    let messages = MessageStore::new(pool.clone());
    let store = AfgStore::new(pool);
    let session = sessions
        .create(NewSession {
            project: PROJECT.to_owned(),
            owner_id: OWNER.to_owned(),
            remote_url: "git@github.com:you/your-project.git".to_owned(),
            branch: "main".to_owned(),
            relative_path: PROJECT.to_owned(),
            agent_kind: None,
            title: None,
            resume_command: None,
        })
        .await
        .unwrap();

    Harness {
        runtime: AfgRuntime::new(store.clone(), sessions, messages.clone()),
        store,
        messages,
        session_id: session.id.0,
    }
}

/// One node, titled — so two revisions of the same file are told apart
/// by what the agent was asked to do.
fn one_node(title: &str) -> String {
    format!("\nname: ship\nnodes:\n  - id: only\n    title: {title}\n    instructions: do it\n")
}

/// Two nodes, the second titled — so a change committed *during* a run
/// would show up on the second dispatch if pinning were absent.
fn two_nodes(second_title: &str) -> String {
    format!(
        "
name: ship
nodes:
  - id: first
    title: First
    instructions: do the first thing
  - id: second
    title: {second_title}
    instructions: do the second thing
    dependsOn: [first]
"
    )
}

fn commit_workflow(root: &TempDir, body: &str) {
    let path = root.path().join(WORKFLOW_PATH);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, body).unwrap();
}

#[tokio::test]
async fn starting_a_run_reads_the_file_as_it_is_now() {
    // The case this whole change exists for: a teammate pulls a
    // changed workflow and runs the changed one, without having to
    // remember to re-register it.
    let h = harness().await;
    let dir = TempDir::new().unwrap();
    let root = dir.path().to_str().unwrap();

    commit_workflow(&dir, &one_node("Old title"));
    let workflow = h
        .runtime
        .register_workflow(PROJECT, root, Some(WORKFLOW_PATH), None)
        .await
        .unwrap();
    assert_eq!(workflow.spec.nodes[0].title, "Old title");

    // Somebody edits the workflow and commits; this checkout pulls it.
    commit_workflow(&dir, &one_node("New title"));

    h.runtime
        .start_run(&workflow.id, &h.session_id, None, &h.caller())
        .await
        .unwrap();

    assert_eq!(
        h.last_dispatched_title().await,
        "New title",
        "the run dispatched the registered copy instead of the file"
    );
}

#[tokio::test]
async fn a_run_already_in_flight_keeps_the_rules_it_started_under() {
    let h = harness().await;
    let dir = TempDir::new().unwrap();
    let root = dir.path().to_str().unwrap();

    commit_workflow(&dir, &two_nodes("As dispatched"));
    let workflow = h
        .runtime
        .register_workflow(PROJECT, root, Some(WORKFLOW_PATH), None)
        .await
        .unwrap();
    let run = h
        .runtime
        .start_run(&workflow.id, &h.session_id, None, &h.caller())
        .await
        .unwrap();
    assert_eq!(h.last_dispatched_title().await, "First");

    // The file changes *while the first agent is working*, and a
    // second run starts — which is what actually refreshes the stored
    // workflow. Two developers running the same workflow on the same
    // day is the ordinary case, not a corner one.
    commit_workflow(&dir, &two_nodes("Changed underneath"));
    h.runtime
        .start_run(&workflow.id, &h.session_id, None, &h.caller())
        .await
        .unwrap();
    assert_eq!(
        h.last_dispatched_title().await,
        "First",
        "the second run did not start from the top"
    );

    // Now the first agent reports back. Judging it against criteria
    // that arrived after it was dispatched would be moving the
    // goalposts mid-run.
    h.runtime
        .submit_task_result(&run.id, "first", &h.caller(), Some(&json!({})))
        .await
        .unwrap();

    assert_eq!(
        h.last_dispatched_title().await,
        "As dispatched",
        "an edit rewrote a run that was already in flight"
    );
}

#[tokio::test]
async fn a_broken_file_fails_the_start_instead_of_running_the_old_copy() {
    // Falling back to the stored spec would run rules the repository
    // no longer contains — silently, which is the worst version of it.
    let h = harness().await;
    let dir = TempDir::new().unwrap();
    let root = dir.path().to_str().unwrap();

    commit_workflow(&dir, &one_node("Fine"));
    let workflow = h
        .runtime
        .register_workflow(PROJECT, root, Some(WORKFLOW_PATH), None)
        .await
        .unwrap();

    commit_workflow(&dir, "nodes: [ this is not\n  valid yaml");

    let err = h
        .runtime
        .start_run(&workflow.id, &h.session_id, None, &h.caller())
        .await
        .unwrap_err();
    assert!(
        matches!(err, AfgError::Parse(_) | AfgError::Validation(_)),
        "expected the broken file to be reported, got {err:?}"
    );
}

#[tokio::test]
async fn a_deleted_file_fails_the_start() {
    let h = harness().await;
    let dir = TempDir::new().unwrap();
    let root = dir.path().to_str().unwrap();

    commit_workflow(&dir, &one_node("Fine"));
    let workflow = h
        .runtime
        .register_workflow(PROJECT, root, Some(WORKFLOW_PATH), None)
        .await
        .unwrap();
    std::fs::remove_file(dir.path().join(WORKFLOW_PATH)).unwrap();

    let err = h
        .runtime
        .start_run(&workflow.id, &h.session_id, None, &h.caller())
        .await
        .unwrap_err();
    assert!(
        matches!(err, AfgError::Io(_)),
        "expected the missing file to be reported, got {err:?}"
    );
}

#[tokio::test]
async fn an_unchanged_file_does_not_bump_the_version() {
    // Otherwise `version` counts runs rather than revisions, and stops
    // answering the only question it is there for: has this workflow
    // changed since I last looked?
    let h = harness().await;
    let dir = TempDir::new().unwrap();
    let root = dir.path().to_str().unwrap();

    commit_workflow(&dir, &one_node("Stable"));
    let workflow = h
        .runtime
        .register_workflow(PROJECT, root, Some(WORKFLOW_PATH), None)
        .await
        .unwrap();

    for _ in 0..3 {
        let run = h
            .runtime
            .start_run(&workflow.id, &h.session_id, None, &h.caller())
            .await
            .unwrap();
        h.runtime
            .submit_task_result(&run.id, "only", &h.caller(), Some(&json!({})))
            .await
            .unwrap();
    }

    assert_eq!(
        h.store.get_workflow(&workflow.id).await.unwrap().version,
        workflow.version
    );
}

#[tokio::test]
async fn a_changed_file_does_bump_the_version() {
    let h = harness().await;
    let dir = TempDir::new().unwrap();
    let root = dir.path().to_str().unwrap();

    commit_workflow(&dir, &one_node("Before"));
    let workflow = h
        .runtime
        .register_workflow(PROJECT, root, Some(WORKFLOW_PATH), None)
        .await
        .unwrap();

    commit_workflow(&dir, &one_node("After"));
    h.runtime
        .start_run(&workflow.id, &h.session_id, None, &h.caller())
        .await
        .unwrap();

    assert!(h.store.get_workflow(&workflow.id).await.unwrap().version > workflow.version);
}

#[tokio::test]
async fn a_workflow_registered_inline_still_runs_without_a_file() {
    // Inline YAML is for one-offs and experiments, where committing a
    // file would be noise. It has no source to re-read, and that is
    // not an error.
    let h = harness().await;
    let dir = TempDir::new().unwrap();
    let root = dir.path().to_str().unwrap();

    let workflow = h
        .runtime
        .register_workflow(PROJECT, root, None, Some(&one_node("Inline")))
        .await
        .unwrap();

    let run = h
        .runtime
        .start_run(&workflow.id, &h.session_id, None, &h.caller())
        .await
        .unwrap();
    assert_eq!(run.status, RunStatus::Running);
    assert_eq!(h.last_dispatched_title().await, "Inline");
}
