//! What an agent is asked to stay within while executing a node.
//!
//! **This is not containment, and nothing here should be described as
//! though it were.** A workflow node is dispatched to a session whose
//! agent holds a real terminal; a list of permitted tool names does
//! not bound what that agent can do, because it can do anything its
//! user can do by typing. Atlas's trust model says the only boundary
//! is the operating system, and that remains true with this in place.
//!
//! What it *is* worth: limiting accidents. A node that says "run the
//! tests" has no business closing an issue, and an agent that decides
//! otherwise — through a misread instruction, a hallucinated plan, or
//! another agent's message — is stopped at the one surface Atlas
//! controls. Accidents between agents are a documented failure mode
//! rather than a hypothetical one, which is the whole reason this is
//! worth its cost.
//!
//! Absent means unrestricted. A workflow that says nothing about
//! tools gets the behaviour it had before this existed, which is what
//! keeps it from being a migration.

use serde::{Deserialize, Serialize};

/// Tools an agent may always call, whatever a node declares.
///
/// Without these a scoped node is a trap: the agent cannot report that
/// it finished, cannot see the run it is executing, and the run stalls
/// until it times out. A scope that can deadlock the thing it scopes
/// would be worse than no scope.
pub const ALWAYS_ALLOWED: &[&str] = &["afg.submit_task_result", "afg.get_run"];

/// One node's declared scope.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolScope {
    /// Exact tool names, or a `prefix.*` wildcard covering a family —
    /// `tracker.*` rather than six names that drift apart as tools are
    /// added.
    patterns: Vec<String>,
}

impl ToolScope {
    #[must_use]
    pub fn new(patterns: Vec<String>) -> Self {
        Self { patterns }
    }

    /// Whether this scope restricts anything at all.
    ///
    /// An empty list means "said nothing", not "permit nothing". The
    /// other reading would turn every existing workflow into one that
    /// can call no tools the moment this shipped.
    #[must_use]
    pub fn is_unrestricted(&self) -> bool {
        self.patterns.is_empty()
    }

    #[must_use]
    pub fn patterns(&self) -> &[String] {
        &self.patterns
    }

    /// Whether `tool` is within this scope.
    #[must_use]
    pub fn permits(&self, tool: &str) -> bool {
        if self.is_unrestricted() || ALWAYS_ALLOWED.contains(&tool) {
            return true;
        }
        self.patterns.iter().any(|p| matches_pattern(p, tool))
    }
}

/// `tracker.*` covers `tracker.list_issues`; everything else is an
/// exact name.
///
/// Deliberately not a glob library. The only shape a workflow author
/// needs is "this family", and a richer syntax would invite patterns
/// whose meaning has to be guessed at from the outside.
fn matches_pattern(pattern: &str, tool: &str) -> bool {
    match pattern.strip_suffix(".*") {
        // The dot is part of the prefix so `tracker.*` cannot match a
        // hypothetical `trackerthing.foo`.
        Some(prefix) => tool
            .strip_prefix(prefix)
            .is_some_and(|r| r.starts_with('.')),
        None => pattern == tool,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saying_nothing_permits_everything() {
        let scope = ToolScope::default();
        assert!(scope.is_unrestricted());
        assert!(scope.permits("terminal.write"));
        assert!(scope.permits("tracker.close_issue"));
    }

    #[test]
    fn an_exact_name_permits_only_itself() {
        let scope = ToolScope::new(vec!["notes.write".to_owned()]);
        assert!(scope.permits("notes.write"));
        assert!(!scope.permits("notes.read"));
        assert!(!scope.permits("tracker.close_issue"));
    }

    #[test]
    fn a_family_wildcard_covers_the_family_and_nothing_else() {
        let scope = ToolScope::new(vec!["tracker.*".to_owned()]);
        assert!(scope.permits("tracker.list_issues"));
        assert!(scope.permits("tracker.close_issue"));
        assert!(!scope.permits("terminal.write"));
    }

    #[test]
    fn a_wildcard_does_not_leak_past_its_dot() {
        // `tracker.*` must not match a tool family whose name merely
        // starts with the same letters.
        let scope = ToolScope::new(vec!["tracker.*".to_owned()]);
        assert!(!scope.permits("trackerthing.foo"));
        assert!(!scope.permits("tracker"));
    }

    #[test]
    fn reporting_a_result_is_always_permitted() {
        // Otherwise a scoped node is a trap: the agent cannot say it
        // finished, and the run stalls until it times out.
        let scope = ToolScope::new(vec!["notes.read".to_owned()]);
        assert!(scope.permits("afg.submit_task_result"));
        assert!(scope.permits("afg.get_run"));
        assert!(!scope.permits("afg.start_run"));
    }

    #[test]
    fn several_patterns_are_a_union() {
        let scope = ToolScope::new(vec!["tracker.*".to_owned(), "notes.write".to_owned()]);
        assert!(scope.permits("tracker.get_issue"));
        assert!(scope.permits("notes.write"));
        assert!(!scope.permits("notes.delete"));
    }
}
