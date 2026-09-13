//! Integration tests for declared modules and layer compliance.
//!
//! Every fixture writes a real `atlas.layers.toml` to a real
//! directory and indexes it for real. A layer violation is the only
//! finding reported at error severity, so the thing that most needs
//! proving is not that the checker works on a hand-built graph — it
//! is that the declaration reaches it from disk at all.

use std::path::Path;

use atlas_graph::api::{
    Analyser, Findings, GraphStore, Indexer, Severity, SqlitePool, run_migrations,
};
use tempfile::TempDir;

async fn store() -> GraphStore {
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    run_migrations(&pool).await.unwrap();
    GraphStore::new(pool)
}

fn write(root: &Path, path: &str, content: &str) {
    let full = root.join(path);
    if let Some(parent) = full.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(full, content).unwrap();
}

const DECLARATION: &str = r#"
[[module]]
name = "core"
path = "src"
description = "everything that matters"

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

/// A project whose every file belongs to a declared layer, so the
/// check has full coverage and an empty result means something.
fn compliant_project() -> TempDir {
    let dir = TempDir::new().unwrap();
    write(dir.path(), "atlas.layers.toml", DECLARATION);
    write(dir.path(), "src/domain/model.rs", "pub struct Thing;\n");
    write(
        dir.path(),
        "src/application/service.rs",
        "use crate::domain::model::Thing;\n",
    );
    write(
        dir.path(),
        "src/infrastructure/store.rs",
        "use crate::domain::model::Thing;\n",
    );
    dir
}

async fn analyse(dir: &TempDir) -> Findings {
    let store = store().await;
    Indexer::new(store.clone())
        .reindex("demo", dir.path())
        .await
        .unwrap();
    Analyser::new(store).analyse("demo").await.unwrap()
}

fn finding<'a>(report: &'a Findings, kind: &str) -> Option<&'a atlas_graph::api::Finding> {
    report.findings.iter().find(|f| f.kind == kind)
}

#[tokio::test]
async fn a_project_obeying_its_own_declaration_reports_no_violation() {
    let report = analyse(&compliant_project()).await;
    assert!(
        finding(&report, "layer_violation").is_none(),
        "a compliant project was reported as violating: {:?}",
        report.findings
    );
    // And with full coverage, so the empty result means compliance
    // rather than blindness.
    assert!(
        finding(&report, "layer_check_incomplete").is_none(),
        "coverage was incomplete on a project where every file has a layer"
    );
}

#[tokio::test]
async fn domain_importing_infrastructure_is_an_error_naming_the_file_and_line() {
    let dir = compliant_project();
    write(
        dir.path(),
        "src/domain/model.rs",
        "pub struct Thing;\nuse crate::infrastructure::store;\n",
    );

    let report = analyse(&dir).await;
    let violation = finding(&report, "layer_violation").expect("the violation was not found");

    assert_eq!(violation.severity, Severity::Error);
    assert!(violation.nodes.contains(&"src/domain/model.rs".to_owned()));
    assert!(
        violation.detail.contains("src/domain/model.rs:2"),
        "the finding does not point at the import statement: {}",
        violation.detail
    );
    assert!(
        violation.detail.contains("domain") && violation.detail.contains("infrastructure"),
        "the finding does not name both layers: {}",
        violation.detail
    );
}

#[tokio::test]
async fn the_permitted_direction_is_not_reported() {
    let dir = compliant_project();
    // infrastructure → domain is declared, and must stay silent even
    // though it crosses a layer boundary.
    write(
        dir.path(),
        "src/infrastructure/store.rs",
        "use crate::domain::model::Thing;\n",
    );
    assert!(finding(&analyse(&dir).await, "layer_violation").is_none());
}

#[tokio::test]
async fn a_project_with_no_declaration_gets_no_layer_findings() {
    let dir = TempDir::new().unwrap();
    write(dir.path(), "src/domain/model.rs", "pub struct Thing;\n");
    write(
        dir.path(),
        "src/infrastructure/store.rs",
        "use crate::domain::model::Thing;\n",
    );

    let report = analyse(&dir).await;
    // The directory names look exactly like layers. Atlas must not
    // infer them: a guessed architecture reported at error severity is
    // worse than no finding.
    assert!(finding(&report, "layer_violation").is_none());
    assert!(finding(&report, "layer_check_incomplete").is_none());
    assert!(
        report
            .findings
            .iter()
            .all(|f| f.severity != Severity::Error),
        "an undeclared project produced an error-severity finding"
    );
}

