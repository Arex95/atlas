use std::collections::HashMap;
use std::sync::Mutex;

use async_trait::async_trait;
use chrono::Utc;

use crate::internal::domain::{
    Issue, IssueFilter, IssueId, IssueRelation, IssueStatus, IssueTracker, NewIssue, ProjectRef,
    TrackerError,
};

type StoreKey = (ProjectRef, IssueId);
type StoreEntry = (Issue, Vec<IssueRelation>);

/// In-memory `IssueTracker` for tests and consumer development.
///
/// Not `pub` outside the crate is intentional in production builds;
/// exposed to integration tests via the `test-support` feature so
/// downstream crates can pull it in for their own tests.
pub struct FakeTracker {
    state: Mutex<HashMap<StoreKey, StoreEntry>>,
    // Starts past any hand-seeded fixture id ("1", "2", ...) so
    // `create_issue` in a test never collides with `insert`-ed data.
    next_id: Mutex<u64>,
}

impl Default for FakeTracker {
    fn default() -> Self {
        Self {
            state: Mutex::new(HashMap::new()),
            next_id: Mutex::new(1000),
        }
    }
}

impl FakeTracker {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Seed the fake with a fixture.
    ///
    /// # Panics
    /// If the internal mutex is poisoned by a previous panic in
    /// another thread — this is a test aid and does not swallow that.
    pub fn insert(&self, project: ProjectRef, issue: Issue, relations: Vec<IssueRelation>) {
        let id = issue.id.clone();
        self.state
            .lock()
            .expect("fake tracker state poisoned")
            .insert((project, id), (issue, relations));
    }
}

#[async_trait]
impl IssueTracker for FakeTracker {
    async fn list_issues(
        &self,
        project: &ProjectRef,
        filter: &IssueFilter,
    ) -> Result<Vec<Issue>, TrackerError> {
        let guard = self.state.lock().expect("fake tracker state poisoned");
        let mut out: Vec<Issue> = guard
            .iter()
            .filter(|((p, _), _)| p == project)
            .map(|(_, (issue, _))| issue.clone())
            .filter(|i| filter.status.is_none_or(|s| i.status == s))
            .filter(|i| {
                filter
                    .labels
                    .iter()
                    .all(|wanted| i.labels.iter().any(|l| l == wanted))
            })
            .filter(|i| filter.updated_after.is_none_or(|ts| i.updated_at >= ts))
            .filter(|i| {
                filter
                    .milestone
                    .as_ref()
                    .is_none_or(|wanted| i.milestone.as_ref() == Some(wanted))
            })
            .collect();
        out.sort_by(|a, b| a.updated_at.cmp(&b.updated_at).reverse());
        Ok(out)
    }

    async fn get_issue(&self, project: &ProjectRef, id: &IssueId) -> Result<Issue, TrackerError> {
        self.state
            .lock()
            .expect("fake tracker state poisoned")
            .get(&(project.clone(), id.clone()))
            .map(|(issue, _)| issue.clone())
            .ok_or(TrackerError::NotFound)
    }

    async fn list_relations(
        &self,
        project: &ProjectRef,
        id: &IssueId,
    ) -> Result<Vec<IssueRelation>, TrackerError> {
        self.state
            .lock()
            .expect("fake tracker state poisoned")
            .get(&(project.clone(), id.clone()))
            .map(|(_, rels)| rels.clone())
            .ok_or(TrackerError::NotFound)
    }

    async fn create_issue(
        &self,
        project: &ProjectRef,
        input: NewIssue,
    ) -> Result<Issue, TrackerError> {
        if input.title.trim().is_empty() {
            return Err(TrackerError::Invalid("title must not be empty".to_owned()));
        }
        let mut next_id = self.next_id.lock().expect("fake tracker id poisoned");
        let id = IssueId(next_id.to_string());
        *next_id += 1;
        drop(next_id);

        let now = Utc::now();
        let issue = Issue {
            id: id.clone(),
            title: input.title,
            status: IssueStatus::Open,
            labels: input.labels,
            author: "fake-tracker".to_owned(),
            created_at: now,
            updated_at: now,
            description: input.description,
            milestone: None,
        };
        self.state
            .lock()
            .expect("fake tracker state poisoned")
            .insert((project.clone(), id), (issue.clone(), Vec::new()));
        Ok(issue)
    }

    async fn update_status(
        &self,
        project: &ProjectRef,
        id: &IssueId,
        status: IssueStatus,
    ) -> Result<Issue, TrackerError> {
        let mut guard = self.state.lock().expect("fake tracker state poisoned");
        let (issue, _) = guard
            .get_mut(&(project.clone(), id.clone()))
            .ok_or(TrackerError::NotFound)?;
        issue.status = status;
        issue.updated_at = Utc::now();
        Ok(issue.clone())
    }
}
