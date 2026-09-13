//! Where a tracker tells Atlas an issue changed.
//!
//! The mirror is kept current by a polling loop on a fixed cadence.
//! This does not replace it — it shortens the gap. A push arrives in
//! seconds; the loop still runs, still catches anything a webhook
//! missed, and is what keeps a deployment correct when no webhook is
//! configured at all.
//!
//! **The handler has four jobs and only four**: authenticate the
//! sender, work out which issue it is about, refresh that one issue,
//! and acknowledge. It does not walk the project, does not touch
//! anything the payload did not name, and does not block the response
//! on the upstream fetch.
//!
//! ## What the authentication does and does not prove
//!
//! GitLab authenticates a webhook by echoing a header chosen when the
//! hook was registered. That proves the sender knew a secret. It does
//! **not** prove the body is unaltered — there is no signature over
//! the payload, unlike some other providers — so the mitigations are
//! that the URL must be HTTPS and the secret must be treated like a
//! password rather than like a name.
//!
//! The consequence is why the handler re-fetches rather than believing
//! the body: the payload is used only to learn *which* issue to ask
//! about. Everything stored comes from an authenticated call back to
//! the tracker, so a forged body can at worst cause a wasted fetch of
//! an issue that already exists.

use std::sync::Arc;

use chrono::Utc;

use serde::Deserialize;
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

use crate::internal::domain::{IssueId, IssueTracker, ProjectRef, TrackerError};
use crate::internal::infrastructure::mirror::MirrorStore;

/// What a webhook told us, reduced to the only two things worth
/// reading from it.
#[derive(Debug, PartialEq, Eq)]
pub struct ChangedIssue {
    pub project: ProjectRef,
    pub id: IssueId,
}

/// The little of GitLab's payload this needs.
///
/// Deliberately partial. Deserialising the whole shape would couple
/// the mirror to a schema that changes without notice, for fields it
/// throws away anyway — the issue itself is re-fetched.
#[derive(Deserialize)]
struct IssueEvent {
    object_kind: String,
    project: EventProject,
    object_attributes: EventIssue,
}

#[derive(Deserialize)]
struct EventProject {
    path_with_namespace: String,
}

#[derive(Deserialize)]
struct EventIssue {
    iid: i64,
}

/// Reads which issue an event is about.
///
/// `None` for an event this does not handle — a push, a pipeline, a
/// comment. Not an error: a tracker may be configured to send more
/// than issues, and answering 200 to something uninteresting is
/// correct, because a non-2xx teaches GitLab to disable the hook.
#[must_use]
pub fn changed_issue(body: &str) -> Option<ChangedIssue> {
    let event: IssueEvent = serde_json::from_str(body).ok()?;
    if event.object_kind != "issue" {
        return None;
    }
    // Parsed rather than trusted: a path that is not `owner/repo` —
    // or is not one at all — yields nothing to refresh instead of a
    // lookup against a reference the rest of the crate cannot use.
    Some(ChangedIssue {
        project: event.project.path_with_namespace.parse().ok()?,
        id: IssueId(event.object_attributes.iid.to_string()),
    })
}

/// Whether a webhook's token matches, in constant time.
///
/// Compared through a fixed-length digest rather than the raw bytes,
/// so a wrong length is rejected without the comparison revealing it.
/// An early return would leak how much of a guess was right, one
/// request at a time, and a secret that leaks a byte at a time has a
/// short life.
#[must_use]
pub fn token_matches(provided: Option<&str>, expected: &str) -> bool {
    let Some(provided) = provided else {
        return false;
    };
    let a = Sha256::digest(provided.as_bytes());
    let b = Sha256::digest(expected.as_bytes());
    a.ct_eq(&b).into()
}

