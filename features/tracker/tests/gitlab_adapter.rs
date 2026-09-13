//! Contract test for the GitLab adapter, backed by `wiremock`.
//!
//! No live GitLab call: CI runs offline. The mocks describe the
//! subset of the GitLab REST v4 responses this issue's read-only
//! adapter needs.

use atlas_tracker::api::{
    GitLabTracker, Issue, IssueFilter, IssueId, IssueRelation, IssueStatus, IssueTracker, Label,
    NewIssue, ProjectRef, TrackerError,
};
use atlas_tracker::contract::run_write_contract;
use chrono::{TimeZone, Utc};
use serde_json::json;
use url::Url;
use wiremock::matchers::{body_string_contains, header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn open_issue_json() -> serde_json::Value {
    json!({
        "iid": 1,
        "title": "add tracker port",
        "state": "opened",
        "labels": ["feature"],
        "author": { "username": "arex95" },
        "created_at": "2026-08-20T10:00:00Z",
        "updated_at": "2026-08-20T12:00:00Z",
    })
}

fn closed_issue_json() -> serde_json::Value {
    json!({
        "iid": 2,
        "title": "bootstrap repo",
        "state": "closed",
        "labels": ["chore"],
        "author": { "username": "arex95" },
        "created_at": "2026-08-20T10:00:00Z",
        "updated_at": "2026-08-20T12:00:00Z",
    })
}

const PROJECT_PATH: &str = "your-org%2Fyour-project";

async fn seeded_server() -> MockServer {
    let server = MockServer::start().await;

    // list open — registered first with higher priority (lower
    // number) so `state=opened` requests hit this one instead of
    // the generic "list all" below.
    Mock::given(method("GET"))
        .and(path(format!("/api/v4/projects/{PROJECT_PATH}/issues")))
        .and(query_param("state", "opened"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([open_issue_json()])))
        .with_priority(1)
        .mount(&server)
        .await;

    // list all — fallback for requests without a `state` filter.
    Mock::given(method("GET"))
        .and(path(format!("/api/v4/projects/{PROJECT_PATH}/issues")))
        .and(header("PRIVATE-TOKEN", "pat-xyz"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!([open_issue_json(), closed_issue_json()])),
        )
        .with_priority(5)
        .mount(&server)
        .await;

    // get by iid — hit
    Mock::given(method("GET"))
        .and(path(format!("/api/v4/projects/{PROJECT_PATH}/issues/1")))
        .respond_with(ResponseTemplate::new(200).set_body_json(open_issue_json()))
        .mount(&server)
        .await;

    // get by iid — miss
    Mock::given(method("GET"))
        .and(path(format!("/api/v4/projects/{PROJECT_PATH}/issues/999")))
        .respond_with(ResponseTemplate::new(404).set_body_string("Not Found"))
        .mount(&server)
        .await;

    // links for iid=1
    Mock::given(method("GET"))
        .and(path(format!(
            "/api/v4/projects/{PROJECT_PATH}/issues/1/links"
        )))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            { "iid": 3, "link_type": "blocks" },
            { "iid": 4, "link_type": "relates_to" },
        ])))
        .mount(&server)
        .await;

    server
}

#[tokio::test]
async fn gitlab_adapter_contract() {
    let server = seeded_server().await;
    let base = Url::parse(&server.uri()).unwrap();
    let tracker =
        GitLabTracker::new(base, "pat-xyz".to_owned(), "atlas-tests/0".to_owned()).unwrap();
    let project = ProjectRef::new("your-org", "your-project").unwrap();
    let updated = Utc.with_ymd_and_hms(2026, 8, 20, 12, 0, 0).unwrap();

    let all = tracker
        .list_issues(&project, &IssueFilter::default())
        .await
        .unwrap();
    assert_eq!(all.len(), 2);

    let open = tracker
        .list_issues(&project, &IssueFilter::open())
        .await
        .unwrap();
    assert_eq!(open.len(), 1);
    assert_eq!(open[0].status, IssueStatus::Open);
    assert_eq!(open[0].labels, vec![Label("feature".to_owned())]);

    let got: Issue = tracker
        .get_issue(&project, &IssueId("1".to_owned()))
        .await
        .unwrap();
    assert_eq!(got.title, "add tracker port");
    assert_eq!(got.updated_at, updated);

    let missing = tracker
        .get_issue(&project, &IssueId("999".to_owned()))
        .await;
    assert!(matches!(missing, Err(TrackerError::NotFound)));

    let rels = tracker
        .list_relations(&project, &IssueId("1".to_owned()))
        .await
        .unwrap();
    assert_eq!(
        rels,
        vec![
            IssueRelation::Blocks(IssueId("3".to_owned())),
            IssueRelation::RelatesTo(IssueId("4".to_owned())),
        ]
    );
}

