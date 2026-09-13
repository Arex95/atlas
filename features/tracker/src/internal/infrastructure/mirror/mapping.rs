use chrono::{DateTime, Utc};
use serde_json::json;

use crate::internal::domain::{Issue, IssueId, IssueRelation, IssueStatus, Label};

pub(super) fn status_to_wire(s: IssueStatus) -> &'static str {
    match s {
        IssueStatus::Open => "open",
        IssueStatus::Closed => "closed",
    }
}

pub(super) fn status_from_wire(s: &str) -> IssueStatus {
    if s == "closed" {
        IssueStatus::Closed
    } else {
        IssueStatus::Open
    }
}

pub(super) fn labels_to_json(labels: &[Label]) -> String {
    let raw: Vec<&str> = labels.iter().map(|l| l.0.as_str()).collect();
    json!(raw).to_string()
}

pub(super) fn labels_from_json(raw: &str) -> Vec<Label> {
    serde_json::from_str::<Vec<String>>(raw)
        .map(|v| v.into_iter().map(Label).collect())
        .unwrap_or_default()
}

pub(super) fn relation_to_wire(rel: &IssueRelation) -> (&'static str, &IssueId) {
    match rel {
        IssueRelation::Blocks(id) => ("blocks", id),
        IssueRelation::BlockedBy(id) => ("blocked_by", id),
        IssueRelation::RelatesTo(id) => ("relates_to", id),
    }
}

pub(super) fn relation_from_wire(kind: &str, target: String) -> Option<IssueRelation> {
    match kind {
        "blocks" => Some(IssueRelation::Blocks(IssueId(target))),
        "blocked_by" => Some(IssueRelation::BlockedBy(IssueId(target))),
        "relates_to" => Some(IssueRelation::RelatesTo(IssueId(target))),
        _ => None,
    }
}

pub(super) fn parse_ts(s: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.with_timezone(&Utc))
        .ok()
}

/// Bundles a `mirror_issues` row's columns so `issue_from_row`
/// doesn't need eight positional arguments (clippy's
/// `too_many_arguments`, at 7).
pub(super) struct MirrorIssueRow {
    pub id: String,
    pub title: String,
    pub status: String,
    pub labels_json: String,
    pub author: String,
    pub created_at: String,
    pub updated_at: String,
    pub description: String,
    pub milestone: String,
}

pub(super) fn issue_from_row(row: MirrorIssueRow) -> Option<Issue> {
    Some(Issue {
        id: IssueId(row.id),
        title: row.title,
        status: status_from_wire(&row.status),
        labels: labels_from_json(&row.labels_json),
        author: row.author,
        created_at: parse_ts(&row.created_at)?,
        updated_at: parse_ts(&row.updated_at)?,
        description: Some(row.description),
        // Empty means no milestone; see the column's migration.
        milestone: Some(row.milestone).filter(|m| !m.is_empty()),
    })
}
