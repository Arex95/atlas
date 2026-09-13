use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use tokio::task::JoinHandle;

use crate::internal::domain::{IssueFilter, IssueTracker, ProjectRef, TrackerError};
use crate::internal::infrastructure::mirror::MirrorStore;

/// Configuration for a running syncer.
pub struct MirrorSyncerConfig {
    pub upstream: Arc<dyn IssueTracker>,
    pub store: MirrorStore,
    pub projects: Vec<ProjectRef>,
    pub interval: Duration,
}

/// Pulls issues from an upstream `IssueTracker` into the local
/// mirror on a fixed cadence.
///
/// Failure to pull one project on one tick is logged and the loop
/// continues; a single bad response does not kill the syncer. Rate
/// limiting from upstream is honoured — the loop skips the project
/// for at least `retry_after` if the tracker returned one.
pub struct MirrorSyncer {
    config: MirrorSyncerConfig,
}

impl MirrorSyncer {
    #[must_use]
    pub fn new(config: MirrorSyncerConfig) -> Self {
        Self { config }
    }

    /// Spawn the background loop; the returned handle owns the task.
    /// Dropping it cancels the task.
    pub fn spawn(self) -> JoinHandle<()> {
        tokio::spawn(self.run())
    }

    /// One full pass across every configured project. Public so
    /// tests can trigger a deterministic sync without waiting a
    /// tick.
    pub async fn sync_once(&self) {
        for project in &self.config.projects {
            self.sync_project(project).await;
        }
    }

    async fn run(self) {
        // Full pass immediately on startup (or a delta pull if a
        // cursor from a previous process is present).
        self.sync_once().await;
        loop {
            tokio::time::sleep(self.config.interval).await;
            self.sync_once().await;
        }
    }

    async fn sync_project(&self, project: &ProjectRef) {
        let cursor = match self.config.store.last_sync(project).await {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!(project = %project, error = %e, "mirror: could not read cursor");
                return;
            }
        };
        let filter = IssueFilter {
            // The syncer mirrors a whole project, never one milestone.
            milestone: None,
            status: None,
            labels: Vec::new(),
            updated_after: cursor,
        };
        let started_at = Utc::now();
        let issues = match self.config.upstream.list_issues(project, &filter).await {
            Ok(list) => list,
            Err(TrackerError::RateLimited { retry_after }) => {
                let wait = retry_after.unwrap_or(self.config.interval);
                tracing::warn!(project = %project, wait_secs = wait.as_secs(), "mirror: upstream rate-limited");
                tokio::time::sleep(wait).await;
                return;
            }
            Err(e) => {
                tracing::warn!(project = %project, error = %e, "mirror: upstream list_issues failed");
                return;
            }
        };

        for issue in &issues {
            if let Err(e) = self
                .config
                .store
                .upsert_issue(project, issue, started_at)
                .await
            {
                tracing::warn!(project = %project, issue = %issue.id, error = %e, "mirror: upsert failed");
                continue;
            }
            match self
                .config
                .upstream
                .list_relations(project, &issue.id)
                .await
            {
                Ok(rels) => {
                    if let Err(e) = self
                        .config
                        .store
                        .replace_relations(project, &issue.id, &rels)
                        .await
                    {
                        tracing::warn!(project = %project, issue = %issue.id, error = %e, "mirror: relations upsert failed");
                    }
                }
                Err(e) => {
                    tracing::warn!(project = %project, issue = %issue.id, error = %e, "mirror: upstream list_relations failed");
                }
            }
        }

        if let Err(e) = self.config.store.set_last_sync(project, started_at).await {
            tracing::warn!(project = %project, error = %e, "mirror: cursor update failed");
        }
    }
}
