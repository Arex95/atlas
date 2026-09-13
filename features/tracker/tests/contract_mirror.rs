//! The mirror adapter satisfies the same contract every real
//! adapter satisfies — that is what makes it interchangeable
//! through the `IssueTracker` port. The seed populates the
//! `MirrorStore` directly (no upstream call), then the contract
//! reads through `MirroredTracker`.

use std::sync::Arc;

use atlas_tracker::api::{
    FakeTracker, MirrorStore, MirroredTracker, ProjectRef, SqlitePool, run_migrations,
};
use atlas_tracker::contract::{ContractFixture, run_contract, run_write_contract};
use chrono::Utc;
use sqlx::sqlite::SqliteConnectOptions;
use tempfile::TempDir;

async fn fresh_pool(dir: &TempDir) -> SqlitePool {
    let path = dir.path().join("mirror.db");
    let options = SqliteConnectOptions::new()
        .filename(&path)
        .create_if_missing(true);
    let pool = SqlitePool::connect_with(options).await.unwrap();
    run_migrations(&pool).await.unwrap();
    pool
}

#[tokio::test]
async fn mirror_satisfies_contract() {
    let dir = TempDir::new().unwrap();
    let pool = fresh_pool(&dir).await;
    let store = MirrorStore::new(pool);
    let store_seed = store.clone();

    run_contract(move |fixture: ContractFixture| {
        let store = store_seed.clone();
        async move {
            let now = Utc::now();
            store
                .upsert_issue(&fixture.project, &fixture.open_issue, now)
                .await
                .unwrap();
            store
                .upsert_issue(&fixture.project, &fixture.closed_issue, now)
                .await
                .unwrap();
            store
                .replace_relations(&fixture.project, &fixture.open_issue.id, &fixture.relations)
                .await
                .unwrap();
            // Reads-only fixture — the upstream is never called by
            // `run_contract`, so a bare disabled-by-default fake is
            // enough here. Write-through is exercised separately in
            // `mirror_satisfies_write_contract` below.
            MirroredTracker::new(store, Arc::new(FakeTracker::new()))
        }
    })
    .await;
}

/// The mirror is not just a cache for reads — a write
/// must reach the real tracker and then land in `SQLite`
/// immediately, without waiting for the next syncer tick. This
/// pins that behaviour: `upstream` here is a distinct `FakeTracker`
/// so the mirror's `store` cannot be reflecting anything except
/// what write-through explicitly copied into it.
#[tokio::test]
async fn mirror_satisfies_write_contract() {
    let dir = TempDir::new().unwrap();
    let pool = fresh_pool(&dir).await;
    let store = MirrorStore::new(pool);
    let store_check = store.clone();
    let upstream = Arc::new(FakeTracker::new());
    let project = ProjectRef::new("your-org", "your-project").unwrap();

    run_write_contract(|| async { (MirroredTracker::new(store, upstream), project.clone()) }).await;

    // Write-through must have landed in SQLite immediately — no
    // wait for a syncer tick, and no upstream read involved here.
    let mirrored = store_check
        .list_issues(&project, &atlas_tracker::api::IssueFilter::default())
        .await
        .unwrap();
    assert_eq!(
        mirrored.len(),
        1,
        "expected the write-through issue to be in the mirror store, got {mirrored:?}"
    );
}
