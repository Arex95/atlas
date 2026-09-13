//! What a change reaches.
//!
//! The question an agent should ask before editing and almost never
//! can: *if I change this file, what else is involved?* The graph
//! already knows — every resolved import is a "this file needs that
//! file" — and git already knows what changed. This joins them.
//!
//! **No risk score.** The obvious shape is a red/amber/green label per
//! change, and it would be the scorecard mistake again: a single word
//! hiding which of several unrelated facts produced it, and inviting
//! the reader to act on the label instead of the thing. What comes out
//! instead is located: these files changed, these files import them at
//! this distance, these of them are the ones many other files depend
//! on, these declared layers and modules are involved.
//!
//! Two honesty requirements, both carried in the report:
//!
//! * **The radius is only as good as the graph's imports.** It is
//!   computed from resolved imports alone, so the report publishes how
//!   many there were and how many were not resolved.
//! * **The graph may predate the change.** A file edited since the last
//!   index is reported as such. Reverse dependents survive that — they
//!   come from *other* files' imports — but a change that added or
//!   removed an import of its own is invisible until a reindex.

use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};
use std::path::Path;

use serde::Serialize;

use crate::internal::application::layer_check::LayerRules;
use crate::internal::domain::{GraphError, LayersFile};
use crate::internal::infrastructure::{ChangedFile, GraphStore};

/// How far to follow reverse dependencies by default.
///
/// Three hops is where a file-level radius stops being informative on
/// a real repository: everything is reachable from everything through
/// a shared error type eventually, and a report naming half the
/// repository is one nobody reads.
pub const DEFAULT_DEPTH: usize = 3;
pub const MAX_DEPTH: usize = 10;

/// A file that imports something that changed.
#[derive(Clone, Debug, Serialize)]
pub struct Impacted {
    pub path: String,
    /// 1 = imports a changed file directly.
    pub distance: usize,
}

/// One changed file, placed in the graph.
#[derive(Clone, Debug, Serialize)]
pub struct ChangedEntry {
    pub path: String,
    pub kind: crate::internal::infrastructure::ChangeKind,
    pub renamed_from: Option<String>,
    /// False for a file the graph has never seen — newly created, or
    /// one the indexer skips. Its radius cannot be computed, and
    /// saying so beats reporting an empty one.
    pub in_graph: bool,
    /// The declared layer this file belongs to, when the project
    /// declares layers and one claims it.
    pub layer: Option<String>,
    /// How many files import this one directly.
    pub imported_by: usize,
}

/// How much the report should be trusted.
#[derive(Clone, Debug, Serialize)]
pub struct ImpactEvidence {
    pub resolved_imports: usize,
    pub unresolved_imports: usize,
    /// Changed files the graph has no node for.
    pub changed_not_in_graph: usize,
    /// Changed files whose content differs from what was indexed, so
    /// the graph predates the edit.
    pub changed_since_indexed: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct ImpactReport {
    pub changed: Vec<ChangedEntry>,
    /// Files reached by following imports backwards, nearest first.
    pub impact: Vec<Impacted>,
    pub modules_touched: Vec<String>,
    pub layers_touched: Vec<String>,
    pub depth: usize,
    pub evidence: ImpactEvidence,
}

pub struct ImpactAnalyser {
    store: GraphStore,
}

impl ImpactAnalyser {
    #[must_use]
    pub fn new(store: GraphStore) -> Self {
        Self { store }
    }

