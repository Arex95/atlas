use chrono::{DateTime, Utc};
use serde::Deserialize;

use crate::internal::domain::{Issue, IssueId, IssueRelation, IssueStatus, Label};

#[derive(Debug, Deserialize)]
pub(super) struct GitLabIssue {
    pub iid: u64,
    pub title: String,
    pub state: String,
    #[serde(default)]
    pub labels: Vec<String>,
    pub author: GitLabAuthor,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub milestone: Option<GitLabMilestone>,
}

/// Only the title is read. The rest of GitLab's milestone object —
/// dates, state, ids — describes the milestone, and Atlas is not the
/// place that owns milestones.
#[derive(Debug, Deserialize)]
pub(super) struct GitLabMilestone {
    pub title: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct GitLabAuthor {
    pub username: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct GitLabIssueLink {
    pub iid: u64,
    pub link_type: String,
}

impl From<GitLabIssue> for Issue {
    fn from(g: GitLabIssue) -> Self {
        let status = if g.state == "closed" {
            IssueStatus::Closed
        } else {
            IssueStatus::Open
        };
        Self {
            id: IssueId(g.iid.to_string()),
            title: g.title,
            status,
            labels: g.labels.into_iter().map(Label).collect(),
            author: g.author.username,
            created_at: g.created_at,
            updated_at: g.updated_at,
            description: g.description,
            milestone: g.milestone.map(|m| m.title),
        }
    }
}

impl GitLabIssueLink {
    pub(super) fn into_relation(self) -> Option<IssueRelation> {
        let target = IssueId(self.iid.to_string());
        match self.link_type.as_str() {
            "blocks" => Some(IssueRelation::Blocks(target)),
            "is_blocked_by" => Some(IssueRelation::BlockedBy(target)),
            "relates_to" => Some(IssueRelation::RelatesTo(target)),
            _ => None,
        }
    }
}
