use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;

use crate::internal::domain::{
    Issue, IssueFilter, IssueId, IssueRelation, IssueStatus, IssueTracker, NewIssue, ProjectRef,
    TrackerError,
};

use super::store::MirrorStore;

/// [`IssueTracker`] backed by the local mirror, write-through to
/// the real tracker.
///
/// The mirror is authoritative for **reads**; a tracker
/// outage never fails a read here because reads don't talk to the
/// tracker at all. Writes are different: `SQLite` is a cache, not a
/// source of truth, so every write goes straight to `upstream`
/// first. On success the returned row is upserted into the mirror
/// immediately, so a read right after a write does not wait for
/// the next syncer tick. A failed upstream write never touches the
/// mirror.
pub struct MirroredTracker {
    store: MirrorStore,
    upstream: Arc<dyn IssueTracker>,
}

impl MirroredTracker {
    #[must_use]
    pub fn new(store: MirrorStore, upstream: Arc<dyn IssueTracker>) -> Self {
        Self { store, upstream }
    }
}

fn to_domain_err(e: &sqlx::Error) -> TrackerError {
    TrackerError::Transport(format!("mirror: {e}"))
}

#[async_trait]
impl IssueTracker for MirroredTracker {
    async fn list_issues(
        &self,
        project: &ProjectRef,
        filter: &IssueFilter,
    ) -> Result<Vec<Issue>, TrackerError> {
        self.store
            .list_issues(project, filter)
            .await
            .map_err(|e| to_domain_err(&e))
    }

    async fn get_issue(&self, project: &ProjectRef, id: &IssueId) -> Result<Issue, TrackerError> {
        self.store
            .get_issue(project, id)
            .await
            .map_err(|e| to_domain_err(&e))?
            .ok_or(TrackerError::NotFound)
    }

    async fn list_relations(
        &self,
        project: &ProjectRef,
        id: &IssueId,
    ) -> Result<Vec<IssueRelation>, TrackerError> {
        self.store
            .list_relations(project, id)
            .await
            .map_err(|e| to_domain_err(&e))
    }

    async fn create_issue(
        &self,
        project: &ProjectRef,
        input: NewIssue,
    ) -> Result<Issue, TrackerError> {
        let issue = self.upstream.create_issue(project, input).await?;
        self.store
            .upsert_issue(project, &issue, Utc::now())
            .await
            .map_err(|e| to_domain_err(&e))?;
        Ok(issue)
    }

    async fn update_status(
        &self,
        project: &ProjectRef,
        id: &IssueId,
        status: IssueStatus,
    ) -> Result<Issue, TrackerError> {
        let issue = self.upstream.update_status(project, id, status).await?;
        self.store
            .upsert_issue(project, &issue, Utc::now())
            .await
            .map_err(|e| to_domain_err(&e))?;
        Ok(issue)
    }
}
