//! Integration tests for `live` mode over SSE
//! transport. Real bound servers and real held-open streams — an SSE
//! connection is exactly the thing a `oneshot` request cannot model,
//! so nothing here uses that pattern.

use std::time::Duration;

use atlas_auth::api::{AuthStore, SqlitePool as AuthPool, run_migrations as run_auth_migrations};
use atlas_sessions::api::{
    NewSession, SessionStore, SqlitePool as SessionsPool, run_migrations as run_sessions_migrations,
};
use atlas_sync::api::{ChangeKind, LiveConfig, SyncClient, SyncMode, SyncSupervisor, router};
use futures_util::StreamExt;

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
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let remote = build_remote(format!("http://{addr}/")).await;
    let app = router(
        remote.auth.clone(),
        remote.sessions.clone(),
        fresh_memory_store().await,
        fresh_note_store().await,
    );
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    remote
}

async fn build_remote(base_url: String) -> RemoteServer {
    let pool = AuthPool::connect("sqlite::memory:").await.unwrap();
    run_auth_migrations(&pool).await.unwrap();
    run_sessions_migrations(&pool).await.unwrap();
    RemoteServer {
        base_url,
        auth: AuthStore::new(pool.clone()),
        sessions: SessionStore::new(pool),
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

/// Waits until `store` holds at least one session for `owner_id`, or
/// the deadline passes. Returns how many it saw (0 on timeout).
///
/// This polls the *assertion*, not the system under test — nothing
/// here triggers a sync, so a non-zero result can only come from the
/// live stream having done its job.
async fn wait_for_session(store: &SessionStore, owner_id: &str, deadline: Duration) -> usize {
    let end = tokio::time::Instant::now() + deadline;
    loop {
        let rows = store.list_since(owner_id, None).await.unwrap();
        if !rows.is_empty() || tokio::time::Instant::now() >= end {
            return rows.len();
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

async fn wait_until_connected(supervisor: &SyncSupervisor, deadline: Duration) -> bool {
    let end = tokio::time::Instant::now() + deadline;
    loop {
        if supervisor.status().await.stream_connected == Some(true) {
            return true;
        }
        if tokio::time::Instant::now() >= end {
            return false;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

#[tokio::test]
async fn a_push_from_another_machine_reaches_a_live_client_without_any_polling() {
    let remote = spawn_remote().await;
    let issued = remote
        .auth
        .register("dev@example.com", "correct horse battery", "Dev")
        .await
        .unwrap();
    let owner_id = issued.user.id.0.clone();

    // The developer's second machine: empty, in live mode.
    let laptop = fresh_local_store().await;
    let supervisor = SyncSupervisor::new(
        laptop.clone(),
        fresh_memory_store().await,
        fresh_note_store().await,
    );
    supervisor
        .set_live(LiveConfig {
            remote_url: remote.base_url.clone(),
            bearer_token: issued.token.clone(),
            owner_id: owner_id.clone(),
        })
        .await
        .unwrap();

    assert!(
        wait_until_connected(&supervisor, Duration::from_secs(5)).await,
        "the live stream never reported itself connected"
    );
    // The connect-time pass has already run and found nothing. Anything
    // that arrives from here on can only have come through the stream.
    assert!(laptop.list_since(&owner_id, None).await.unwrap().is_empty());

    // The developer's first machine pushes a session.
    let desktop = fresh_local_store().await;
    desktop
        .create(sample(&owner_id, "desktop-workspace"))
        .await
        .unwrap();
    SyncClient::new(&remote.base_url)
        .unwrap()
        .sync_sessions(&issued.token, &owner_id, &desktop)
        .await
        .unwrap();

    let count = wait_for_session(&laptop, &owner_id, Duration::from_secs(5)).await;
    assert_eq!(
        count, 1,
        "the live client never converged — no poll exists to save it here"
    );
}

#[tokio::test]
async fn the_event_stream_carries_only_the_authenticated_users_own_changes() {
    let remote = spawn_remote().await;
    let alice = remote
        .auth
        .register("alice@example.com", "correct horse battery", "Alice")
        .await
        .unwrap();
    // register() is bootstrap-only, so the second account is invited.
    let invited = remote
        .auth
        .invite_user("bob@example.com", "Bob")
        .await
        .unwrap();
    let bob = remote
        .auth
        .login("bob@example.com", &invited.temporary_password)
        .await
        .unwrap();

    let client = SyncClient::new(&remote.base_url).unwrap();
    let stream = client.events(&alice.token).await.unwrap();
    let mut stream = Box::pin(stream);

    // Bob pushes. Alice's stream must stay silent.
    let bobs_machine = fresh_local_store().await;
    bobs_machine
        .create(sample(&bob.user.id.0, "bobs-workspace"))
        .await
        .unwrap();
    SyncClient::new(&remote.base_url)
        .unwrap()
        .sync_sessions(&bob.token, &bob.user.id.0, &bobs_machine)
        .await
        .unwrap();

    let leaked = tokio::time::timeout(Duration::from_secs(1), stream.next()).await;
    assert!(
        leaked.is_err(),
        "another user's push produced an event on this stream"
    );

    // Alice pushes. Now her stream must fire.
    let alices_machine = fresh_local_store().await;
    alices_machine
        .create(sample(&alice.user.id.0, "alices-workspace"))
        .await
        .unwrap();
    SyncClient::new(&remote.base_url)
        .unwrap()
        .sync_sessions(&alice.token, &alice.user.id.0, &alices_machine)
        .await
        .unwrap();

    let event = tokio::time::timeout(Duration::from_secs(5), stream.next())
        .await
        .expect("the stream stayed silent on the owner's own push")
        .expect("the stream ended instead of delivering")
        .expect("the stream yielded an error");
    assert_eq!(event, ChangeKind::Sessions);
}

#[tokio::test]
async fn the_event_stream_rejects_a_missing_or_invalid_bearer() {
    let remote = spawn_remote().await;
    let client = SyncClient::new(&remote.base_url).unwrap();

    // `impl Stream` isn't `Debug`, so match rather than `unwrap_err`.
    let Err(err) = client.events("not-a-real-token").await else {
        panic!("the stream opened with an invalid bearer token");
    };
    assert!(matches!(err, atlas_sync::api::SyncError::Unauthorized));
}

#[tokio::test]
async fn switching_from_live_to_focus_closes_the_stream_and_stops_convergence() {
    let remote = spawn_remote().await;
    let issued = remote
        .auth
        .register("dev@example.com", "correct horse battery", "Dev")
        .await
        .unwrap();
    let owner_id = issued.user.id.0.clone();

    let laptop = fresh_local_store().await;
    let supervisor = SyncSupervisor::new(
        laptop.clone(),
        fresh_memory_store().await,
        fresh_note_store().await,
    );
    supervisor
        .set_live(LiveConfig {
            remote_url: remote.base_url.clone(),
            bearer_token: issued.token.clone(),
            owner_id: owner_id.clone(),
        })
        .await
        .unwrap();
    assert!(wait_until_connected(&supervisor, Duration::from_secs(5)).await);

    supervisor.set_focus().await;
    let status = supervisor.status().await;
    assert_eq!(status.mode, SyncMode::Focus);
    assert_eq!(
        status.stream_connected, None,
        "focus mode should report no stream at all, not a disconnected one"
    );

    // A push from elsewhere must now go unnoticed.
    let desktop = fresh_local_store().await;
    desktop
        .create(sample(&owner_id, "after-focus"))
        .await
        .unwrap();
    SyncClient::new(&remote.base_url)
        .unwrap()
        .sync_sessions(&issued.token, &owner_id, &desktop)
        .await
        .unwrap();

    tokio::time::sleep(Duration::from_secs(2)).await;
    assert!(
        laptop.list_since(&owner_id, None).await.unwrap().is_empty(),
        "a session converged after switching to focus — the stream was not closed"
    );
}

#[tokio::test]
async fn live_mode_waits_out_a_remote_that_is_not_up_yet_and_converges_once_it_appears() {
    // Claim a port, then release it, so we can point `live` mode at a
    // server that does not exist yet and start it later.
    let probe = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = probe.local_addr().unwrap();
    drop(probe);

    let base_url = format!("http://{addr}/");
    let remote = build_remote(base_url.clone()).await;
    let issued = remote
        .auth
        .register("dev@example.com", "correct horse battery", "Dev")
        .await
        .unwrap();
    let owner_id = issued.user.id.0.clone();

    let laptop = fresh_local_store().await;
    let supervisor = SyncSupervisor::new(
        laptop.clone(),
        fresh_memory_store().await,
        fresh_note_store().await,
    );

    // Arming live mode against a remote that is down must succeed —
    // failing here would make the mode unavailable exactly when the
    // server is having a bad day.
    supervisor
        .set_live(LiveConfig {
            remote_url: base_url,
            bearer_token: issued.token.clone(),
            owner_id: owner_id.clone(),
        })
        .await
        .unwrap();

    tokio::time::sleep(Duration::from_millis(500)).await;
    let status = supervisor.status().await;
    assert_eq!(status.mode, SyncMode::Live);
    assert_eq!(
        status.stream_connected,
        Some(false),
        "a live stream that has never connected must report itself disconnected"
    );

    // The server comes up on the port the client has been retrying.
    let app = router(
        remote.auth.clone(),
        remote.sessions.clone(),
        fresh_memory_store().await,
        fresh_note_store().await,
    );
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    assert!(
        wait_until_connected(&supervisor, Duration::from_secs(10)).await,
        "the client never reconnected once the remote came up"
    );

    // And it works: a push from elsewhere converges.
    let desktop = fresh_local_store().await;
    desktop
        .create(sample(&owner_id, "after-reconnect"))
        .await
        .unwrap();
    SyncClient::new(&remote.base_url)
        .unwrap()
        .sync_sessions(&issued.token, &owner_id, &desktop)
        .await
        .unwrap();

    let count = wait_for_session(&laptop, &owner_id, Duration::from_secs(5)).await;
    assert_eq!(count, 1);
}

// ---- memory replication ---------------------------------

async fn fresh_memory() -> atlas_memory::api::MemoryStore {
    fresh_memory_store().await
}

/// Builds a remote whose sync router is backed by a memory store the
/// test can also inspect directly.
async fn spawn_remote_with_memory() -> (RemoteServer, atlas_memory::api::MemoryStore) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let remote = build_remote(format!("http://{addr}/")).await;
    let memory = fresh_memory().await;
    let app = router(
        remote.auth.clone(),
        remote.sessions.clone(),
        memory.clone(),
        fresh_note_store().await,
    );
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    (remote, memory)
}

#[tokio::test]
async fn project_memory_replicates_but_personal_memory_stays_with_its_owner() {
    let (remote, remote_memory) = spawn_remote_with_memory().await;
    let alice = remote
        .auth
        .register("alice@example.com", "correct horse battery", "Alice")
        .await
        .unwrap();
    let invited = remote
        .auth
        .invite_user("bob@example.com", "Bob")
        .await
        .unwrap();
    let bob = remote
        .auth
        .login("bob@example.com", &invited.temporary_password)
        .await
        .unwrap();

    // Alice's machine remembers one of each kind and syncs.
    let alices = fresh_memory().await;
    alices
        .remember_project(
            "your-org/your-project",
            "build",
            &serde_json::json!("cargo build"),
        )
        .await
        .unwrap();
    alices
        .remember_personal(&alice.user.id.0, "diary", &serde_json::json!("mine alone"))
        .await
        .unwrap();
    SyncClient::new(&remote.base_url)
        .unwrap()
        .sync_memory(&alice.token, &alice.user.id.0, &alices)
        .await
        .unwrap();

    // Both landed on the remote, each in its own bucket.
    assert_eq!(
        remote_memory
            .recall_project("your-org/your-project", "build")
            .await
            .unwrap()
            .value,
        serde_json::json!("cargo build")
    );
    assert_eq!(
        remote_memory
            .recall_personal(&alice.user.id.0, "diary")
            .await
            .unwrap()
            .value,
        serde_json::json!("mine alone")
    );

    // Bob syncs from his own empty machine. He must receive the
    // project memory and nothing of Alice's personal memory.
    let bobs = fresh_memory().await;
    SyncClient::new(&remote.base_url)
        .unwrap()
        .sync_memory(&bob.token, &bob.user.id.0, &bobs)
        .await
        .unwrap();

    assert_eq!(
        bobs.recall_project("your-org/your-project", "build")
            .await
            .unwrap()
            .value,
        serde_json::json!("cargo build"),
        "Type 1 memory should replicate freely"
    );
    assert!(
        bobs.list_personal(&bob.user.id.0).await.unwrap().is_empty(),
        "Alice's personal memory reached Bob's machine"
    );
    assert!(
        bobs.list_personal(&alice.user.id.0)
            .await
            .unwrap()
            .is_empty(),
        "Alice's personal memory reached Bob's machine under her own id"
    );
}

#[tokio::test]
async fn a_client_cannot_push_personal_memory_under_another_owner() {
    let (remote, remote_memory) = spawn_remote_with_memory().await;
    let alice = remote
        .auth
        .register("alice@example.com", "correct horse battery", "Alice")
        .await
        .unwrap();
    let invited = remote
        .auth
        .invite_user("bob@example.com", "Bob")
        .await
        .unwrap();
    let bob = remote
        .auth
        .login("bob@example.com", &invited.temporary_password)
        .await
        .unwrap();

    // Bob's machine holds a row claiming to be Alice's, and Bob syncs
    // with his own bearer. The server must file it under Bob.
    let bobs = fresh_memory().await;
    bobs.remember_personal(&alice.user.id.0, "planted", &serde_json::json!("not yours"))
        .await
        .unwrap();
    SyncClient::new(&remote.base_url)
        .unwrap()
        .sync_memory(&bob.token, &alice.user.id.0, &bobs)
        .await
        .unwrap();

    assert!(
        remote_memory
            .recall_personal(&alice.user.id.0, "planted")
            .await
            .is_err(),
        "a row was written into another developer's personal memory"
    );
    assert_eq!(
        remote_memory
            .recall_personal(&bob.user.id.0, "planted")
            .await
            .unwrap()
            .value,
        serde_json::json!("not yours"),
        "the row should have been forced under the authenticated owner"
    );
}
