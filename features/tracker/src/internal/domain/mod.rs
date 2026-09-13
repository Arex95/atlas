//! Tracker-agnostic domain vocabulary and the port trait.
//!
//! The types here describe issues in the language Atlas uses across
//! the codebase; adapter crates translate their vendor payloads into
//! these types and never leak vendor concerns upward.

mod error;
mod filter;
mod issue;
mod plan_progress;
mod port;
mod project;

pub use error::TrackerError;
pub use filter::IssueFilter;
pub use issue::{Issue, IssueId, IssueRelation, IssueStatus, Label, NewIssue};
pub use plan_progress::{
    AcceptanceCriteriaCount, IssuePlanProgress, PlanProgress, aggregate_plan_progress,
    parse_acceptance_criteria,
};
pub use port::IssueTracker;
pub use project::{ProjectRef, ProjectRefError};
