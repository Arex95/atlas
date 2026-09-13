use std::fmt;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Tracker-agnostic issue identifier.
///
/// Stored as a string because the source of truth is the tracker:
/// GitLab emits `iid` per project; GitHub emits `number`. Atlas
/// treats both as opaque and does not synthesise its own ids.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct IssueId(pub String);

impl fmt::Display for IssueId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<&str> for IssueId {
    fn from(s: &str) -> Self {
        Self(s.to_owned())
    }
}

impl From<u64> for IssueId {
    fn from(n: u64) -> Self {
        Self(n.to_string())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum IssueStatus {
    Open,
    Closed,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Label(pub String);

impl fmt::Display for Label {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<&str> for Label {
    fn from(s: &str) -> Self {
        Self(s.to_owned())
    }
}

/// One relation from *this* issue's perspective to another.
///
/// `Blocks(x)` and `BlockedBy(x)` are both kept explicitly because
/// `list_relations(A)` cannot derive the inverse without also
/// querying the other end — the querying side needs the truth
/// GitLab (and the other trackers) return.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", content = "target")]
pub enum IssueRelation {
    Blocks(IssueId),
    BlockedBy(IssueId),
    RelatesTo(IssueId),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Issue {
    pub id: IssueId,
    pub title: String,
    pub status: IssueStatus,
    pub labels: Vec<Label>,
    pub author: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    /// Raw markdown body. `None` means the tracker returned no body
    /// (or the mirror predates this field); an empty string means
    /// the issue genuinely has no description. Both parse to zero
    /// acceptance criteria — see `plan_progress`.
    pub description: Option<String>,
    /// The milestone the tracker has this issue in, if any.
    ///
    /// Carried because a milestone is how a team already names a
    /// roadmap — "the thing we are shipping next" — and asking how far
    /// along one is the only way `plan_progress` answers a question
    /// about a plan rather than about a label somebody remembered to
    /// apply. `None` means no milestone, not "unknown".
    pub milestone: Option<String>,
}

/// What a caller supplies to create a new issue.
///
/// No `id`, `author`, or timestamps — the tracker assigns those.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NewIssue {
    pub title: String,
    pub description: Option<String>,
    pub labels: Vec<Label>,
}
