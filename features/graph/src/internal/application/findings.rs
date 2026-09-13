//! What is worth looking at in a project, and nothing else.
//!
//! Deliberately **not** an architecture score. A composite 0–100
//! invites making the number go up, and it would be computed here
//! from a graph that cannot support it: on this repository the real
//! dependency graph is 146 resolved import edges across 220 files,
//! while `mentions` — the one heuristic predicate — contributes 971.
//! A "cohesion: 68" derived mostly from files sharing a word is not
//! measuring architecture, it is laundering a guess into a number.
//!
//! So: findings, each one a specific thing at a specific place that a
//! person can go and look at, and each one computed from evidence
//! strong enough to act on.
//!
//! **Only resolved `imports` edges count as dependencies.** Not
//! `mentions`, which is heuristic; not `links`, which is documentation
//! pointing at documentation; not `contains`, which is structure. An
//! import that resolved to a real node in this project is the only
//! edge here that means "this file needs that file".
//!
//! Every report carries the size of the evidence it was drawn from,
//! because a reader deciding how much to trust a finding needs to know
//! whether it came from a hundred edges or ten thousand.

use std::collections::{HashMap, HashSet};

use serde::Serialize;

use crate::internal::application::layer_check::{LayerReport, LayerRules, check as check_layers};
use crate::internal::domain::FileFacts;
use crate::internal::domain::strongly_connected;
use crate::internal::domain::{GraphError, LAYERS_FILE, LayersFile};
use crate::internal::infrastructure::{GraphStore, is_code_path};

/// A file past this many lines is worth a second look. Not a rule —
/// a long file is sometimes the right answer — which is why this is
/// reported and never scored.
const LARGE_FILE_LINES: i64 = 600;

/// A file everything depends on. Not a defect either: a widely-used
/// type *should* have high fan-in. Worth knowing before changing it.
const HUB_FAN_IN: i64 = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    /// A rule the project wrote down for itself, broken. The only
    /// severity here that is measured rather than guessed — which is
    /// why nothing heuristic is ever allowed to use it.
    Error,
    /// Something that is very likely wrong.
    Warning,
    /// Something worth knowing before acting, not worth fixing on its
    /// own.
    Info,
}

#[derive(Clone, Debug, Serialize)]
pub struct Finding {
    pub kind: &'static str,
    pub severity: Severity,
    /// What was found, in one sentence, naming the places involved.
    pub detail: String,
    /// The nodes this is about, so a caller can go straight to them.
    pub nodes: Vec<String>,
}

/// A report, plus what it was drawn from.
#[derive(Clone, Debug, Serialize)]
pub struct Findings {
    pub findings: Vec<Finding>,
    pub evidence: Evidence,
}

/// The size of the sample. Published with every report because a
/// finding drawn from a sparse graph deserves less confidence than
/// the same finding drawn from a dense one, and the reader cannot
/// tell without this.
#[derive(Clone, Copy, Debug, Serialize)]
pub struct Evidence {
    pub files: usize,
    /// Import edges that resolved to a real node in this project —
    /// the only edges any finding here is computed from.
    pub resolved_imports: usize,
    /// Import edges naming something outside the project, or nothing
    /// at all. Counted so the ratio is visible rather than implied.
    pub unresolved_imports: usize,
}

/// Names that are entry points rather than dead code. A file nothing
/// imports is only interesting if something *should* have.
const ENTRY_POINT_STEMS: &[&str] = &["main", "lib", "mod", "index", "__init__", "build", "setup"];

pub struct Analyser {
    store: GraphStore,
}

impl Analyser {
    #[must_use]
    pub fn new(store: GraphStore) -> Self {
        Self { store }
    }