#[tokio::test]
async fn an_unreadable_declaration_is_reported_rather_than_ignored() {
    let dir = compliant_project();
    write(
        dir.path(),
        "atlas.layers.toml",
        "[[layer]]\nname = \"application\"\npaths = [\"a/**\"]\ndepends_on = [\"nope\"]\n",
    );

    let report = analyse(&dir).await;
    let broken =
        finding(&report, "layers_file_unreadable").expect("a broken declaration passed silently");

    assert!(
        broken.detail.contains("nope"),
        "the finding does not name what was wrong: {}",
        broken.detail
    );
    // Reporting no violations here would read as compliance.
    assert!(finding(&report, "layer_violation").is_none());
}

#[tokio::test]
async fn a_malformed_declaration_does_not_fail_indexing() {
    let dir = compliant_project();
    write(dir.path(), "atlas.layers.toml", "[[layer]\nname = ");

    let store = store().await;
    // The rest of the graph is still worth having: a typo in an
    // optional file must not leave a project unsearchable.
    let stats = Indexer::new(store.clone())
        .reindex("demo", dir.path())
        .await
        .expect("indexing failed on a malformed declaration");
    assert!(stats.files_indexed >= 3);

    let report = Analyser::new(store).analyse("demo").await.unwrap();
    assert!(finding(&report, "layers_file_unreadable").is_some());
}

#[tokio::test]
async fn incomplete_coverage_is_stated_so_silence_is_not_read_as_compliance() {
    let dir = compliant_project();
    // Belongs to no declared layer.
    write(dir.path(), "src/main.rs", "fn main() {}\n");

    let report = analyse(&dir).await;
    let incomplete =
        finding(&report, "layer_check_incomplete").expect("an incomplete check did not say so");

    assert_eq!(incomplete.severity, Severity::Info);
    assert!(
        incomplete.detail.contains("not proof of compliance"),
        "{}",
        incomplete.detail
    );
}

#[tokio::test]
async fn a_layer_matching_nothing_is_named() {
    let dir = compliant_project();
    write(
        dir.path(),
        "atlas.layers.toml",
        &format!("{DECLARATION}\n[[layer]]\nname = \"ports\"\npaths = [\"does/not/exist/**\"]\n"),
    );
    write(dir.path(), "src/main.rs", "fn main() {}\n");

    let report = analyse(&dir).await;
    let incomplete = finding(&report, "layer_check_incomplete").expect("no coverage finding");
    // A pattern matching nothing produces no violations, which is
    // indistinguishable from a clean layer unless it is said.
    assert!(
        incomplete.detail.contains("ports"),
        "a layer matching no file was not named: {}",
        incomplete.detail
    );
}

#[tokio::test]
async fn declared_modules_reach_the_overview() {
    let store = store().await;
    let dir = compliant_project();
    Indexer::new(store.clone())
        .reindex("demo", dir.path())
        .await
        .unwrap();

    let overview = store.overview("demo").await.unwrap();
    assert_eq!(overview.modules.len(), 1);
    assert_eq!(overview.modules[0].name, "core");
    assert_eq!(overview.modules[0].path, "src");
}

#[tokio::test]
async fn deleting_the_declaration_stops_the_rules_applying() {
    let store = store().await;
    let dir = compliant_project();
    write(
        dir.path(),
        "src/domain/model.rs",
        "pub struct Thing;\nuse crate::infrastructure::store;\n",
    );

    let indexer = Indexer::new(store.clone());
    indexer.reindex("demo", dir.path()).await.unwrap();
    let before = Analyser::new(store.clone()).analyse("demo").await.unwrap();
    assert!(finding(&before, "layer_violation").is_some());

    std::fs::remove_file(dir.path().join("atlas.layers.toml")).unwrap();
    indexer.reindex("demo", dir.path()).await.unwrap();

    let after = Analyser::new(store).analyse("demo").await.unwrap();
    // A project must not keep being judged against a rule it deleted.
    assert!(
        finding(&after, "layer_violation").is_none(),
        "a deleted declaration was still being enforced"
    );
}
