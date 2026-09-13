//! A milestone is how a team already names a roadmap, so asking how
//! far along one is has to work end to end: read from the tracker,
//! survive the mirror, and narrow `plan_progress` to that milestone
//! alone.
//!
//! The last part is the one worth testing hardest. A filter that is
//! accepted and then ignored does not fail — it answers, confidently,
//! about the wrong set of issues.

use atlas_tracker::api::{
    FakeTracker, Issue, IssueFilter, IssueId, IssueStatus, Label, MirrorStore, MirroredTracker,
    ProjectRef, SqlitePool, TrackerRuntime, run_migrations,
};
use chrono::Utc;
use std::sync::Arc;
use tempfile::TempDir;

fn project() -> ProjectRef {
    ProjectRef::new("your-org", "your-project").unwrap()
}

/// An issue with two of three acceptance criteria ticked, in
/// `milestone`.
fn issue(id: &str, milestone: Option<&str>) -> Issue {
    let now = Utc::now();
    Issue {
        id: IssueId(id.to_owned()),
        title: format!("issue {id}"),
        status: IssueStatus::Open,
        labels: vec![Label("feature".to_owned())],
        author: "arex95".to_owned(),
        created_at: now,
        updated_at: now,
        description: Some("## Acceptance criteria\n- [x] one\n- [x] two\n- [ ] three\n".to_owned()),
        milestone: milestone.map(str::to_owned),
    }
}

async fn mirror(dir: &TempDir) -> MirrorStore {
    let pool = SqlitePool::connect(&format!(
        "sqlite://{}?mode=rwc",
        dir.path().join("mirror.db").display()
    ))
    .await
    .unwrap();
    run_migrations(&pool).await.unwrap();
    MirrorStore::new(pool)
}

#[tokio::test]
async fn a_milestone_survives_the_mirror() {
    // Without the column, every issue comes back with no milestone and
    // a roadmap question silently has no roadmap to ask about.
    let dir = TempDir::new().unwrap();
    let store = mirror(&dir).await;

    store
        .upsert_issue(&project(), &issue("1", Some("v1")), Utc::now())
        .await
        .unwrap();

    let read = store
        .get_issue(&project(), &IssueId("1".to_owned()))
        .await
        .unwrap()
        .expect("not mirrored");
    assert_eq!(read.milestone.as_deref(), Some("v1"));
}

#[tokio::test]
async fn an_issue_in_no_milestone_reads_back_as_none_not_as_empty() {
    // The column stores "" for both "no milestone" and "written before
    // the column existed". Neither should reach a caller as a
    // milestone named "".
    let dir = TempDir::new().unwrap();
    let store = mirror(&dir).await;

    store
        .upsert_issue(&project(), &issue("1", None), Utc::now())
        .await
        .unwrap();

    let read = store
        .get_issue(&project(), &IssueId("1".to_owned()))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(read.milestone, None);
}

#[tokio::test]
async fn the_mirror_narrows_a_listing_to_one_milestone() {
    let dir = TempDir::new().unwrap();
    let store = mirror(&dir).await;
    for (id, m) in [("1", Some("v1")), ("2", Some("v2")), ("3", None)] {
        store
            .upsert_issue(&project(), &issue(id, m), Utc::now())
            .await
            .unwrap();
    }

    let filter = IssueFilter {
        milestone: Some("v1".to_owned()),
        ..IssueFilter::default()
    };
    let listed = store.list_issues(&project(), &filter).await.unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id.0, "1");

    // And saying nothing still means every issue.
    let all = store
        .list_issues(&project(), &IssueFilter::default())
        .await
        .unwrap();
    assert_eq!(all.len(), 3);
}

/// The property the whole slice exists for.
#[tokio::test]
async fn plan_progress_counts_only_the_milestone_it_was_asked_about() {
    let dir = TempDir::new().unwrap();
    let store = mirror(&dir).await;

    // Two issues in v1, one in v2. Each has 2 of 3 criteria ticked.
    // Seeded into the mirror, because reads go through it — this
    // exercises the SQL filter rather than the fake's.
    for (id, m) in [("1", Some("v1")), ("2", Some("v1")), ("3", Some("v2"))] {
        store
            .upsert_issue(&project(), &issue(id, m), Utc::now())
            .await
            .unwrap();
    }

    let mirrored = MirroredTracker::new(store, Arc::new(FakeTracker::new()));
    let runtime = TrackerRuntime::new(Arc::new(mirrored));

    let v1 = runtime
        .plan_progress(
            &project(),
            &IssueFilter {
                milestone: Some("v1".to_owned()),
                ..IssueFilter::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(
        (v1.done, v1.total),
        (4, 6),
        "a roadmap's progress counted issues outside it"
    );

    let everything = runtime
        .plan_progress(&project(), &IssueFilter::default())
        .await
        .unwrap();
    assert_eq!((everything.done, everything.total), (6, 9));
}

#[tokio::test]
async fn an_unknown_milestone_is_empty_rather_than_everything() {
    // The failure that matters: a typo answering about the whole
    // project as though it were the milestone.
    let upstream = Arc::new(FakeTracker::new());
    upstream.insert(project(), issue("1", Some("v1")), vec![]);
    let runtime = TrackerRuntime::new(upstream);

    let progress = runtime
        .plan_progress(
            &project(),
            &IssueFilter {
                milestone: Some("v-typo".to_owned()),
                ..IssueFilter::default()
            },
        )
        .await
        .unwrap();
    assert_eq!((progress.done, progress.total), (0, 0));
}
