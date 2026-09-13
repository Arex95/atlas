//! Integration tests for `MemoryStore` against a real `SQLite`
//! database — including the schema-level guarantees, which are the
//! point of "first-class column, not a convention".

use atlas_memory::api::{MemoryError, MemoryScope, MemoryStore, SqlitePool, run_migrations};
use serde_json::json;

async fn fresh() -> (MemoryStore, SqlitePool) {
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    run_migrations(&pool).await.unwrap();
    (MemoryStore::new(pool.clone()), pool)
}

#[tokio::test]
async fn project_memory_round_trips() {
    let (store, _) = fresh().await;
    let written = store
        .remember_project(
            "your-org/your-project",
            "build-command",
            &json!("cargo build"),
        )
        .await
        .unwrap();
    assert_eq!(written.scope, MemoryScope::Project);
    assert_eq!(written.project.as_deref(), Some("your-org/your-project"));
    assert!(written.owner_id.is_none());

    let read = store
        .recall_project("your-org/your-project", "build-command")
        .await
        .unwrap();
    assert_eq!(read.value, json!("cargo build"));
}

#[tokio::test]
async fn personal_memory_round_trips_and_is_not_scoped_to_a_project() {
    let (store, _) = fresh().await;
    let written = store
        .remember_personal("owner-1", "prefers", &json!({"tone": "direct"}))
        .await
        .unwrap();
    assert_eq!(written.scope, MemoryScope::Personal);
    assert_eq!(written.owner_id.as_deref(), Some("owner-1"));
    // Personal state belongs to a developer across *any* project.
    assert!(written.project.is_none());

    let read = store.recall_personal("owner-1", "prefers").await.unwrap();
    assert_eq!(read.value["tone"], "direct");
}

#[tokio::test]
async fn writing_the_same_key_twice_updates_rather_than_duplicating() {
    let (store, _) = fresh().await;
    store
        .remember_project("your-org/your-project", "k", &json!(1))
        .await
        .unwrap();
    store
        .remember_project("your-org/your-project", "k", &json!(2))
        .await
        .unwrap();

    let all = store.list_project("your-org/your-project").await.unwrap();
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].value, json!(2));

    store
        .remember_personal("owner-1", "k", &json!("a"))
        .await
        .unwrap();
    store
        .remember_personal("owner-1", "k", &json!("b"))
        .await
        .unwrap();
    let mine = store.list_personal("owner-1").await.unwrap();
    assert_eq!(mine.len(), 1);
    assert_eq!(mine[0].value, json!("b"));
}

#[tokio::test]
async fn one_owners_personal_memory_is_invisible_to_another() {
    let (store, _) = fresh().await;
    store
        .remember_personal("owner-1", "secret", &json!("mine"))
        .await
        .unwrap();

    let err = store
        .recall_personal("owner-2", "secret")
        .await
        .unwrap_err();
    assert!(matches!(err, MemoryError::NotFound));
    assert!(store.list_personal("owner-2").await.unwrap().is_empty());
}

#[tokio::test]
async fn one_projects_memory_is_invisible_to_another() {
    let (store, _) = fresh().await;
    store
        .remember_project("your-org/your-project", "k", &json!("v"))
        .await
        .unwrap();

    let err = store
        .recall_project("your-org/other", "k")
        .await
        .unwrap_err();
    assert!(matches!(err, MemoryError::NotFound));
    assert!(
        store
            .list_project("your-org/other")
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn the_two_buckets_do_not_bleed_into_each_other_on_a_shared_key() {
    let (store, _) = fresh().await;
    store
        .remember_project("your-org/your-project", "same-key", &json!("project value"))
        .await
        .unwrap();
    store
        .remember_personal("owner-1", "same-key", &json!("personal value"))
        .await
        .unwrap();

    assert_eq!(
        store
            .recall_project("your-org/your-project", "same-key")
            .await
            .unwrap()
            .value,
        json!("project value")
    );
    assert_eq!(
        store
            .recall_personal("owner-1", "same-key")
            .await
            .unwrap()
            .value,
        json!("personal value")
    );
    assert_eq!(
        store
            .list_project("your-org/your-project")
            .await
            .unwrap()
            .len(),
        1
    );
    assert_eq!(store.list_personal("owner-1").await.unwrap().len(), 1);
}

#[tokio::test]
async fn forget_removes_only_the_named_key_and_reports_a_miss() {
    let (store, _) = fresh().await;
    store
        .remember_project("your-org/your-project", "keep", &json!(1))
        .await
        .unwrap();
    store
        .remember_project("your-org/your-project", "drop", &json!(2))
        .await
        .unwrap();

    store
        .forget_project("your-org/your-project", "drop")
        .await
        .unwrap();
    let left = store.list_project("your-org/your-project").await.unwrap();
    assert_eq!(left.len(), 1);
    assert_eq!(left[0].key, "keep");

    let err = store
        .forget_project("your-org/your-project", "drop")
        .await
        .unwrap_err();
    assert!(matches!(err, MemoryError::NotFound));
}

#[tokio::test]
async fn blank_input_is_rejected_before_it_reaches_storage() {
    let (store, _) = fresh().await;
    assert!(matches!(
        store
            .remember_project("your-org/your-project", "   ", &json!(1))
            .await
            .unwrap_err(),
        MemoryError::EmptyKey
    ));
    assert!(matches!(
        store
            .remember_project("", "k", &json!(1))
            .await
            .unwrap_err(),
        MemoryError::EmptyProject
    ));
    assert!(matches!(
        store
            .remember_personal("", "k", &json!(1))
            .await
            .unwrap_err(),
        MemoryError::EmptyOwner
    ));
}

/// The classification is structural rather than
/// conventional. These go around the store entirely and write raw
/// SQL: if only the Rust code enforced the rule, they would succeed.
#[tokio::test]
async fn the_database_itself_rejects_a_row_that_names_both_a_project_and_an_owner() {
    let (_, pool) = fresh().await;
    let result = sqlx::query(
        "INSERT INTO agent_memory \
         (id, state_type, project, owner_id, key, value, created_at, updated_at) \
         VALUES ('x', 'project', 'your-org/your-project', 'owner-1', 'k', '1', 'now', 'now')",
    )
    .execute(&pool)
    .await;
    assert!(result.is_err(), "a both-scopes row was accepted");
}

#[tokio::test]
async fn the_database_itself_rejects_a_row_that_names_neither() {
    let (_, pool) = fresh().await;
    let result = sqlx::query(
        "INSERT INTO agent_memory \
         (id, state_type, project, owner_id, key, value, created_at, updated_at) \
         VALUES ('x', 'personal', NULL, NULL, 'k', '1', 'now', 'now')",
    )
    .execute(&pool)
    .await;
    assert!(result.is_err(), "a scopeless row was accepted");
}

#[tokio::test]
async fn the_database_itself_rejects_an_unknown_state_type() {
    let (_, pool) = fresh().await;
    let result = sqlx::query(
        "INSERT INTO agent_memory \
         (id, state_type, project, owner_id, key, value, created_at, updated_at) \
         VALUES ('x', 'team-ish', 'your-org/your-project', NULL, 'k', '1', 'now', 'now')",
    )
    .execute(&pool)
    .await;
    assert!(result.is_err(), "an invented state_type was accepted");
}