/// A stateless mock server for the write contract: POST always
/// returns a fresh open issue at iid 1 with the exact title
/// `run_write_contract` sends; PUT distinguishes `state_event`
/// values by matching the form body, since a stateless mock cannot
/// otherwise tell a close from a reopen. `run_write_contract` never
/// depends on a subsequent GET reflecting the write, so this is
/// faithful to the real wire shape it asserts on.
async fn write_server() -> MockServer {
    let server = MockServer::start().await;
    let written_issue = |state: &str| {
        json!({
            "iid": 1,
            "title": "written by the write contract",
            "state": state,
            "labels": ["feature"],
            "author": { "username": "arex95" },
            "created_at": "2026-08-22T09:00:00Z",
            "updated_at": "2026-08-22T09:00:00Z",
        })
    };

    Mock::given(method("POST"))
        .and(path(format!("/api/v4/projects/{PROJECT_PATH}/issues")))
        .respond_with(ResponseTemplate::new(201).set_body_json(written_issue("opened")))
        .mount(&server)
        .await;

    Mock::given(method("PUT"))
        .and(path(format!("/api/v4/projects/{PROJECT_PATH}/issues/1")))
        .and(body_string_contains("state_event=close"))
        .respond_with(ResponseTemplate::new(200).set_body_json(written_issue("closed")))
        .mount(&server)
        .await;

    Mock::given(method("PUT"))
        .and(path(format!("/api/v4/projects/{PROJECT_PATH}/issues/1")))
        .and(body_string_contains("state_event=reopen"))
        .respond_with(ResponseTemplate::new(200).set_body_json(written_issue("opened")))
        .mount(&server)
        .await;

    server
}

#[tokio::test]
async fn gitlab_adapter_satisfies_write_contract() {
    let server = write_server().await;
    let base = Url::parse(&server.uri()).unwrap();

    run_write_contract(|| async {
        let tracker =
            GitLabTracker::new(base, "pat-xyz".to_owned(), "atlas-tests/0".to_owned()).unwrap();
        (
            tracker,
            ProjectRef::new("your-org", "your-project").unwrap(),
        )
    })
    .await;
}

#[tokio::test]
async fn create_issue_rejection_maps_to_invalid() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(422).set_body_string("title is missing"))
        .mount(&server)
        .await;
    let base = Url::parse(&server.uri()).unwrap();
    let tracker =
        GitLabTracker::new(base, "pat-xyz".to_owned(), "atlas-tests/0".to_owned()).unwrap();
    let project = ProjectRef::new("your-org", "your-project").unwrap();

    let err = tracker
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

#[tokio::test]
async fn update_status_conflict_maps_to_conflict() {
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .respond_with(ResponseTemplate::new(409).set_body_string("already in that state"))
        .mount(&server)
        .await;
    let base = Url::parse(&server.uri()).unwrap();
    let tracker =
        GitLabTracker::new(base, "pat-xyz".to_owned(), "atlas-tests/0".to_owned()).unwrap();
    let project = ProjectRef::new("your-org", "your-project").unwrap();

    let err = tracker
        .update_status(&project, &IssueId("1".to_owned()), IssueStatus::Closed)
        .await
        .unwrap_err();
    assert!(matches!(err, TrackerError::Conflict(_)));
}

#[tokio::test]
async fn unauthorized_maps_to_domain_error() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(401).set_body_string("Unauthorized"))
        .mount(&server)
        .await;
    let base = Url::parse(&server.uri()).unwrap();
    let tracker = GitLabTracker::new(base, "bad".to_owned(), "atlas-tests/0".to_owned()).unwrap();
    let project = ProjectRef::new("your-org", "your-project").unwrap();
    let err = tracker
        .list_issues(&project, &IssueFilter::default())
        .await
        .unwrap_err();
    assert!(matches!(err, TrackerError::Unauthorized));
}
