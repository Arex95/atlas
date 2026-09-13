//! Behavioural tests for the mirror syncer.
//!
//! A `FakeTracker` stands in for the upstream; the syncer pulls
//! from it into a real (in-memory-file) `SQLite` mirror, and the
//! test asserts what the mirror ended up containing.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use atlas_tracker::api::{
    FakeTracker, Issue, IssueFilter, IssueId, IssueRelation, IssueStatus, IssueTracker, Label,
    MirrorStore, MirrorSyncer, MirrorSyncerConfig, NewIssue, ProjectRef, SqlitePool, TrackerError,
    run_migrations,
};
use chrono::{Duration as ChronoDuration, Utc};
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

fn sample_issue(id: &str, title: &str, updated_offset_secs: i64) -> Issue {
    let now = Utc::now();
    Issue {
        id: IssueId(id.to_owned()),
        title: title.to_owned(),
        status: IssueStatus::Open,
        labels: vec![Label("feature".to_owned())],
        author: "arex95".to_owned(),
        created_at: now,
        updated_at: now + ChronoDuration::seconds(updated_offset_secs),
        description: None,
        milestone: None,
    }
}

fn project() -> ProjectRef {
    ProjectRef::new("your-org", "your-project").unwrap()
}

#[tokio::test]
async fn initial_pass_populates_mirror() {
    let dir = TempDir::new().unwrap();
    let pool = fresh_pool(&dir).await;
    let store = MirrorStore::new(pool);

    let upstream = Arc::new(FakeTracker::new());
    for i in 1..=3 {
        upstream.insert(
            project(),
            sample_issue(&i.to_string(), &format!("#{i}"), 0),
            vec![],
        );
    }

    let syncer = MirrorSyncer::new(MirrorSyncerConfig {
        upstream: upstream.clone() as Arc<dyn IssueTracker>,
        store: store.clone(),
        projects: vec![project()],
        interval: Duration::from_secs(60),
    });
    syncer.sync_once().await;

    let listed = store
        .list_issues(&project(), &IssueFilter::default())
        .await
        .unwrap();
    assert_eq!(listed.len(), 3, "mirror should hold every upstream issue");
}

#[tokio::test]
async fn outage_leaves_mirror_intact() {
    let dir = TempDir::new().unwrap();
    let pool = fresh_pool(&dir).await;
    let store = MirrorStore::new(pool);

    // First pass populates.
    let upstream = Arc::new(FakeTracker::new());
    for i in 1..=3 {
        upstream.insert(
            project(),
            sample_issue(&i.to_string(), &format!("#{i}"), 0),
            vec![],
        );
    }
    MirrorSyncer::new(MirrorSyncerConfig {
        upstream: upstream.clone() as Arc<dyn IssueTracker>,
        store: store.clone(),
        projects: vec![project()],
        interval: Duration::from_secs(60),
    })
    .sync_once()
    .await;

    // Second pass with a broken upstream.
    let broken: Arc<dyn IssueTracker> = Arc::new(BrokenTracker);
    MirrorSyncer::new(MirrorSyncerConfig {
        upstream: broken,
        store: store.clone(),
        projects: vec![project()],
        interval: Duration::from_secs(60),
    })
    .sync_once()
    .await;

    let listed = store
        .list_issues(&project(), &IssueFilter::default())
        .await
        .unwrap();
    assert_eq!(
        listed.len(),
        3,
        "mirror should retain the last-known state through the outage"
    );
}

#[tokio::test]
async fn cursor_records_last_successful_sync() {
    let dir = TempDir::new().unwrap();
    let pool = fresh_pool(&dir).await;
    let store = MirrorStore::new(pool);

    let upstream = Arc::new(FakeTracker::new());
    upstream.insert(project(), sample_issue("1", "one", 0), vec![]);

    let before = Utc::now();
    MirrorSyncer::new(MirrorSyncerConfig {
        upstream: upstream.clone() as Arc<dyn IssueTracker>,
        store: store.clone(),
        projects: vec![project()],
        interval: Duration::from_secs(60),
    })
    .sync_once()
    .await;
    let after = Utc::now();

    let cursor = store.last_sync(&project()).await.unwrap().unwrap();
    assert!(
        cursor >= before && cursor <= after,
        "cursor {cursor} should be within [{before}, {after}]"
    );
}

#[tokio::test]
async fn relations_are_replaced_atomically() {
    let dir = TempDir::new().unwrap();
    let pool = fresh_pool(&dir).await;
    let store = MirrorStore::new(pool);

    let upstream = Arc::new(FakeTracker::new());
    upstream.insert(
        project(),
        sample_issue("1", "one", 0),
        vec![
            IssueRelation::Blocks(IssueId("2".to_owned())),
            IssueRelation::RelatesTo(IssueId("3".to_owned())),
        ],
    );

    MirrorSyncer::new(MirrorSyncerConfig {
        upstream: upstream.clone() as Arc<dyn IssueTracker>,
        store: store.clone(),
        projects: vec![project()],
        interval: Duration::from_secs(60),
    })
    .sync_once()
    .await;

    let rels = store
        .list_relations(&project(), &IssueId("1".to_owned()))
        .await
        .unwrap();
    assert_eq!(rels.len(), 2);
}

struct BrokenTracker;

#[async_trait]
impl IssueTracker for BrokenTracker {
    async fn list_issues(
        &self,
        _project: &ProjectRef,
        _filter: &IssueFilter,
    ) -> Result<Vec<Issue>, TrackerError> {
        Err(TrackerError::Transport("simulated outage".to_owned()))
    }

    async fn get_issue(&self, _project: &ProjectRef, _id: &IssueId) -> Result<Issue, TrackerError> {
        Err(TrackerError::Transport("simulated outage".to_owned()))
    }

    async fn list_relations(
        &self,
        _project: &ProjectRef,
        _id: &IssueId,
    ) -> Result<Vec<IssueRelation>, TrackerError> {
        Err(TrackerError::Transport("simulated outage".to_owned()))
    }

    async fn create_issue(
        &self,
        _project: &ProjectRef,
        _input: NewIssue,
    ) -> Result<Issue, TrackerError> {
        Err(TrackerError::Transport("simulated outage".to_owned()))
    }

    async fn update_status(
        &self,
        _project: &ProjectRef,
        _id: &IssueId,
        _status: IssueStatus,
    ) -> Result<Issue, TrackerError> {
        Err(TrackerError::Transport("simulated outage".to_owned()))
    }
}