    /// Places a set of changed files in the graph and follows what
    /// depends on them.
    ///
    /// # Errors
    /// `Storage` on any SQL failure.
    pub async fn analyse(
        &self,
        project: &str,
        root: &Path,
        changes: &[ChangedFile],
        depth: usize,
    ) -> Result<ImpactReport, GraphError> {
        let depth = depth.clamp(1, MAX_DEPTH);

        let files = self.store.file_nodes(project).await?;
        let imports = self.store.resolved_imports(project).await?;
        let unresolved = self.store.unresolved_import_count(project).await?;
        let hashes = self.store.file_hashes(project).await?;
        let (layers_source, _) = self.store.project_layers(project).await?;

        let id_of: HashMap<&str, &str> = files
            .iter()
            .map(|f| (f.fqn.as_str(), f.id.as_str()))
            .collect();
        let path_of: HashMap<&str, &str> = files
            .iter()
            .map(|f| (f.id.as_str(), f.fqn.as_str()))
            .collect();

        // Reverse edges: who imports this file. The radius runs
        // backwards because the question is what a change reaches, not
        // what it needs.
        let mut importers: HashMap<&str, Vec<&str>> = HashMap::new();
        let mut fan_in: HashMap<&str, usize> = HashMap::new();
        for (src, dst, _) in &imports {
            importers
                .entry(dst.as_str())
                .or_default()
                .push(src.as_str());
            *fan_in.entry(dst.as_str()).or_default() += 1;
        }

        let declared = layers_source
            .as_deref()
            .and_then(|s| LayersFile::parse(s).ok());
        let rules = declared
            .as_ref()
            .filter(|d| !d.layers.is_empty())
            .and_then(|d| LayerRules::compile(d).ok());

        let mut placed = Vec::with_capacity(changes.len());
        let mut seeds: Vec<&str> = Vec::new();
        let mut not_in_graph = 0;
        let mut stale = 0;

        for change in changes {
            let id = id_of.get(change.path.as_str()).copied();
            if id.is_none() {
                not_in_graph += 1;
            } else if hashes
                .get(&change.path)
                .is_some_and(|indexed| !content_matches(&root.join(&change.path), indexed))
            {
                stale += 1;
            }
            if let Some(id) = id {
                seeds.push(id);
            }

            placed.push(ChangedEntry {
                path: change.path.clone(),
                kind: change.kind,
                renamed_from: change.renamed_from.clone(),
                in_graph: id.is_some(),
                layer: rules
                    .as_ref()
                    .and_then(|r| r.layer_of(&change.path))
                    .map(ToOwned::to_owned),
                imported_by: id.and_then(|i| fan_in.get(i).copied()).unwrap_or(0),
            });
        }

        let impact = spread(&seeds, &importers, &path_of, depth);

        // Every file involved, changed or reached, so a module or layer
        // is named whether the change is in it or merely reaches it.
        let mut involved: BTreeSet<&str> = placed.iter().map(|c| c.path.as_str()).collect();
        involved.extend(impact.iter().map(|i| i.path.as_str()));

        let layers_touched = rules.as_ref().map_or_else(Vec::new, |r| {
            involved
                .iter()
                .filter_map(|p| r.layer_of(p))
                .map(ToOwned::to_owned)
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect()
        });

        let modules_touched = declared.as_ref().map_or_else(Vec::new, |d| {
            d.modules
                .iter()
                .filter(|m| involved.iter().any(|p| is_within(p, &m.path)))
                .map(|m| m.name.clone())
                .collect()
        });

        Ok(ImpactReport {
            changed: placed,
            impact,
            modules_touched,
            layers_touched,
            depth,
            evidence: ImpactEvidence {
                resolved_imports: imports.len(),
                unresolved_imports: unresolved,
                changed_not_in_graph: not_in_graph,
                changed_since_indexed: stale,
            },
        })
    }
}

/// Breadth-first over reverse imports, nearest first.
///
/// Breadth-first rather than depth-first so that a file's reported
/// distance is its shortest one. Reached two ways, the near answer is
/// the one that describes the risk of touching it.
fn spread<'a>(
    seeds: &[&'a str],
    importers: &HashMap<&'a str, Vec<&'a str>>,
    path_of: &HashMap<&'a str, &'a str>,
    depth: usize,
) -> Vec<Impacted> {
    let mut seen: HashSet<&str> = seeds.iter().copied().collect();
    let mut queue: VecDeque<(&str, usize)> = seeds.iter().map(|id| (*id, 0)).collect();
    let mut out = Vec::new();

    while let Some((id, distance)) = queue.pop_front() {
        if distance == depth {
            continue;
        }
        for importer in importers.get(id).into_iter().flatten() {
            // A changed file reached from another changed file is
            // already in `changed`; reporting it again as impact would
            // double-count the same file.
            if !seen.insert(importer) {
                continue;
            }
            if let Some(path) = path_of.get(importer) {
                out.push(Impacted {
                    path: (*path).to_owned(),
                    distance: distance + 1,
                });
            }
            queue.push_back((importer, distance + 1));
        }
    }

    out.sort_by(|a, b| (a.distance, &a.path).cmp(&(b.distance, &b.path)));
    out
}

/// Whether a path sits inside a module's directory.
///
/// Compared segment-wise so that `features/auth` does not claim
/// `features/authz`.
fn is_within(path: &str, directory: &str) -> bool {
    let directory = directory.trim_end_matches('/');
    path == directory
        || path
            .strip_prefix(directory)
            .is_some_and(|rest| rest.starts_with('/'))
}

/// Whether a file on disk still matches what was indexed.
///
/// Takes an absolute path, joined from the project root by the caller:
/// graph paths are relative to that root, and the server's working
/// directory is somewhere else entirely. Reading the relative path
/// would fail for every file and report the whole project as stale.
///
/// A file that cannot be read is treated as changed — it was deleted
/// or became unreadable since the index, and either way the graph no
/// longer describes it.
fn content_matches(path: &Path, indexed_hash: &str) -> bool {
    std::fs::read(path).is_ok_and(|bytes| blake3::hash(&bytes).to_hex().to_string() == indexed_hash)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_module_directory_does_not_claim_a_similarly_named_sibling() {
        assert!(is_within("features/auth/src/api.rs", "features/auth"));
        assert!(is_within("features/auth", "features/auth"));
        assert!(
            !is_within("features/authz/src/api.rs", "features/auth"),
            "a prefix match claimed a different module"
        );
    }

    #[test]
    fn a_trailing_slash_in_a_declaration_is_tolerated() {
        assert!(is_within("features/auth/src/api.rs", "features/auth/"));
    }

    fn ids<'a>(pairs: &[(&'a str, &'a str)]) -> HashMap<&'a str, Vec<&'a str>> {
        let mut m: HashMap<&str, Vec<&str>> = HashMap::new();
        for (importer, imported) in pairs {
            m.entry(imported).or_default().push(importer);
        }
        m
    }

