//! Integration tests for the findings report.
//!
//! Each fixture is a real project on disk, indexed for real. A finding
//! that only holds against a hand-built graph proves the algorithm,
//! not that the extractors produce the edges it needs.

use std::path::Path;

use atlas_graph::api::{Analyser, GraphStore, Indexer, Severity, SqlitePool, run_migrations};
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

async fn analyse(dir: &TempDir) -> atlas_graph::api::Findings {
    let store = store().await;
    Indexer::new(store.clone())
        .reindex("demo", dir.path())
        .await
        .unwrap();
    Analyser::new(store).analyse("demo").await.unwrap()
}

fn is_markdown(path: &String) -> bool {
    Path::new(path).extension().is_some_and(|e| e == "md")
}

fn of_kind<'a>(
    findings: &'a atlas_graph::api::Findings,
    kind: &str,
) -> Option<&'a atlas_graph::api::Finding> {
    findings.findings.iter().find(|f| f.kind == kind)
}

#[tokio::test]
async fn a_two_file_import_cycle_is_found() {
    let dir = TempDir::new().unwrap();
    write(dir.path(), "src/a.ts", "import { b } from './b';\n");
    write(dir.path(), "src/b.ts", "import { a } from './a';\n");

    let report = analyse(&dir).await;
    let cycle = of_kind(&report, "import_cycle").expect("the cycle was not found");

    assert_eq!(cycle.severity, Severity::Warning);
    assert_eq!(cycle.nodes.len(), 2);
    assert!(cycle.nodes.contains(&"src/a.ts".to_owned()));
    assert!(cycle.nodes.contains(&"src/b.ts".to_owned()));
}

#[tokio::test]
async fn a_longer_cycle_is_found_as_one_finding() {
    let dir = TempDir::new().unwrap();
    write(dir.path(), "src/a.ts", "import { b } from './b';\n");
    write(dir.path(), "src/b.ts", "import { c } from './c';\n");
    write(dir.path(), "src/c.ts", "import { a } from './a';\n");

    let report = analyse(&dir).await;
    let cycles: Vec<_> = report
        .findings
        .iter()
        .filter(|f| f.kind == "import_cycle")
        .collect();

    assert_eq!(cycles.len(), 1, "a three-file cycle should be one finding");
    assert_eq!(cycles[0].nodes.len(), 3);
}

#[tokio::test]
async fn a_clean_dependency_chain_reports_no_cycle() {
    let dir = TempDir::new().unwrap();
    write(dir.path(), "src/a.ts", "import { b } from './b';\n");
    write(dir.path(), "src/b.ts", "import { c } from './c';\n");
    write(dir.path(), "src/c.ts", "export const c = 1;\n");

    let report = analyse(&dir).await;
    assert!(
        of_kind(&report, "import_cycle").is_none(),
        "a straight chain was reported as a cycle"
    );
}

#[tokio::test]
async fn two_separate_cycles_are_two_findings() {
    let dir = TempDir::new().unwrap();
    write(dir.path(), "one/a.ts", "import { b } from './b';\n");
    write(dir.path(), "one/b.ts", "import { a } from './a';\n");
    write(dir.path(), "two/x.ts", "import { y } from './y';\n");
    write(dir.path(), "two/y.ts", "import { x } from './x';\n");

    let report = analyse(&dir).await;
    let cycles: Vec<_> = report
        .findings
        .iter()
        .filter(|f| f.kind == "import_cycle")
        .collect();
    assert_eq!(cycles.len(), 2);
}

#[tokio::test]
async fn mentions_never_produce_a_cycle() {
    let dir = TempDir::new().unwrap();
    // These two name each other in prose. `mentions` will connect them
    // both ways; findings must not treat that as a dependency cycle,
    // because it is a guess and a cycle is a warning.
    write(
        dir.path(),
        "alpha.ts",
        "// see bravo for the other half\nexport const a = 1;\n",
    );
    write(
        dir.path(),
        "bravo.ts",
        "// see alpha for the other half\nexport const b = 1;\n",
    );

    let report = analyse(&dir).await;
    assert!(
        of_kind(&report, "import_cycle").is_none(),
        "a mentions loop was reported as an import cycle"
    );
    assert_eq!(
        report.evidence.resolved_imports, 0,
        "there are no imports here at all"
    );
}

