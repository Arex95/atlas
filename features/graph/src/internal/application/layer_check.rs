//! Checking a project against the architecture it declared.
//!
//! This is the only finding reported at error severity, and it is the
//! only one that can be: every other finding here is a heuristic
//! observation, while a layer violation is measured against a rule the
//! project wrote down itself. Nothing is inferred — a project with no
//! `atlas.layers.toml` gets no layer findings at all, because a
//! guessed architecture reported as a violation is worse than silence.
//!
//! Two ways this could lie, and what is done about each:
//!
//! * **A violation through an import the graph could not resolve.**
//!   Unresolved imports are invisible here, so an absence of
//!   violations is not proof of compliance. The report therefore
//!   carries its own coverage — files classified, files not, imports
//!   resolved and not — and the finding's text says so when coverage
//!   is incomplete. A silent "0 violations" reads as a fact; this one
//!   has to show its work.
//! * **A file matching no layer.** It is left unclassified and never
//!   reported. Reporting it would mean guessing which layer its author
//!   meant, and that guess would be an error-severity finding.

use std::collections::{BTreeMap, HashMap, HashSet};

use globset::{Glob, GlobSetBuilder};
use serde::Serialize;

use crate::internal::domain::{LayersError, LayersFile};

/// How much of the project the layer check could actually see.
///
/// Published with every layer report. A reader deciding whether "no
/// violations" means "compliant" cannot tell without it.
#[derive(Clone, Copy, Debug, Serialize)]
pub struct Coverage {
    pub files_classified: usize,
    pub files_unclassified: usize,
    /// Import edges between two classified files — the only ones a
    /// violation can be found in.
    pub imports_checked: usize,
    /// Import edges that resolved to a file no layer claims, and so
    /// could not be checked either way.
    pub imports_unclassified: usize,
}

impl Coverage {
    /// Whether the check saw everything it would need to for an empty
    /// result to mean compliance.
    #[must_use]
    pub const fn is_complete(&self) -> bool {
        self.files_unclassified == 0 && self.imports_unclassified == 0
    }
}

/// One import that crosses a boundary the project declared closed.
#[derive(Clone, Debug, Serialize)]
pub struct Violation {
    pub from_file: String,
    pub from_layer: String,
    pub to_file: String,
    pub to_layer: String,
    pub line: Option<i64>,
}

/// The outcome of checking one project.
#[derive(Clone, Debug, Serialize)]
pub struct LayerReport {
    pub violations: Vec<Violation>,
    pub coverage: Coverage,
    /// Files per layer, so a reader can see a layer matching nothing —
    /// which is a broken pattern wearing the face of a clean layer.
    pub files_per_layer: BTreeMap<String, usize>,
}

/// A compiled declaration, ready to classify files.
pub struct LayerRules {
    matchers: Vec<(String, globset::GlobSet)>,
    /// layer → the layers it may import from.
    allowed: HashMap<String, HashSet<String>>,
}

impl LayerRules {
    /// Compiles the glob patterns in a validated declaration.
    ///
    /// # Errors
    /// `InvalidPattern` naming the layer and the pattern that would
    /// not compile. Refused rather than skipped: a dropped pattern
    /// means files silently leave their layer, and the check then
    /// reports compliance it did not verify.
    pub fn compile(file: &LayersFile) -> Result<Self, LayersError> {
        let mut matchers = Vec::with_capacity(file.layers.len());
        let mut allowed = HashMap::with_capacity(file.layers.len());

        for layer in &file.layers {
            let mut builder = GlobSetBuilder::new();
            for pattern in &layer.paths {
                let glob = Glob::new(pattern).map_err(|e| LayersError::InvalidPattern {
                    layer: layer.name.clone(),
                    pattern: pattern.clone(),
                    reason: e.to_string(),
                })?;
                builder.add(glob);
            }
            let set = builder.build().map_err(|e| LayersError::InvalidPattern {
                layer: layer.name.clone(),
                pattern: layer.paths.join(", "),
                reason: e.to_string(),
            })?;
            matchers.push((layer.name.clone(), set));
            allowed.insert(
                layer.name.clone(),
                layer.depends_on.iter().cloned().collect(),
            );
        }

        Ok(Self { matchers, allowed })
    }

