//! The in-memory fake must satisfy the same contract every real
//! adapter satisfies. This test is what makes the fake safe to use
//! in downstream tests: whatever it returns, a real adapter would
//! have returned the same shape.

use atlas_tracker::api::{FakeTracker, IssueTracker, NewIssue, ProjectRef, TrackerError};
use atlas_tracker::contract::{ContractFixture, run_contract, run_write_contract};

#[tokio::test]
async fn fake_satisfies_contract() {
    run_contract(|fixture: ContractFixture| async move {
        let fake = FakeTracker::new();
        fake.insert(
            fixture.project.clone(),
            fixture.open_issue.clone(),
            fixture.relations.clone(),
        );
        fake.insert(fixture.project, fixture.closed_issue, Vec::new());
        fake
    })
    .await;
}

#[tokio::test]
async fn fake_satisfies_write_contract() {
    run_write_contract(|| async {
        (
            FakeTracker::new(),
            ProjectRef::new("your-org", "atlas").unwrap(),
        )
    })
    .await;
}

#[tokio::test]
async fn fake_rejects_an_empty_title() {
    let fake = FakeTracker::new();
    let project = ProjectRef::new("your-org", "atlas").unwrap();
    let err = fake
        .create_issue(
            &project,
            NewIssue {
                title: String::new(),
                description: None,
                labels: Vec::new(),
            },
        )
        .await
        .unwrap_err();
    assert!(matches!(err, TrackerError::Invalid(_)));
}
