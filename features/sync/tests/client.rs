//! End-to-end `SyncClient` tests against a *real* bound TCP server
//! playing the "team server" role — not `oneshot`. This is the
//! actual shape of `focus`-mode sync: two independent `SessionStore`s
//! (one local, one remote) reconciled over real HTTP.

use atlas_auth::api::{AuthStore, SqlitePool as AuthPool, run_migrations as run_auth_migrations};
use atlas_sessions::api::{
    NewSession, SessionStatus, SessionStore, SqlitePool as SessionsPool,
    run_migrations as run_sessions_migrations,
};
use atlas_sync::api::{SyncClient, router};

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

struct RemoteServer {
    base_url: String,
    auth: AuthStore,
    sessions: SessionStore,
}

async fn spawn_remote() -> RemoteServer {
    let pool = AuthPool::connect("sqlite::memory:").await.unwrap();
    run_auth_migrations(&pool).await.unwrap();
    run_sessions_migrations(&pool).await.unwrap();
    let auth = AuthStore::new(pool.clone());
    let sessions = SessionStore::new(pool);

    let app = router(
        auth.clone(),
        sessions.clone(),
        fresh_memory_store().await,
        fresh_note_store().await,
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    RemoteServer {
        base_url: format!("http://{addr}/"),
        auth,
        sessions,
    }
}

async fn fresh_local_store() -> SessionStore {
    let pool = SessionsPool::connect("sqlite::memory:").await.unwrap();
    run_sessions_migrations(&pool).await.unwrap();
    SessionStore::new(pool)
}

fn sample(owner_id: &str, relative_path: &str) -> NewSession {
    NewSession {
        project: "your-org/your-project".to_owned(),
        owner_id: owner_id.to_owned(),
        remote_url: "git@github.com:you/your-project.git".to_owned(),
        branch: "main".to_owned(),
        relative_path: relative_path.to_owned(),
        agent_kind: None,
        title: None,
        resume_command: None,
    }
}

#[tokio::test]
async fn a_session_created_locally_reaches_the_remote_via_push() {
    let remote = spawn_remote().await;
    let issued = remote
        .auth
        .register("dev@example.com", "correct horse battery", "Dev")
        .await
        .unwrap();
    let owner_id = issued.user.id.0.clone();

    let local = fresh_local_store().await;
    local
        .create(sample(&owner_id, "laptop-workspace"))
        .await
        .unwrap();

    let client = SyncClient::new(&remote.base_url).unwrap();
    let report = client
        .sync_sessions(&issued.token, &owner_id, &local)
        .await
        .unwrap();
    assert_eq!(report.pushed, 1);

    let on_remote = remote.sessions.list_since(&owner_id, None).await.unwrap();
    assert_eq!(on_remote.len(), 1);
    assert_eq!(on_remote[0].relative_path, "laptop-workspace");
}

#[tokio::test]
async fn a_session_created_directly_on_the_remote_reaches_local_via_pull() {
    let remote = spawn_remote().await;
    let issued = remote
        .auth
        .register("dev@example.com", "correct horse battery", "Dev")
        .await
        .unwrap();
    let owner_id = issued.user.id.0.clone();

    // Created straight on the "team server" — e.g. a teammate's own
    // machine synced it there first.
    remote
        .sessions
        .create(sample(&owner_id, "from-teammate"))
        .await
        .unwrap();

    let local = fresh_local_store().await;
    let client = SyncClient::new(&remote.base_url).unwrap();
    let report = client
        .sync_sessions(&issued.token, &owner_id, &local)
        .await
        .unwrap();
    assert_eq!(report.pulled, 1);

    let locally = local.list_since(&owner_id, None).await.unwrap();
    assert_eq!(locally.len(), 1);
    assert_eq!(locally[0].relative_path, "from-teammate");
}

#[tokio::test]
async fn conflicting_edits_converge_to_the_newer_updated_at() {
    let remote = spawn_remote().await;
    let issued = remote
        .auth
        .register("dev@example.com", "correct horse battery", "Dev")
        .await
        .unwrap();
    let owner_id = issued.user.id.0.clone();

    let local = fresh_local_store().await;
    let created = local
        .create(sample(&owner_id, "conflict-workspace"))
        .await
        .unwrap();

    let client = SyncClient::new(&remote.base_url).unwrap();
    // First sync: remote now has the same row.
    client
        .sync_sessions(&issued.token, &owner_id, &local)
        .await
        .unwrap();

    // Remote-side edit happens after the first sync, so it carries
    // a strictly later `updated_at`.
    let remote_winner = remote
        .sessions
        .update_status(&created.id, &owner_id, SessionStatus::Archived)
        .await
        .unwrap();

    // Second sync reconciles: local should pick up the remote's win.
    client
        .sync_sessions(&issued.token, &owner_id, &local)
        .await
        .unwrap();

    let locally = local.get(&created.id, &owner_id).await.unwrap();
    assert_eq!(locally.status, SessionStatus::Archived);
    assert_eq!(locally.updated_at, remote_winner.updated_at);
}

#[tokio::test]
async fn a_bad_bearer_token_is_reported_as_unauthorized() {
    let remote = spawn_remote().await;
    let local = fresh_local_store().await;
    let client = SyncClient::new(&remote.base_url).unwrap();

    let err = client
        .sync_sessions("not-a-real-token", "owner-a", &local)
        .await
        .unwrap_err();
    assert!(matches!(err, atlas_sync::api::SyncError::Unauthorized));
}
