use std::collections::{HashMap, HashSet};

use super::error::AfgError;
use super::spec::{WorkflowNodeSpec, WorkflowSpec};

/// Parse a YAML workflow spec and enforce the invariants the
/// runtime relies on: non-empty node list, unique ids, `dependsOn`
/// targets exist, no cycles.
///
/// # Errors
/// `Parse` on malformed YAML, `Validation` on any invariant
/// violation above.
pub fn parse_spec(yaml: &str) -> Result<WorkflowSpec, AfgError> {
    let spec: WorkflowSpec =
        serde_yaml::from_str(yaml).map_err(|e| AfgError::Parse(e.to_string()))?;
    validate_spec(&spec)?;
    Ok(spec)
}

fn validate_spec(spec: &WorkflowSpec) -> Result<(), AfgError> {
    if spec.nodes.is_empty() {
        return Err(AfgError::Validation("workflow has no nodes".to_owned()));
    }

    let mut ids: HashSet<&str> = HashSet::new();
    for node in &spec.nodes {
        if node.id.trim().is_empty() {
            return Err(AfgError::Validation("node id is empty".to_owned()));
        }
        if !ids.insert(node.id.as_str()) {
            return Err(AfgError::Validation(format!(
                "duplicate node id: {}",
                node.id
            )));
        }
    }

    for node in &spec.nodes {
        for dep in &node.depends_on {
            if !ids.contains(dep.as_str()) {
                return Err(AfgError::Validation(format!(
                    "node '{}' depends on unknown node '{dep}'",
                    node.id
                )));
            }
        }
    }

    detect_cycle(spec)
}

fn detect_cycle(spec: &WorkflowSpec) -> Result<(), AfgError> {
    let idx: HashMap<&str, &WorkflowNodeSpec> =
        spec.nodes.iter().map(|n| (n.id.as_str(), n)).collect();
    let mut white: HashSet<&str> = idx.keys().copied().collect();
    let mut gray: HashSet<&str> = HashSet::new();
    let mut black: HashSet<&str> = HashSet::new();

    while let Some(&id) = white.iter().next() {
        visit(id, &idx, &mut white, &mut gray, &mut black)?;
    }
    Ok(())
}

fn visit<'a>(
    id: &'a str,
    idx: &HashMap<&'a str, &'a WorkflowNodeSpec>,
    white: &mut HashSet<&'a str>,
    gray: &mut HashSet<&'a str>,
    black: &mut HashSet<&'a str>,
) -> Result<(), AfgError> {
    white.remove(id);
    gray.insert(id);
    let node = idx.get(id).expect("known id");
    for dep in &node.depends_on {
        let dep_str = dep.as_str();
        if gray.contains(dep_str) {
            return Err(AfgError::Validation(format!(
                "cycle detected involving node '{dep_str}'"
            )));
        }
        if white.contains(dep_str) {
            visit(dep_str, idx, white, gray, black)?;
        }
    }
    gray.remove(id);
    black.insert(id);
    Ok(())
}

/// The first node with every dependency in `completed` and not
/// itself completed yet. `None` means the run is done (or, for a
/// spec with no runnable start node, misconfigured — callers treat
/// that as a validation error at `start_run` time).
#[must_use]
pub fn pick_next_node<'a>(
    spec: &'a WorkflowSpec,
    completed: &[String],
) -> Option<&'a WorkflowNodeSpec> {
    let done: HashSet<&str> = completed.iter().map(String::as_str).collect();
    spec.nodes.iter().find(|n| {
        !done.contains(n.id.as_str()) && n.depends_on.iter().all(|d| done.contains(d.as_str()))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_empty_nodes() {
        let yaml = "name: empty\nnodes: []\n";
        assert!(matches!(parse_spec(yaml), Err(AfgError::Validation(_))));
    }

    #[test]
    fn rejects_duplicate_ids() {
        let yaml = "
name: dup
nodes:
  - id: a
    title: A
    instructions: do a
  - id: a
    title: A again
    instructions: do a again
";
        assert!(matches!(parse_spec(yaml), Err(AfgError::Validation(_))));
    }

    #[test]
    fn rejects_dangling_dependency() {
        let yaml = "
name: dangling
nodes:
  - id: a
    title: A
    instructions: do a
    dependsOn: [ghost]
";
        assert!(matches!(parse_spec(yaml), Err(AfgError::Validation(_))));
    }

    #[test]
    fn rejects_cycle() {
        let yaml = "
name: cycle
nodes:
  - id: a
    title: A
    instructions: do a
    dependsOn: [b]
  - id: b
    title: B
    instructions: do b
    dependsOn: [a]
";
        assert!(matches!(parse_spec(yaml), Err(AfgError::Validation(_))));
    }

    #[test]
    fn accepts_a_valid_linear_graph() {
        let yaml = "
name: linear
nodes:
  - id: a
    title: A
    instructions: do a
  - id: b
    title: B
    instructions: do b
    dependsOn: [a]
";
        let spec = parse_spec(yaml).unwrap();
        assert_eq!(spec.nodes.len(), 2);
        assert_eq!(spec.version, 1);
    }

    #[test]
    fn accepts_a_dag_with_two_independent_branches() {
        // The parser accepts branching graphs even though slices 1+2's
        // dispatcher only ever runs one node at a time — the schema
        // should not silently drop a capability the runtime just
        // hasn't grown into yet.
        let yaml = "
name: branches
nodes:
  - id: root
    title: Root
    instructions: start
  - id: left
    title: Left
    instructions: left branch
    dependsOn: [root]
  - id: right
    title: Right
    instructions: right branch
    dependsOn: [root]
";
        let spec = parse_spec(yaml).unwrap();
        assert_eq!(spec.nodes.len(), 3);
    }

    #[test]
    fn pick_next_node_returns_first_runnable() {
        let spec = parse_spec(
            "
name: t
nodes:
  - id: a
    title: A
    instructions: a
  - id: b
    title: B
    instructions: b
    dependsOn: [a]
",
        )
        .unwrap();
        let first = pick_next_node(&spec, &[]).unwrap();
        assert_eq!(first.id, "a");
        let second = pick_next_node(&spec, &["a".to_owned()]).unwrap();
        assert_eq!(second.id, "b");
        assert!(pick_next_node(&spec, &["a".to_owned(), "b".to_owned()]).is_none());
    }
}
