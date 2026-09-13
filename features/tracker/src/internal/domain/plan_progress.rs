//! Acceptance-criteria progress derived from issue descriptions.
//!
//! No new schema: the source of truth is the GitLab task-list
//! checkboxes (`- [ ]` / `- [x]`) every issue already carries under
//! a `## Acceptance criteria` heading (a declared
//! convention). This module only counts them — the plan itself
//! lives in the tracker, never in Atlas.

use serde::Serialize;

use super::issue::{Issue, IssueId};

const HEADING: &str = "## Acceptance criteria";

/// Raw ticked/total count. `total == 0` means the issue has no
/// acceptance-criteria section at all, not that it is 100% done.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct AcceptanceCriteriaCount {
    pub done: u32,
    pub total: u32,
}

impl AcceptanceCriteriaCount {
    fn add(&mut self, other: Self) {
        self.done += other.done;
        self.total += other.total;
    }
}

/// One issue's contribution to a [`PlanProgress`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct IssuePlanProgress {
    pub id: IssueId,
    pub title: String,
    pub done: u32,
    pub total: u32,
}

/// Aggregate progress across every issue a caller's [`IssueFilter`]
/// selected.
///
/// [`IssueFilter`]: super::filter::IssueFilter
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct PlanProgress {
    pub done: u32,
    pub total: u32,
    pub by_issue: Vec<IssuePlanProgress>,
}

/// Count checkboxes under a literal `## Acceptance criteria`
/// heading in `description`.
///
/// The heading match is case-sensitive and exact — it is the
/// convention the team writes down, not a natural-language heuristic.
/// Scanning stops at the next `##` heading or the end of the text.
/// Checkboxes anywhere else in the description (scope-excluded
/// bullets, verification checklists, sub-headers nested under the
/// section) are counted if they fall inside that span; sub-headers
/// (`###` and deeper) do not end the section, only another `##` does.
#[must_use]
pub fn parse_acceptance_criteria(description: &str) -> AcceptanceCriteriaCount {
    let mut lines = description.lines();
    let Some(_) = lines.by_ref().find(|line| line.trim() == HEADING) else {
        return AcceptanceCriteriaCount::default();
    };

    let mut count = AcceptanceCriteriaCount::default();
    for line in lines {
        let trimmed = line.trim_start();
        if trimmed.starts_with("## ") || trimmed == "##" {
            break;
        }
        if let Some(checked) = checkbox_state(trimmed) {
            count.total += 1;
            if checked {
                count.done += 1;
            }
        }
    }
    count
}

/// `Some(true)` for `- [x]`/`- [X]`, `Some(false)` for `- [ ]`,
/// `None` if the line is not a task-list item at all.
fn checkbox_state(trimmed: &str) -> Option<bool> {
    let rest = trimmed
        .strip_prefix("- [")
        .or_else(|| trimmed.strip_prefix("* ["))?;
    let mark = rest.chars().next()?;
    if rest.as_bytes().get(1) != Some(&b']') {
        return None;
    }
    match mark {
        ' ' => Some(false),
        'x' | 'X' => Some(true),
        _ => None,
    }
}

/// Fold [`parse_acceptance_criteria`] over a set of issues into one
/// aggregate. Zero-criteria issues still appear in `by_issue` at
/// `0/0` — that absence is itself information, not noise to hide.
#[must_use]
pub fn aggregate_plan_progress(issues: &[Issue]) -> PlanProgress {
    let mut total = AcceptanceCriteriaCount::default();
    let mut by_issue = Vec::with_capacity(issues.len());
    for issue in issues {
        let count = parse_acceptance_criteria(issue.description.as_deref().unwrap_or(""));
        total.add(count);
        by_issue.push(IssuePlanProgress {
            id: issue.id.clone(),
            title: issue.title.clone(),
            done: count.done,
            total: count.total,
        });
    }
    PlanProgress {
        done: total.done,
        total: total.total,
        by_issue,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_heading_is_zero_zero() {
        let count = parse_acceptance_criteria("just some notes, no checklist here");
        assert_eq!(count, AcceptanceCriteriaCount::default());
    }

    #[test]
    fn counts_mixed_done_and_not_done() {
        let description =
            "## What\nsomething\n\n## Acceptance criteria\n- [x] one\n- [ ] two\n- [X] three\n";
        let count = parse_acceptance_criteria(description);
        assert_eq!(count, AcceptanceCriteriaCount { done: 2, total: 3 });
    }

    #[test]
    fn stops_at_next_heading() {
        let description =
            "## Acceptance criteria\n- [x] counted\n\n## Verification\n- [ ] not counted\n";
        let count = parse_acceptance_criteria(description);
        assert_eq!(count, AcceptanceCriteriaCount { done: 1, total: 1 });
    }

    #[test]
    fn heading_immediately_followed_by_another_heading_is_zero() {
        let description = "## Acceptance criteria\n## Verification\n- [ ] not counted\n";
        let count = parse_acceptance_criteria(description);
        assert_eq!(count, AcceptanceCriteriaCount::default());
    }

    #[test]
    fn nested_sub_headers_do_not_end_the_section() {
        let description = "## Acceptance criteria\n\n`scripts/mcp-connect.sh`:\n- [x] one\n\n`docs/connecting-agents.md`:\n- [ ] two\n\n## Verification\n- [ ] not counted\n";
        let count = parse_acceptance_criteria(description);
        assert_eq!(count, AcceptanceCriteriaCount { done: 1, total: 2 });
    }

    #[test]
    fn aggregate_includes_zero_criteria_issues() {
        let issues = vec![
            Issue {
                id: IssueId("1".to_owned()),
                title: "has criteria".to_owned(),
                status: super::super::issue::IssueStatus::Open,
                labels: vec![],
                author: "arex95".to_owned(),
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
                description: Some("## Acceptance criteria\n- [x] a\n- [ ] b\n".to_owned()),
                milestone: None,
            },
            Issue {
                id: IssueId("2".to_owned()),
                title: "no criteria".to_owned(),
                status: super::super::issue::IssueStatus::Open,
                labels: vec![],
                author: "arex95".to_owned(),
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
                description: None,
                milestone: None,
            },
        ];
        let progress = aggregate_plan_progress(&issues);
        assert_eq!(progress.done, 1);
        assert_eq!(progress.total, 2);
        assert_eq!(progress.by_issue.len(), 2);
        assert_eq!(progress.by_issue[1].total, 0);
    }
}