    /// The layer a file belongs to, or `None` if none claims it.
    ///
    /// Declaration order decides an overlap. Two layers matching the
    /// same file is a contradiction the project has to resolve, but
    /// resolving it here by refusing would take down the whole check
    /// over one ambiguous path — so the first declaration wins, which
    /// is at least stable and documented.
    #[must_use]
    pub fn layer_of(&self, path: &str) -> Option<&str> {
        self.matchers
            .iter()
            .find(|(_, set)| set.is_match(path))
            .map(|(name, _)| name.as_str())
    }

    /// Whether `from` is permitted to import from `to`.
    #[must_use]
    pub fn permits(&self, from: &str, to: &str) -> bool {
        // A layer always depends on itself; declaring that would be
        // noise in every file.
        from == to
            || self
                .allowed
                .get(from)
                .is_some_and(|allowed| allowed.contains(to))
    }
}

/// Checks resolved imports against a compiled declaration.
///
/// `files` is (`file_id`, `path`); `imports` is (`src_id`, `dst_id`, line).
#[must_use]
pub fn check(
    rules: &LayerRules,
    files: &[(String, String)],
    imports: &[(String, String, Option<i64>)],
) -> LayerReport {
    let mut layer_of_id: HashMap<&str, &str> = HashMap::new();
    let mut path_of_id: HashMap<&str, &str> = HashMap::new();
    let mut files_per_layer: BTreeMap<String, usize> = BTreeMap::new();
    // Every declared layer appears, including one that matched nothing
    // — a layer with no files is a broken pattern, and a report that
    // omits it looks clean.
    for (name, _) in &rules.matchers {
        files_per_layer.insert(name.clone(), 0);
    }

    let mut classified = 0;
    for (id, path) in files {
        path_of_id.insert(id.as_str(), path.as_str());
        if let Some(layer) = rules.layer_of(path) {
            layer_of_id.insert(id.as_str(), layer);
            *files_per_layer.entry(layer.to_owned()).or_default() += 1;
            classified += 1;
        }
    }

    let mut violations = Vec::new();
    let mut checked = 0;
    let mut unclassified_imports = 0;

    for (src, dst, line) in imports {
        let (Some(from), Some(to)) = (
            layer_of_id.get(src.as_str()).copied(),
            layer_of_id.get(dst.as_str()).copied(),
        ) else {
            unclassified_imports += 1;
            continue;
        };
        checked += 1;
        if rules.permits(from, to) {
            continue;
        }
        violations.push(Violation {
            from_file: (*path_of_id.get(src.as_str()).unwrap_or(&"")).to_owned(),
            from_layer: from.to_owned(),
            to_file: (*path_of_id.get(dst.as_str()).unwrap_or(&"")).to_owned(),
            to_layer: to.to_owned(),
            line: *line,
        });
    }

    // Stable output: the same project twice produces the same report,
    // so a diff between two runs is a change in the code.
    violations.sort_by(|a, b| {
        (&a.from_file, a.line, &a.to_file).cmp(&(&b.from_file, b.line, &b.to_file))
    });

    LayerReport {
        violations,
        coverage: Coverage {
            files_classified: classified,
            files_unclassified: files.len() - classified,
            imports_checked: checked,
            imports_unclassified: unclassified_imports,
        },
        files_per_layer,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rules(toml: &str) -> LayerRules {
        LayerRules::compile(&LayersFile::parse(toml).unwrap()).unwrap()
    }

    const THREE_LAYERS: &str = r#"
        [[layer]]
        name = "domain"
        paths = ["**/domain/**"]

        [[layer]]
        name = "application"
        paths = ["**/application/**"]
        depends_on = ["domain"]

        [[layer]]
        name = "infrastructure"
        paths = ["**/infrastructure/**"]
        depends_on = ["domain", "application"]
    "#;

    #[test]
    fn a_layer_may_always_import_from_itself() {
        let rules = rules(THREE_LAYERS);
        assert!(rules.permits("domain", "domain"));
    }

    #[test]
    fn a_declared_dependency_is_permitted_and_its_reverse_is_not() {
        let rules = rules(THREE_LAYERS);
        assert!(rules.permits("application", "domain"));
        assert!(
            !rules.permits("domain", "application"),
            "the dependency was permitted in both directions"
        );
    }

    #[test]
    fn files_are_classified_by_their_path() {
        let rules = rules(THREE_LAYERS);
        assert_eq!(
            rules.layer_of("src/internal/domain/model.rs"),
            Some("domain")
        );
        assert_eq!(
            rules.layer_of("src/internal/infrastructure/store.rs"),
            Some("infrastructure")
        );
        assert_eq!(rules.layer_of("src/main.rs"), None);
    }

    #[test]
    fn a_domain_file_importing_infrastructure_is_a_violation() {
        let rules = rules(THREE_LAYERS);
        let files = vec![
            ("d".to_owned(), "src/domain/model.rs".to_owned()),
            ("i".to_owned(), "src/infrastructure/store.rs".to_owned()),
        ];
        let imports = vec![("d".to_owned(), "i".to_owned(), Some(3))];

        let report = check(&rules, &files, &imports);
        assert_eq!(report.violations.len(), 1);
        assert_eq!(report.violations[0].from_layer, "domain");
        assert_eq!(report.violations[0].to_layer, "infrastructure");
        assert_eq!(report.violations[0].line, Some(3));
        assert!(report.coverage.is_complete());
    }

    #[test]
    fn the_permitted_direction_is_not_a_violation() {
        let rules = rules(THREE_LAYERS);
        let files = vec![
            ("d".to_owned(), "src/domain/model.rs".to_owned()),
            ("a".to_owned(), "src/application/service.rs".to_owned()),
        ];
        let imports = vec![("a".to_owned(), "d".to_owned(), Some(1))];

        assert!(check(&rules, &files, &imports).violations.is_empty());
    }

    #[test]
    fn an_unclassified_file_is_never_reported() {
        let rules = rules(THREE_LAYERS);
        let files = vec![
            ("m".to_owned(), "src/main.rs".to_owned()),
            ("d".to_owned(), "src/domain/model.rs".to_owned()),
        ];
        // main.rs belongs to no layer. Deciding which one its author
        // meant would be a guess reported at error severity.
        let imports = vec![("m".to_owned(), "d".to_owned(), None)];

        let report = check(&rules, &files, &imports);
        assert!(report.violations.is_empty());
        assert_eq!(report.coverage.files_unclassified, 1);
        assert_eq!(report.coverage.imports_unclassified, 1);
        assert_eq!(report.coverage.imports_checked, 0);
        assert!(
            !report.coverage.is_complete(),
            "an incomplete check reported itself as complete"
        );
    }

    #[test]
    fn a_layer_matching_no_file_still_appears_in_the_report() {
        // A pattern that matches nothing produces no violations, which
        // is indistinguishable from a clean layer unless it is shown.
        let rules = rules(
            r#"
            [[layer]]
            name = "domain"
            paths = ["**/domain/**"]

            [[layer]]
            name = "nothing"
            paths = ["does/not/exist/**"]
            "#,
        );
        let files = vec![("d".to_owned(), "src/domain/model.rs".to_owned())];

        let report = check(&rules, &files, &[]);
        assert_eq!(report.files_per_layer.get("nothing"), Some(&0));
        assert_eq!(report.files_per_layer.get("domain"), Some(&1));
    }

    #[test]
    fn an_invalid_pattern_is_refused_rather_than_skipped() {
        let file = LayersFile::parse(
            r#"
            [[layer]]
            name = "domain"
            paths = ["**/domain/{"]
            "#,
        )
        .unwrap();
        // Skipping it would let domain files silently leave the layer,
        // and the check would then report a compliance it never
        // verified.
        assert!(matches!(
            LayerRules::compile(&file),
            Err(LayersError::InvalidPattern { .. })
        ));
    }

    #[test]
    fn an_overlap_resolves_to_the_first_declaration() {
        let rules = rules(
            r#"
            [[layer]]
            name = "first"
            paths = ["src/**"]

            [[layer]]
            name = "second"
            paths = ["src/**"]
            "#,
        );
        assert_eq!(rules.layer_of("src/a.rs"), Some("first"));
    }

    #[test]
    fn the_same_input_twice_produces_the_same_report() {
        let rules = rules(THREE_LAYERS);
        let files = vec![
            ("d".to_owned(), "src/domain/a.rs".to_owned()),
            ("e".to_owned(), "src/domain/b.rs".to_owned()),
            ("i".to_owned(), "src/infrastructure/s.rs".to_owned()),
        ];
        let imports = vec![
            ("e".to_owned(), "i".to_owned(), Some(2)),
            ("d".to_owned(), "i".to_owned(), Some(9)),
        ];

        let first = check(&rules, &files, &imports);
        let second = check(&rules, &files, &imports);
        let paths = |r: &LayerReport| {
            r.violations
                .iter()
                .map(|v| (v.from_file.clone(), v.line))
                .collect::<Vec<_>>()
        };
        assert_eq!(paths(&first), paths(&second));
        assert_eq!(paths(&first)[0].0, "src/domain/a.rs");
    }
}