    /// Everything worth looking at in one project.
    ///
    /// # Errors
    /// `Storage` on any SQL failure.
    pub async fn analyse(&self, project: &str) -> Result<Findings, GraphError> {
        let files = self.store.file_nodes(project).await?;
        let with_lines = self.store.resolved_imports(project).await?;
        let unresolved = self.store.unresolved_import_count(project).await?;
        let (layers_source, layers_error) = self.store.project_layers(project).await?;

        let imports: Vec<(String, String)> = with_lines
            .iter()
            .map(|(src, dst, _)| (src.clone(), dst.clone()))
            .collect();

        let evidence = Evidence {
            files: files.len(),
            resolved_imports: imports.len(),
            unresolved_imports: unresolved,
        };

        let mut findings = Vec::new();
        findings.extend(layer_findings(
            &files,
            &with_lines,
            layers_source.as_deref(),
            layers_error.as_deref(),
        ));
        findings.extend(import_cycles(&files, &imports));
        findings.extend(orphans(&files, &imports));
        findings.extend(hubs(&files, &imports));
        findings.extend(large_files(&files));
        findings.extend(undocumented_directories(&files));

        // Most severe first: a report read top-down should lead with
        // what is most likely wrong.
        findings.sort_by_key(|f| match f.severity {
            Severity::Error => 0,
            Severity::Warning => 1,
            Severity::Info => 2,
        });

        Ok(Findings { findings, evidence })
    }
}

/// Findings from the project's own declared architecture.
///
/// Nothing here is inferred. A project with no `atlas.layers.toml`
/// produces nothing at all, because Atlas does not know what its
/// layers are and reporting a guess at error severity would be worse
/// than staying quiet.
fn layer_findings(
    files: &[FileFacts],
    imports: &[(String, String, Option<i64>)],
    source: Option<&str>,
    parse_error: Option<&str>,
) -> Vec<Finding> {
    // A declaration that failed to parse is itself the finding. It is
    // a warning rather than an error because nothing was violated —
    // the rules simply could not be read, and reporting silence as
    // compliance is the failure being avoided.
    if let Some(reason) = parse_error {
        return vec![Finding {
            kind: "layers_file_unreadable",
            severity: Severity::Warning,
            detail: format!(
                "{LAYERS_FILE} could not be read, so no layer rule was checked: {reason}"
            ),
            nodes: vec![LAYERS_FILE.to_owned()],
        }];
    }

    let Some(source) = source else {
        return Vec::new();
    };
    let Ok(declared) = LayersFile::parse(source) else {
        // Stored without a parse error but failing now: the two came
        // from different versions of this code. Say so rather than
        // reporting no violations.
        return vec![Finding {
            kind: "layers_file_unreadable",
            severity: Severity::Warning,
            detail: format!("{LAYERS_FILE} no longer parses; reindex the project"),
            nodes: vec![LAYERS_FILE.to_owned()],
        }];
    };
    if declared.layers.is_empty() {
        return Vec::new();
    }

    let rules = match LayerRules::compile(&declared) {
        Ok(rules) => rules,
        Err(e) => {
            return vec![Finding {
                kind: "layers_file_unreadable",
                severity: Severity::Warning,
                detail: format!("{LAYERS_FILE} declares a pattern that cannot be used: {e}"),
                nodes: vec![LAYERS_FILE.to_owned()],
            }];
        }
    };

    // Only code. A layer declaration is a statement about code, so a
    // README or a lockfile belonging to no layer is not a gap in
    // coverage — and counting it as one would make the incompleteness
    // notice fire on every project that has a readme, which is all of
    // them. `atlas.layers.toml` itself is the clearest case: it would
    // otherwise be permanently unclassified in the very report it
    // configures.
    let paths: Vec<(String, String)> = files
        .iter()
        .filter(|f| is_code_path(&f.fqn))
        .map(|f| (f.id.clone(), f.fqn.clone()))
        .collect();
    let report = check_layers(&rules, &paths, imports);

    let mut findings = Vec::new();

    if !report.violations.is_empty() {
        let mut nodes: Vec<String> = report
            .violations
            .iter()
            .map(|v| v.from_file.clone())
            .collect();
        nodes.dedup();
        let worst = report
            .violations
            .iter()
            .take(3)
            .map(|v| {
                let at = v.line.map_or_else(String::new, |l| format!(":{l}"));
                format!(
                    "{}{at} ({}) imports {} ({})",
                    v.from_file, v.from_layer, v.to_file, v.to_layer
                )
            })
            .collect::<Vec<_>>()
            .join("; ");
        findings.push(Finding {
            kind: "layer_violation",
            severity: Severity::Error,
            detail: format!(
                "{} imports cross a boundary {LAYERS_FILE} declares closed: {worst}{}. \
                 This is measured against the project's own declaration, not inferred.",
                report.violations.len(),
                if report.violations.len() > 3 {
                    ", …"
                } else {
                    ""
                }
            ),
            nodes,
        });
    }

    findings.extend(coverage_notice(&report));
    findings
}