/// Why a refresh did not happen.
///
/// Separate from `TrackerError` because a mirror write failing is not
/// a tracker problem, and the polling loop — which reports the same
/// two failures by logging and carrying on — has no variant for it
/// either.
#[derive(Debug, thiserror::Error)]
pub enum RefreshError {
    #[error("could not fetch the issue: {0}")]
    Upstream(#[from] TrackerError),
    #[error("could not write it to the mirror: {0}")]
    Mirror(String),
}

/// Re-fetches one issue from the tracker and writes it to the mirror.
///
/// Everything stored comes from this call, never from the webhook
/// body — see the note at the top of the file on what the token does
/// not prove.
///
/// # Errors
/// `Upstream` if the tracker refused or was unreachable, `Mirror` if
/// the write failed. Both are worth distinguishing: the first usually
/// means credentials or rate limiting, the second a disk.
pub async fn refresh_issue(
    upstream: &Arc<dyn IssueTracker>,
    store: &MirrorStore,
    changed: &ChangedIssue,
) -> Result<(), RefreshError> {
    let issue = upstream.get_issue(&changed.project, &changed.id).await?;
    // `fetched_at` is now rather than any time in the payload: it
    // records when *this* row was read from the tracker, and a
    // timestamp taken from an unsigned body could make a stale row
    // look fresh.
    store
        .upsert_issue(&changed.project, &issue, Utc::now())
        .await
        .map_err(|e| RefreshError::Mirror(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const ISSUE_EVENT: &str = r#"{
        "object_kind": "issue",
        "project": { "path_with_namespace": "your-org/your-project" },
        "object_attributes": { "iid": 42, "title": "whatever" }
    }"#;

    #[test]
    fn an_issue_event_names_its_project_and_issue() {
        let changed = changed_issue(ISSUE_EVENT).expect("not read");
        assert_eq!(changed.project.path(), "your-org/your-project");
        assert_eq!(changed.id.0, "42");
    }

    #[test]
    fn an_event_of_another_kind_is_ignored_rather_than_refused() {
        // Answering non-2xx would teach GitLab to disable the hook, so
        // an event this does not handle is still a success.
        let push = r#"{ "object_kind": "push", "project": { "path_with_namespace": "a/b" },
                        "object_attributes": { "iid": 1 } }"#;
        assert_eq!(changed_issue(push), None);
    }

    #[test]
    fn a_project_path_that_is_not_owner_slash_repo_is_ignored() {
        let flat = r#"{ "object_kind": "issue", "project": { "path_with_namespace": "justaname" },
                        "object_attributes": { "iid": 1 } }"#;
        assert_eq!(changed_issue(flat), None);
    }

    #[test]
    fn a_payload_missing_what_it_needs_is_ignored() {
        assert_eq!(changed_issue("{}"), None);
        assert_eq!(changed_issue("not json at all"), None);
        assert_eq!(
            changed_issue(r#"{"object_kind":"issue","project":{}}"#),
            None
        );
    }

    #[test]
    fn extra_fields_do_not_break_it() {
        // The payload carries far more than this reads, and it changes
        // without notice.
        let fat = r#"{
            "object_kind": "issue", "event_type": "issue", "user": { "id": 1 },
            "project": { "path_with_namespace": "a/b", "web_url": "…", "id": 9 },
            "object_attributes": { "iid": 7, "state": "opened", "labels": [] },
            "changes": {}, "repository": {}
        }"#;
        assert_eq!(changed_issue(fat).unwrap().id.0, "7");
    }

    #[test]
    fn a_matching_token_is_accepted_and_nothing_else_is() {
        assert!(token_matches(Some("s3cret"), "s3cret"));
        assert!(!token_matches(Some("s3cre"), "s3cret"));
        assert!(!token_matches(Some("s3crett"), "s3cret"));
        assert!(!token_matches(Some(""), "s3cret"));
        assert!(!token_matches(None, "s3cret"));
    }

    #[test]
    fn an_empty_expected_token_matches_nothing_useful() {
        // A deployment that configured no secret must not accept every
        // request that omits the header.
        assert!(!token_matches(None, ""));
    }
}