    #[test]
    fn distance_counts_hops_away_from_the_change() {
        // c imports b imports a. Changing a reaches b at 1, c at 2.
        let importers = ids(&[("b", "a"), ("c", "b")]);
        let path_of: HashMap<&str, &str> = [("a", "a.rs"), ("b", "b.rs"), ("c", "c.rs")].into();

        let out = spread(&["a"], &importers, &path_of, 3);
        assert_eq!(out.len(), 2);
        assert_eq!((out[0].path.as_str(), out[0].distance), ("b.rs", 1));
        assert_eq!((out[1].path.as_str(), out[1].distance), ("c.rs", 2));
    }

    #[test]
    fn depth_stops_the_spread() {
        let importers = ids(&[("b", "a"), ("c", "b"), ("d", "c")]);
        let path_of: HashMap<&str, &str> =
            [("a", "a.rs"), ("b", "b.rs"), ("c", "c.rs"), ("d", "d.rs")].into();

        let out = spread(&["a"], &importers, &path_of, 1);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].path, "b.rs");
    }

    #[test]
    fn a_cycle_terminates_rather_than_spinning() {
        // a and b import each other. Without the seen set this never
        // returns, and an import cycle is a real shape in real code.
        let importers = ids(&[("b", "a"), ("a", "b")]);
        let path_of: HashMap<&str, &str> = [("a", "a.rs"), ("b", "b.rs")].into();

        let out = spread(&["a"], &importers, &path_of, 5);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].path, "b.rs");
    }

    #[test]
    fn a_file_reachable_two_ways_reports_its_shortest_distance() {
        // d imports a directly and also imports c, which imports a.
        let importers = ids(&[("d", "a"), ("c", "a"), ("d", "c")]);
        let path_of: HashMap<&str, &str> = [("a", "a.rs"), ("c", "c.rs"), ("d", "d.rs")].into();

        let out = spread(&["a"], &importers, &path_of, 5);
        let d = out.iter().find(|i| i.path == "d.rs").unwrap();
        assert_eq!(d.distance, 1, "the longer path won");
    }

    #[test]
    fn a_second_changed_file_is_not_also_reported_as_impact() {
        // Both a and b changed, and b imports a. b is already in the
        // changed list; naming it again as impact double-counts it.
        let importers = ids(&[("b", "a")]);
        let path_of: HashMap<&str, &str> = [("a", "a.rs"), ("b", "b.rs")].into();

        let out = spread(&["a", "b"], &importers, &path_of, 5);
        assert!(out.is_empty(), "{out:?}");
    }
}