/// Says how much of the project the layer check could see.
///
/// Emitted whether or not a violation was found, and it matters most
/// when none was: an empty result from a check that saw half the
/// project is not a clean bill of health, and nothing else in the
/// output distinguishes the two.
fn coverage_notice(report: &LayerReport) -> Option<Finding> {
    if report.coverage.is_complete() {
        return None;
    }

    let empty: Vec<&str> = report
        .files_per_layer
        .iter()
        .filter(|(_, n)| **n == 0)
        .map(|(name, _)| name.as_str())
        .collect();
    // A layer matching nothing produces no violations, which is
    // indistinguishable from a clean layer unless it is named.
    let no_files = if empty.is_empty() {
        String::new()
    } else {
        format!(" Layers matching no file at all: {}.", empty.join(", "))
    };

    Some(Finding {
        kind: "layer_check_incomplete",
        severity: Severity::Info,
        detail: format!(
            "The layer check saw {} of {} code files and {} imports; {} files belong to no \
             declared layer and {} imports point at one of them, so they were not checked in \
             either direction. An absence of violations is not proof of compliance.{no_files}",
            report.coverage.files_classified,
            report.coverage.files_classified + report.coverage.files_unclassified,
            report.coverage.imports_checked,
            report.coverage.files_unclassified,
            report.coverage.imports_unclassified,
        ),
        nodes: Vec::new(),
    })
}

/// Import cycles: A needs B needs A, directly or through a chain.
///
/// The strongest finding here, and the only one that is nearly always
/// a real problem — a cycle means neither file can be understood,
/// tested or moved without the other.
///
/// Found with Tarjan's strongly-connected components, iteratively
/// rather than recursively: the recursion depth is the length of a
/// dependency chain, and a large monorepo can exceed a stack.
fn import_cycles(files: &[FileFacts], imports: &[(String, String)]) -> Vec<Finding> {
    let index_of: HashMap<&str, usize> = files
        .iter()
        .enumerate()
        .map(|(i, f)| (f.id.as_str(), i))
        .collect();

    let mut adjacency: Vec<Vec<usize>> = vec![Vec::new(); files.len()];
    for (src, dst) in imports {
        // A file importing itself is not a cycle worth reporting, and
        // an edge to a file outside this set has nothing to point at.
        if let (Some(&a), Some(&b)) = (index_of.get(src.as_str()), index_of.get(dst.as_str()))
            && a != b
        {
            adjacency[a].push(b);
        }
    }

    strongly_connected(&adjacency)
        .into_iter()
        .filter(|component| component.len() > 1)
        .map(|component| {
            let mut members: Vec<String> =
                component.iter().map(|i| files[*i].fqn.clone()).collect();
            members.sort();
            Finding {
                kind: "import_cycle",
                severity: Severity::Warning,
                detail: format!(
                    "{} files import each other in a cycle: {}. None can be understood, \
                     tested or moved without the others.",
                    members.len(),
                    members.join(" → ")
                ),
                nodes: members,
            }
        })
        .collect()
}