#[tokio::test]
async fn the_report_publishes_what_it_was_computed_from() {
    let dir = TempDir::new().unwrap();
    write(
        dir.path(),
        "src/a.ts",
        "import { b } from './b';\nimport React from 'react';\n",
    );
    write(dir.path(), "src/b.ts", "export const b = 1;\n");

    let report = analyse(&dir).await;
    assert_eq!(report.evidence.files, 2);
    assert_eq!(
        report.evidence.resolved_imports, 1,
        "only ./b resolves; react is a dependency"
    );
}

#[tokio::test]
async fn an_unreferenced_source_file_is_reported_but_entry_points_are_not() {
    let dir = TempDir::new().unwrap();
    write(
        dir.path(),
        "src/main.ts",
        "import { used } from './used';\n",
    );
    write(dir.path(), "src/used.ts", "export const used = 1;\n");
    write(dir.path(), "src/forgotten.ts", "export const nobody = 1;\n");
    write(dir.path(), "README.md", "# Docs\n");

    let report = analyse(&dir).await;
    let orphans = of_kind(&report, "unreferenced_file").expect("nothing was reported");

    assert!(orphans.nodes.contains(&"src/forgotten.ts".to_owned()));
    assert!(
        !orphans.nodes.iter().any(|n| n.contains("main")),
        "an entry point was reported as dead code"
    );
    assert!(
        !orphans.nodes.iter().any(is_markdown),
        "a markdown file was reported as an unreferenced source file"
    );
}

#[tokio::test]
async fn a_long_file_is_reported_and_prose_is_not() {
    let dir = TempDir::new().unwrap();
    let long = "const x = 1;\n".repeat(700);
    write(dir.path(), "src/huge.ts", &long);
    write(dir.path(), "docs/huge.md", &"a line of prose\n".repeat(700));
    write(dir.path(), "src/small.ts", "const y = 2;\n");

    let report = analyse(&dir).await;
    let large = of_kind(&report, "large_file").expect("the long file was not reported");

    assert!(large.nodes.contains(&"src/huge.ts".to_owned()));
    assert!(
        !large.nodes.iter().any(is_markdown),
        "a long document is not a large source file"
    );
    assert!(!large.nodes.contains(&"src/small.ts".to_owned()));
}

#[tokio::test]
async fn a_directory_without_a_readme_is_reported() {
    let dir = TempDir::new().unwrap();
    write(dir.path(), "documented/README.md", "# Here\n");
    write(dir.path(), "documented/code.ts", "export const a = 1;\n");
    write(dir.path(), "bare/code.ts", "export const b = 1;\n");

    let report = analyse(&dir).await;
    let undocumented = of_kind(&report, "undocumented_directory").expect("nothing was reported");

    assert!(undocumented.nodes.contains(&"bare".to_owned()));
    assert!(
        !undocumented.nodes.contains(&"documented".to_owned()),
        "a directory with a README was reported as undocumented"
    );
}

#[tokio::test]
async fn warnings_come_before_information() {
    let dir = TempDir::new().unwrap();
    write(dir.path(), "src/a.ts", "import { b } from './b';\n");
    write(dir.path(), "src/b.ts", "import { a } from './a';\n");
    write(dir.path(), "src/lonely.ts", "export const c = 1;\n");

    let report = analyse(&dir).await;
    assert!(report.findings.len() >= 2);
    assert_eq!(
        report.findings[0].severity,
        Severity::Warning,
        "a report read top-down should lead with what is most likely wrong"
    );
}

#[tokio::test]
async fn a_clean_project_reports_nothing_alarming() {
    let dir = TempDir::new().unwrap();
    write(dir.path(), "README.md", "# Clean\n");
    write(
        dir.path(),
        "src/main.ts",
        "import { helper } from './helper';\n",
    );
    write(dir.path(), "src/helper.ts", "export const helper = 1;\n");
    write(dir.path(), "src/README.md", "# Source\n");

    let report = analyse(&dir).await;
    assert!(
        report.findings.iter().all(|f| f.severity == Severity::Info),
        "a clean project produced a warning: {:?}",
        report.findings
    );
}
