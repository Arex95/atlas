//! Contract every [`IssueTracker`] implementation must satisfy.
//!
//! Exposed under `#[cfg(any(test, feature = "test-support"))]` so
//! the crate's own tests and downstream crates' tests can pin any
//! implementation (real or fake) against the same suite. If an
//! adapter passes this contract, callers depending only on the
//! port can trust it.
//!
//! The contract does *not* prescribe how the adapter's state gets
//! populated — that is the seed callback's job. Every test seeds
//! its own fixture through the callback and then exercises the
//! adapter through the port.

use std::future::Future;

use chrono::{TimeZone, Utc};

use crate::api::{
    Issue, IssueFilter, IssueId, IssueRelation, IssueStatus, IssueTracker, Label, NewIssue,
    ProjectRef, TrackerError,
};

/// The data every contract test expects the adapter to serve back.
///
/// A seed function receives this struct and returns an
/// `IssueTracker` populated with it. Real adapters can seed via
/// their transport (HTTP mocks); fakes seed in memory.
#[derive(Clone)]
pub struct ContractFixture {
    pub project: ProjectRef,
    pub open_issue: Issue,
    pub closed_issue: Issue,
    pub relations: Vec<IssueRelation>,
}

impl ContractFixture {
    /// # Panics
    /// If the hard-coded fixture timestamps cannot be constructed —
    /// they are constants and this only panics if someone edits
    /// them to an invalid date.
    #[must_use]
    pub fn sample() -> Self {
        let created = Utc.with_ymd_and_hms(2026, 8, 20, 10, 0, 0).unwrap();
        let updated = Utc.with_ymd_and_hms(2026, 8, 20, 12, 0, 0).unwrap();
        let project = ProjectRef::new("your-org", "your-project").unwrap();
        let open_issue = Issue {
            id: IssueId("1".to_owned()),
            title: "add tracker port".to_owned(),
            status: IssueStatus::Open,
            labels: vec![Label("feature".to_owned())],
            author: "arex95".to_owned(),
            created_at: created,
            updated_at: updated,
            description: Some("## Acceptance criteria\n- [x] one\n- [ ] two\n".to_owned()),
            milestone: None,
        };
        let closed_issue = Issue {
            id: IssueId("2".to_owned()),
            title: "bootstrap repo".to_owned(),
            status: IssueStatus::Closed,
            labels: vec![Label("chore".to_owned())],
            author: "arex95".to_owned(),
            created_at: created,
            updated_at: updated,
            description: None,
            milestone: None,
        };
        let relations = vec![
            IssueRelation::Blocks(IssueId("3".to_owned())),
            IssueRelation::RelatesTo(IssueId("4".to_owned())),
        ];
        Self {
            project,
            open_issue,
            closed_issue,
            relations,
        }
    }
}

/// Run the shared contract suite against an adapter produced by
/// `seed`.
///
/// # Panics
/// On the first violation of the contract, with an assertion
/// message pointing at the failing expectation.
pub async fn run_contract<T, F, Fut>(seed: F)
where
    T: IssueTracker,
    F: Fn(ContractFixture) -> Fut,
    Fut: Future<Output = T>,
{
    let fixture = ContractFixture::sample();
    let tracker = seed(fixture.clone()).await;

    // list_issues with no filter returns both.
    let all = tracker
        .list_issues(&fixture.project, &IssueFilter::default())
        .await
        .expect("list_issues");
    assert_eq!(all.len(), 2, "expected both fixture issues, got {all:?}");

    // filter by status=open drops the closed one.
    let open = tracker
        .list_issues(&fixture.project, &IssueFilter::open())
        .await
        .expect("list_issues open");
    assert_eq!(open.len(), 1);
    assert_eq!(open[0].id, fixture.open_issue.id);

    // get_issue returns the exact record for a known id.
    let got = tracker
        .get_issue(&fixture.project, &fixture.open_issue.id)
        .await
        .expect("get_issue open");
    assert_eq!(got.title, fixture.open_issue.title);

    // get_issue on an unknown id maps to NotFound.
    let missing = tracker
        .get_issue(&fixture.project, &IssueId("999".to_owned()))
        .await;
    assert!(
        matches!(missing, Err(TrackerError::NotFound)),
        "expected NotFound, got {missing:?}"
    );

    // list_relations returns the seeded set.
    let rels = tracker
        .list_relations(&fixture.project, &fixture.open_issue.id)
        .await
        .expect("list_relations");
    assert_eq!(rels, fixture.relations);
}

/// Write contract every [`IssueTracker`] implementation must
/// satisfy. Separate from [`run_contract`] because it needs a
/// *fresh*, writable tracker — `build` constructs one from
/// scratch rather than seeding a fixture, so it works uniformly
/// whether the adapter's state lives in memory, `SQLite`, or a
/// (possibly stateless-mock) HTTP backend.
///
/// # Panics
/// On the first violation of the contract, with an assertion
/// message pointing at the failing expectation.
pub async fn run_write_contract<T, F, Fut>(build: F)
where
    T: IssueTracker,
    F: FnOnce() -> Fut,
    Fut: Future<Output = (T, ProjectRef)>,
{
    let (tracker, project) = build().await;

    let input = NewIssue {
        title: "written by the write contract".to_owned(),
        description: Some("exercised by run_write_contract".to_owned()),
        labels: vec![Label("feature".to_owned())],
    };
    let created = tracker
        .create_issue(&project, input.clone())
        .await
        .expect("create_issue");
    assert_eq!(created.title, input.title);
    assert_eq!(created.status, IssueStatus::Open);

    let closed = tracker
        .update_status(&project, &created.id, IssueStatus::Closed)
        .await
        .expect("update_status to closed");
    assert_eq!(closed.id, created.id);
    assert_eq!(closed.status, IssueStatus::Closed);

    let reopened = tracker
        .update_status(&project, &created.id, IssueStatus::Open)
        .await
        .expect("update_status to open");
    assert_eq!(reopened.status, IssueStatus::Open);
}
