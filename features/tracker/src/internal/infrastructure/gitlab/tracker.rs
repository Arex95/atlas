use async_trait::async_trait;
use url::Url;

use crate::internal::domain::{
    Issue, IssueFilter, IssueId, IssueRelation, IssueStatus, IssueTracker, NewIssue, ProjectRef,
    TrackerError,
};

use super::client::HttpClient;
use super::mapping::{GitLabIssue, GitLabIssueLink};

/// Adapter for the GitLab REST v4 API.
///
/// Instances are cheap to clone through the `Arc` the runtime holds.
pub struct GitLabTracker {
    http: HttpClient,
}

impl GitLabTracker {
    /// # Errors
    /// Fails if the underlying HTTP client cannot be constructed
    /// (e.g. TLS backend init).
    pub fn new(base_url: Url, token: String, user_agent: String) -> Result<Self, String> {
        Ok(Self {
            http: HttpClient::new(base_url, token, user_agent)?,
        })
    }
}

#[async_trait]
impl IssueTracker for GitLabTracker {
    async fn list_issues(
        &self,
        project: &ProjectRef,
        filter: &IssueFilter,
    ) -> Result<Vec<Issue>, TrackerError> {
        let project_segment = urlencoding::encode(&project.path()).into_owned();
        let path = format!("/api/v4/projects/{project_segment}/issues");
        let mut query: Vec<(&str, String)> = Vec::new();
        if let Some(status) = filter.status {
            query.push((
                "state",
                match status {
                    IssueStatus::Open => "opened".to_owned(),
                    IssueStatus::Closed => "closed".to_owned(),
                },
            ));
        }
        if !filter.labels.is_empty() {
            let joined = filter
                .labels
                .iter()
                .map(|l| l.0.as_str())
                .collect::<Vec<_>>()
                .join(",");
            query.push(("labels", joined));
        }
        if let Some(milestone) = &filter.milestone {
            query.push(("milestone", milestone.clone()));
        }
        if let Some(ts) = filter.updated_after {
            query.push(("updated_after", ts.to_rfc3339()));
        }
        query.push(("per_page", "100".to_owned()));

        let payload: Vec<GitLabIssue> = self.http.get_json(&path, &query).await?;
        Ok(payload.into_iter().map(Issue::from).collect())
    }

    async fn get_issue(&self, project: &ProjectRef, id: &IssueId) -> Result<Issue, TrackerError> {
        let project_segment = urlencoding::encode(&project.path()).into_owned();
        let iid = url_safe_iid(id)?;
        let path = format!("/api/v4/projects/{project_segment}/issues/{iid}");
        let payload: GitLabIssue = self.http.get_json(&path, &[]).await?;
        Ok(payload.into())
    }

    async fn list_relations(
        &self,
        project: &ProjectRef,
        id: &IssueId,
    ) -> Result<Vec<IssueRelation>, TrackerError> {
        let project_segment = urlencoding::encode(&project.path()).into_owned();
        let iid = url_safe_iid(id)?;
        let path = format!("/api/v4/projects/{project_segment}/issues/{iid}/links");
        let payload: Vec<GitLabIssueLink> = self.http.get_json(&path, &[]).await?;
        Ok(payload
            .into_iter()
            .filter_map(GitLabIssueLink::into_relation)
            .collect())
    }

    async fn create_issue(
        &self,
        project: &ProjectRef,
        input: NewIssue,
    ) -> Result<Issue, TrackerError> {
        let project_segment = urlencoding::encode(&project.path()).into_owned();
        let path = format!("/api/v4/projects/{project_segment}/issues");
        let mut form: Vec<(&str, String)> = vec![("title", input.title)];
        if let Some(description) = input.description {
            form.push(("description", description));
        }
        if !input.labels.is_empty() {
            let joined = input
                .labels
                .iter()
                .map(|l| l.0.as_str())
                .collect::<Vec<_>>()
                .join(",");
            form.push(("labels", joined));
        }
        let payload: GitLabIssue = self.http.post_json(&path, &form).await?;
        Ok(payload.into())
    }

    async fn update_status(
        &self,
        project: &ProjectRef,
        id: &IssueId,
        status: IssueStatus,
    ) -> Result<Issue, TrackerError> {
        let project_segment = urlencoding::encode(&project.path()).into_owned();
        let iid = url_safe_iid(id)?;
        let path = format!("/api/v4/projects/{project_segment}/issues/{iid}");
        let state_event = match status {
            IssueStatus::Open => "reopen",
            IssueStatus::Closed => "close",
        };
        let form = [("state_event", state_event.to_owned())];
        let payload: GitLabIssue = self.http.put_json(&path, &form).await?;
        Ok(payload.into())
    }
}

fn url_safe_iid(id: &IssueId) -> Result<u64, TrackerError> {
    id.0.parse::<u64>().map_err(|_| {
        TrackerError::Transport(format!("issue id {:?} is not a valid GitLab iid", id.0))
    })
}
