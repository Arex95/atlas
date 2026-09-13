//! Integration tests for the webhook receiver, driven through the
//! real `axum` router.
//!
//! The unit tests in `webhook.rs` cover what a payload is read as.
//! What they cannot cover is the property the endpoint exists to hold:
//! **an unauthenticated request changes nothing.** That one is only
//! true of the router, so it is asserted here — against a real
//! `SQLite` mirror, by looking at what the mirror contains afterwards
//! rather than at the status code.

use std::sync::Arc;
use std::time::Duration;

use atlas_tracker::api::{
    FakeTracker, Issue, IssueId, IssueStatus, Label, MirrorStore, ProjectRef, SqlitePool,
    run_migrations, webhook_router,
};
use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use chrono::Utc;
use tower::ServiceExt;

const SECRET: &str = "the-webhook-secret";

const ISSUE_EVENT: &str = r#"{
    "object_kind": "issue",
    "project": { "path_with_namespace": "your-org/your-project" },
    "object_attributes": { "iid": 7 }
}"#;

fn project() -> ProjectRef {
    ProjectRef::new("your-org", "your-project").unwrap()
}

fn upstream_issue() -> Issue {
    let now = Utc::now();
    Issue {
        id: IssueId("7".to_owned()),
        title: "from the tracker".to_owned(),
        status: IssueStatus::Open,
        labels: vec![Label("feature".to_owned())],
        author: "arex95".to_owned(),
        created_at: now,
        updated_at: now,
        description: None,
        milestone: None,
    }
}

/// A router over a real mirror, with one issue waiting upstream.
async fn harness(secret: Option<String>) -> (Router, MirrorStore) {
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = MirrorStore::new(pool);

    let upstream = Arc::new(FakeTracker::new());
    upstream.insert(project(), upstream_issue(), vec![]);

    let router = webhook_router(upstream, store.clone(), secret);
    (router, store)
}

async fn post(router: &Router, token: Option<&str>, body: &str) -> StatusCode {
    let mut request = Request::builder()
        .method("POST")
        .uri("/gitlab")
        .header("content-type", "application/json");
    if let Some(token) = token {
        request = request.header("x-gitlab-token", token);
    }
    router
        .clone()
        .oneshot(request.body(Body::from(body.to_owned())).unwrap())
        .await
        .unwrap()
        .status()
}

/// The refresh runs off the response path, so the assertion has to
/// wait for it. A deadline rather than a fixed sleep: a passing run
/// stays fast and a failing one still fails.
async fn mirrored_within(store: &MirrorStore, id: &str) -> Option<Issue> {
    for _ in 0..100 {
        if let Some(issue) = store
            .get_issue(&project(), &IssueId(id.to_owned()))
            .await
            .unwrap()
        {
            return Some(issue);
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    None
}

#[tokio::test]
async fn a_valid_event_refreshes_that_issue_from_the_tracker() {
    let (router, store) = harness(Some(SECRET.to_owned())).await;

    assert_eq!(
        post(&router, Some(SECRET), ISSUE_EVENT).await,
        StatusCode::OK
    );

    let mirrored = mirrored_within(&store, "7").await.expect("never mirrored");
    // The title came from the tracker, not from the payload — the
    // payload never carried one.
    assert_eq!(mirrored.title, "from the tracker");
}

#[tokio::test]
async fn a_wrong_token_is_refused_and_writes_nothing() {
    let (router, store) = harness(Some(SECRET.to_owned())).await;

    assert_eq!(
        post(&router, Some("not-the-secret"), ISSUE_EVENT).await,
        StatusCode::UNAUTHORIZED
    );
    // The status alone would not prove this: what matters is that the
    // request did not reach the tracker on our credentials.
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(
        store
            .get_issue(&project(), &IssueId("7".to_owned()))
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn a_missing_token_is_refused_and_writes_nothing() {
    let (router, store) = harness(Some(SECRET.to_owned())).await;

    assert_eq!(
        post(&router, None, ISSUE_EVENT).await,
        StatusCode::UNAUTHORIZED
    );
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(
        store
            .get_issue(&project(), &IssueId("7".to_owned()))
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn an_event_of_another_kind_is_acknowledged_without_a_refresh() {
    // A non-2xx teaches GitLab to disable the hook, and there is
    // nothing wrong on its side.
    let (router, store) = harness(Some(SECRET.to_owned())).await;
    let push = r#"{ "object_kind": "push", "project": { "path_with_namespace": "your-org/your-project" },
                    "object_attributes": { "iid": 7 } }"#;

    assert_eq!(post(&router, Some(SECRET), push).await, StatusCode::OK);
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(
        store
            .get_issue(&project(), &IssueId("7".to_owned()))
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn a_malformed_body_is_acknowledged_rather_than_refused() {
    let (router, _) = harness(Some(SECRET.to_owned())).await;
    assert_eq!(
        post(&router, Some(SECRET), "not json at all").await,
        StatusCode::OK
    );
}

#[tokio::test]
async fn without_a_secret_the_endpoint_does_not_exist() {
    // An unauthenticated receiver would be a way for anyone to drive
    // requests to the tracker on our credentials, so a deployment that
    // configured no secret gets no route at all.
    let (router, _) = harness(None).await;
    assert_eq!(
        post(&router, Some(SECRET), ISSUE_EVENT).await,
        StatusCode::NOT_FOUND
    );

    let (blank, _) = harness(Some("   ".to_owned())).await;
    assert_eq!(
        post(&blank, Some("   "), ISSUE_EVENT).await,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn an_event_about_an_issue_the_tracker_does_not_have_is_still_acknowledged() {
    // The refresh fails off the response path and is logged; GitLab
    // is told the delivery arrived, because retrying would not help.
    let (router, store) = harness(Some(SECRET.to_owned())).await;
    let unknown = r#"{ "object_kind": "issue", "project": { "path_with_namespace": "your-org/your-project" },
                       "object_attributes": { "iid": 999 } }"#;

    assert_eq!(post(&router, Some(SECRET), unknown).await, StatusCode::OK);
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(
        store
            .get_issue(&project(), &IssueId("999".to_owned()))
            .await
            .unwrap()
            .is_none()
    );
}