/// Files nothing imports and which import nothing.
///
/// Possibly dead, possibly an entry point — so entry-point names are
/// excluded, and the finding says "possibly" because this cannot tell
/// the difference on its own.
fn orphans(files: &[FileFacts], imports: &[(String, String)]) -> Vec<Finding> {
    let mut touched: HashSet<&str> = HashSet::new();
    for (src, dst) in imports {
        touched.insert(src.as_str());
        touched.insert(dst.as_str());
    }

    let orphaned: Vec<String> = files
        .iter()
        .filter(|f| !touched.contains(f.id.as_str()))
        .filter(|f| !is_entry_point(&f.fqn))
        // Only files whose imports were actually looked for. A
        // `.gitignore` imports nothing and nothing imports it; saying
        // so is noise, not a finding.
        .filter(|f| is_code_path(&f.fqn))
        .map(|f| f.fqn.clone())
        .collect();

    if orphaned.is_empty() {
        return Vec::new();
    }

    vec![Finding {
        kind: "unreferenced_file",
        severity: Severity::Info,
        detail: format!(
            "{} source files neither import anything in this project nor are imported by it. \
             Possibly dead, possibly reached another way — worth checking, not worth assuming.",
            orphaned.len()
        ),
        nodes: orphaned,
    }]
}

/// The files most of the project depends on.
///
/// Not a defect — a widely-used type *should* be widely used. Reported
/// because changing one of these is a different act from changing a
/// leaf, and it is useful to know which you are about to do.
fn hubs(files: &[FileFacts], imports: &[(String, String)]) -> Vec<Finding> {
    let mut fan_in: HashMap<&str, i64> = HashMap::new();
    for (_, dst) in imports {
        *fan_in.entry(dst.as_str()).or_insert(0) += 1;
    }

    let mut found: Vec<(String, i64)> = files
        .iter()
        .filter_map(|f| {
            fan_in
                .get(f.id.as_str())
                .filter(|count| **count >= HUB_FAN_IN)
                .map(|count| (f.fqn.clone(), *count))
        })
        .collect();

    if found.is_empty() {
        return Vec::new();
    }
    found.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

    vec![Finding {
        kind: "widely_depended_on",
        severity: Severity::Info,
        detail: format!(
            "{} files are imported by {HUB_FAN_IN} or more others. Changing one of these \
             reaches further than changing a leaf.",
            found.len()
        ),
        nodes: found.into_iter().map(|(fqn, _)| fqn).collect(),
    }]
}

/// Long files. Objective, needs no edges, and the one finding here
/// that is entirely independent of how good the import graph is.
fn large_files(files: &[FileFacts]) -> Vec<Finding> {
    let mut large: Vec<(String, i64)> = files
        .iter()
        .filter(|f| f.lines >= LARGE_FILE_LINES)
        .filter(|f| is_code_path(&f.fqn))
        .map(|f| (f.fqn.clone(), f.lines))
        .collect();

    if large.is_empty() {
        return Vec::new();
    }
    large.sort_by(|a, b| b.1.cmp(&a.1));

    vec![Finding {
        kind: "large_file",
        severity: Severity::Info,
        detail: format!(
            "{} source files are over {LARGE_FILE_LINES} lines. Sometimes the right answer, \
             often a module that grew into two.",
            large.len()
        ),
        nodes: large.into_iter().map(|(fqn, _)| fqn).collect(),
    }]
}

/// Directories holding source but no README.
///
/// The weakest finding here, and included because it is the cheapest
/// thing a person can fix that helps the next reader most.
fn undocumented_directories(files: &[FileFacts]) -> Vec<Finding> {
    let mut with_source: HashSet<&str> = HashSet::new();
    let mut with_readme: HashSet<&str> = HashSet::new();

    for file in files {
        let Some((dir, name)) = file.fqn.rsplit_once('/') else {
            continue;
        };
        if name.to_lowercase().starts_with("readme") {
            with_readme.insert(dir);
        } else if is_code_path(&file.fqn) {
            with_source.insert(dir);
        }
    }

    let mut missing: Vec<String> = with_source
        .difference(&with_readme)
        .map(|d| (*d).to_owned())
        .collect();

    if missing.is_empty() {
        return Vec::new();
    }
    missing.sort();

    vec![Finding {
        kind: "undocumented_directory",
        severity: Severity::Info,
        detail: format!(
            "{} directories hold source files and no README. The cheapest thing to fix that \
             helps the next reader most.",
            missing.len()
        ),
        nodes: missing,
    }]
}

fn is_entry_point(fqn: &str) -> bool {
    fqn.rsplit('/')
        .next()
        .and_then(|name| name.split('.').next())
        .is_some_and(|stem| ENTRY_POINT_STEMS.contains(&stem))
}
