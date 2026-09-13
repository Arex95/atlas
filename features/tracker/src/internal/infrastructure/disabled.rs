use async_trait::async_trait;

use crate::internal::domain::{
    Issue, IssueFilter, IssueId, IssueRelation, IssueStatus, IssueTracker, NewIssue, ProjectRef,
    TrackerError,
};

/// Adapter installed when the feature is turned off in config.
///
/// Every method returns [`TrackerError::Disabled`], letting callers
/// centralise the "feature off" branch instead of gating each site
/// on an `Option<...>`.
#[derive(Debug, Default)]
pub struct DisabledTracker;

#[async_trait]
impl IssueTracker for DisabledTracker {
    async fn list_issues(
        &self,
        _project: &ProjectRef,
        _filter: &IssueFilter,
    ) -> Result<Vec<Issue>, TrackerError> {
        Err(TrackerError::Disabled)
    }

    async fn get_issue(&self, _project: &ProjectRef, _id: &IssueId) -> Result<Issue, TrackerError> {
        Err(TrackerError::Disabled)
    }

    async fn list_relations(
        &self,
        _project: &ProjectRef,
        _id: &IssueId,
    ) -> Result<Vec<IssueRelation>, TrackerError> {
        Err(TrackerError::Disabled)
    }

    async fn create_issue(
        &self,
        _project: &ProjectRef,
        _input: NewIssue,
    ) -> Result<Issue, TrackerError> {
        Err(TrackerError::Disabled)
    }

    async fn update_status(
        &self,
        _project: &ProjectRef,
        _id: &IssueId,
        _status: IssueStatus,
    ) -> Result<Issue, TrackerError> {
        Err(TrackerError::Disabled)
    }
}
