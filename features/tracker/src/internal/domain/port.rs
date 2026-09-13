use async_trait::async_trait;

use super::error::TrackerError;
use super::filter::IssueFilter;
use super::issue::{Issue, IssueId, IssueRelation, IssueStatus, NewIssue};
use super::project::ProjectRef;

/// The external-tracker port.
///
/// Every method takes a `ProjectRef` because Atlas is multi-project;
/// the adapter is not bound to a single tracker project at
/// construction. Errors follow the mapping in [`TrackerError`].
///
/// `close_issue` is not its own method: it is `update_status` with
/// [`IssueStatus::Closed`] — same wire call on every adapter, no
/// reason to double the trait surface for it.
#[async_trait]
pub trait IssueTracker: Send + Sync {
    async fn list_issues(
        &self,
        project: &ProjectRef,
        filter: &IssueFilter,
    ) -> Result<Vec<Issue>, TrackerError>;

    async fn get_issue(&self, project: &ProjectRef, id: &IssueId) -> Result<Issue, TrackerError>;

    async fn list_relations(
        &self,
        project: &ProjectRef,
        id: &IssueId,
    ) -> Result<Vec<IssueRelation>, TrackerError>;

    async fn create_issue(
        &self,
        project: &ProjectRef,
        input: NewIssue,
    ) -> Result<Issue, TrackerError>;

    async fn update_status(
        &self,
        project: &ProjectRef,
        id: &IssueId,
        status: IssueStatus,
    ) -> Result<Issue, TrackerError>;
}
