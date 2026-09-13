use std::sync::Arc;

use crate::internal::domain::{
    Issue, IssueFilter, IssueId, IssueRelation, IssueStatus, IssueTracker, NewIssue, PlanProgress,
    ProjectRef, TrackerError, aggregate_plan_progress,
};

/// The composition-side handle callers hold instead of a bare
/// `Arc<dyn IssueTracker>`.
///
/// It lets the binary (and, later, feature crates) reason about
/// "the tracker" as one object regardless of whether it is
/// disabled, a real adapter, or a fake in a test. The runtime
/// forwards to the port; it does not add business rules — those
/// belong in the callers that own them.
#[derive(Clone)]
pub struct TrackerRuntime {
    inner: Arc<dyn IssueTracker>,
}

impl TrackerRuntime {
    #[must_use]
    pub fn new(tracker: Arc<dyn IssueTracker>) -> Self {
        Self { inner: tracker }
    }

    /// # Errors
    /// Propagates whatever the underlying [`IssueTracker`] returns.
    pub async fn list_issues(
        &self,
        project: &ProjectRef,
        filter: &IssueFilter,
    ) -> Result<Vec<Issue>, TrackerError> {
        self.inner.list_issues(project, filter).await
    }

    /// # Errors
    /// Propagates whatever the underlying [`IssueTracker`] returns.
    pub async fn get_issue(
        &self,
        project: &ProjectRef,
        id: &IssueId,
    ) -> Result<Issue, TrackerError> {
        self.inner.get_issue(project, id).await
    }

    /// # Errors
    /// Propagates whatever the underlying [`IssueTracker`] returns.
    pub async fn list_relations(
        &self,
        project: &ProjectRef,
        id: &IssueId,
    ) -> Result<Vec<IssueRelation>, TrackerError> {
        self.inner.list_relations(project, id).await
    }

    /// # Errors
    /// Propagates whatever the underlying [`IssueTracker`] returns.
    pub async fn create_issue(
        &self,
        project: &ProjectRef,
        input: NewIssue,
    ) -> Result<Issue, TrackerError> {
        self.inner.create_issue(project, input).await
    }

    /// # Errors
    /// Propagates whatever the underlying [`IssueTracker`] returns.
    pub async fn update_status(
        &self,
        project: &ProjectRef,
        id: &IssueId,
        status: IssueStatus,
    ) -> Result<Issue, TrackerError> {
        self.inner.update_status(project, id, status).await
    }

    /// Sugar over [`Self::update_status`] with [`IssueStatus::Closed`].
    ///
    /// # Errors
    /// Propagates whatever the underlying [`IssueTracker`] returns.
    pub async fn close_issue(
        &self,
        project: &ProjectRef,
        id: &IssueId,
    ) -> Result<Issue, TrackerError> {
        self.update_status(project, id, IssueStatus::Closed).await
    }

    /// Acceptance-criteria progress across every issue `filter`
    /// selects. Not a new tracker capability — derived entirely
    /// from `list_issues` and each issue's `description`, so no
    /// [`IssueTracker`] implementation needs to know this exists.
    ///
    /// # Errors
    /// Propagates whatever the underlying [`IssueTracker`] returns.
    pub async fn plan_progress(
        &self,
        project: &ProjectRef,
        filter: &IssueFilter,
    ) -> Result<PlanProgress, TrackerError> {
        let issues = self.list_issues(project, filter).await?;
        Ok(aggregate_plan_progress(&issues))
    }
}
