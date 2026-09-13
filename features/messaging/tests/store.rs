//! Integration tests for `MessageStore` against a real (temp-file)
//! `SQLite` database — the same shape `contract_mirror.rs` uses in
//! the tracker crate.

use atlas_messaging::api::{MessageStore, MessagingError, NewMessage, run_migrations};
use serde_json::json;
use sqlx::SqlitePool;
use sqlx::sqlite::SqliteConnectOptions;
use tempfile::TempDir;

async fn fresh_store(dir: &TempDir) -> MessageStore {
    let path = dir.path().join("messages.db");
    let options = SqliteConnectOptions::new()
        .filename(&path)
        .create_if_missing(true);
    let pool = SqlitePool::connect_with(options).await.unwrap();
    run_migrations(&pool).await.unwrap();
    MessageStore::new(pool)
}

fn sample(from: &str, to: Option<&str>) -> NewMessage {
    NewMessage {
        from_session: from.to_owned(),
        to_session: to.map(str::to_owned),
        message_type: "status".to_owned(),
        payload: json!({ "note": "hello" }),
        correlation_id: None,
        reply_to: None,
    }
}

#[tokio::test]
async fn broadcast_is_visible_to_every_reader() {
    let dir = TempDir::new().unwrap();
    let store = fresh_store(&dir).await;

    let sent = store
        .send("your-org/your-project", sample("agent-a", None))
        .await
        .unwrap();
    assert_eq!(sent.to_session, None);

    let inbox_b = store
        .read_inbox("your-org/your-project", "agent-b", None, None)
        .await
        .unwrap();
    assert_eq!(inbox_b.len(), 1);
    assert_eq!(inbox_b[0].id, sent.id);

    let inbox_c = store
        .read_inbox("your-org/your-project", "agent-c", None, None)
        .await
        .unwrap();
    assert_eq!(inbox_c.len(), 1, "every reader sees a broadcast");
}

#[tokio::test]
async fn direct_message_is_visible_only_to_its_recipient() {
    let dir = TempDir::new().unwrap();
    let store = fresh_store(&dir).await;

    store
        .send("your-org/your-project", sample("agent-a", Some("agent-b")))
        .await
        .unwrap();

    let inbox_b = store
        .read_inbox("your-org/your-project", "agent-b", None, None)
        .await
        .unwrap();
    assert_eq!(inbox_b.len(), 1);

    let inbox_c = store
        .read_inbox("your-org/your-project", "agent-c", None, None)
        .await
        .unwrap();
    assert_eq!(
        inbox_c.len(),
        0,
        "a direct message must not leak to a reader it wasn't addressed to"
    );
}

#[tokio::test]
async fn since_cursor_excludes_prior_messages() {
    let dir = TempDir::new().unwrap();
    let store = fresh_store(&dir).await;

    let first = store
        .send("your-org/your-project", sample("agent-a", None))
        .await
        .unwrap();
    let second = store
        .send("your-org/your-project", sample("agent-a", None))
        .await
        .unwrap();

    let all = store
        .read_inbox("your-org/your-project", "agent-b", None, None)
        .await
        .unwrap();
    assert_eq!(all.len(), 2);

    let after_first = store
        .read_inbox("your-org/your-project", "agent-b", Some(&first.id), None)
        .await
        .unwrap();
    assert_eq!(after_first.len(), 1);
    assert_eq!(after_first[0].id, second.id);
}

#[tokio::test]
async fn limit_caps_the_page() {
    let dir = TempDir::new().unwrap();
    let store = fresh_store(&dir).await;

    for _ in 0..5 {
        store
            .send("your-org/your-project", sample("agent-a", None))
            .await
            .unwrap();
    }

    let page = store
        .read_inbox("your-org/your-project", "agent-b", None, Some(2))
        .await
        .unwrap();
    assert_eq!(page.len(), 2);
}

#[tokio::test]
async fn unknown_project_returns_empty_not_error() {
    let dir = TempDir::new().unwrap();
    let store = fresh_store(&dir).await;

    let inbox = store
        .read_inbox("nobody/nothing", "agent-a", None, None)
        .await
        .unwrap();
    assert_eq!(inbox, Vec::new());
}

#[tokio::test]
async fn empty_project_is_rejected() {
    let dir = TempDir::new().unwrap();
    let store = fresh_store(&dir).await;

    let err = store.send("", sample("agent-a", None)).await.unwrap_err();
    assert!(matches!(err, MessagingError::EmptyProject));
}

#[tokio::test]
async fn empty_from_session_is_rejected() {
    let dir = TempDir::new().unwrap();
    let store = fresh_store(&dir).await;

    let err = store
        .send("your-org/your-project", sample("", None))
        .await
        .unwrap_err();
    assert!(matches!(err, MessagingError::EmptyFromSession));
}
