//! Integration tests for change impact.
//!
//! Every fixture is a real git repository with real commits. Parsing
//! `git status` output from a string proves the parser; only running
//! `git` proves the two agree — and the whole value of shelling out is
//! that the answer matches what the developer sees in their terminal.

use std::path::Path;
use std::process::Command;

use atlas_graph::api::{
    GraphStore, ImpactAnalyser, ImpactReport, Indexer, SqlitePool, changed_files, run_migrations,
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

fn git(root: &Path, args: &[&str]) {
    let out = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .expect("git is not available; these tests need it");
    assert!(
        out.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// A committed chain: `api` ← `service` ← `store`, so changing the
/// deepest file has somewhere to travel.
fn repo() -> TempDir {
    let dir = TempDir::new().unwrap();
    let root = dir.path();

    write(root, "src/store.rs", "pub struct Store;\n");
    write(
        root,
        "src/service.rs",
        "use crate::store::Store;\npub struct Service;\n",
    );
    write(root, "src/api.rs", "use crate::service::Service;\n");
    write(root, "src/unrelated.rs", "pub fn alone() {}\n");

    git(root, &["init", "-q", "-b", "main"]);
    git(root, &["config", "user.email", "t@example.com"]);
    git(root, &["config", "user.name", "Test"]);
    git(root, &["add", "-A"]);
    git(root, &["commit", "-q", "-m", "initial"]);
    dir
}

async fn impact_of(dir: &TempDir, against: Option<&str>, depth: usize) -> ImpactReport {
    let store = store().await;
    Indexer::new(store.clone())
        .reindex("demo", dir.path())
        .await
        .unwrap();
    let changes = changed_files(dir.path(), against).unwrap();
    ImpactAnalyser::new(store)
        .analyse("demo", dir.path(), &changes, depth)
        .await
        .unwrap()
}

fn impacted(report: &ImpactReport, path: &str) -> Option<usize> {
    report
        .impact
        .iter()
        .find(|i| i.path == path)
        .map(|i| i.distance)
}

#[tokio::test]
async fn a_clean_repository_reports_nothing() {
    let report = impact_of(&repo(), None, 3).await;
    assert!(report.changed.is_empty(), "{:?}", report.changed);
    assert!(report.impact.is_empty());
}

#[tokio::test]
async fn an_edit_reaches_everything_that_imports_it_transitively() {
    let dir = repo();
    write(
        dir.path(),
        "src/store.rs",
        "pub struct Store;\npub fn go() {}\n",
    );

    let report = impact_of(&dir, None, 3).await;

    assert_eq!(report.changed.len(), 1);
    assert_eq!(report.changed[0].path, "src/store.rs");
    assert!(report.changed[0].in_graph);

    // service imports store directly; api imports service.
    assert_eq!(impacted(&report, "src/service.rs"), Some(1));
    assert_eq!(impacted(&report, "src/api.rs"), Some(2));
    assert_eq!(
        impacted(&report, "src/unrelated.rs"),
        None,
        "a file with no path to the change was reported as impacted"
    );
}

#[tokio::test]
async fn depth_bounds_the_radius() {
    let dir = repo();
    write(
        dir.path(),
        "src/store.rs",
        "pub struct Store;\npub fn go() {}\n",
    );

    let report = impact_of(&dir, None, 1).await;
    assert_eq!(impacted(&report, "src/service.rs"), Some(1));
    assert_eq!(
        impacted(&report, "src/api.rs"),
        None,
        "depth 1 reached two hops away"
    );
    assert_eq!(report.depth, 1);
}

#[tokio::test]
async fn an_untracked_file_is_reported_and_marked_absent_from_the_graph() {
    let dir = repo();
    write(dir.path(), "src/brand_new.rs", "pub fn fresh() {}\n");

    // Deliberately not reindexed: this is what a new file looks like
    // to an agent that just created it.
    let store = store().await;
    Indexer::new(store.clone())
        .reindex("demo", dir.path())
        .await
        .unwrap();
    std::fs::write(dir.path().join("src/later.rs"), "pub fn later() {}\n").unwrap();

    let changes = changed_files(dir.path(), None).unwrap();
    let report = ImpactAnalyser::new(store)
        .analyse("demo", dir.path(), &changes, 3)
        .await
        .unwrap();

    let later = report
        .changed
        .iter()
        .find(|c| c.path == "src/later.rs")
        .expect("the new file was not reported");
    assert!(
        !later.in_graph,
        "a file the graph has never seen was reported as known"
    );
    assert!(report.evidence.changed_not_in_graph >= 1);
}

#[tokio::test]
async fn a_deleted_file_is_still_placed_in_the_graph_and_traced() {
    let dir = repo();
    std::fs::remove_file(dir.path().join("src/store.rs")).unwrap();

    // Indexed before the delete, so the graph still knows who imported
    // it — which is exactly what makes the deletion worth tracing.
    let store = store().await;
    Indexer::new(store.clone())
        .reindex("demo", dir.path())
        .await
        .unwrap();
    let changes = changed_files(dir.path(), None).unwrap();
    let report = ImpactAnalyser::new(store)
        .analyse("demo", dir.path(), &changes, 3)
        .await
        .unwrap();

    assert_eq!(report.changed[0].path, "src/store.rs");
    assert_eq!(
        report.changed[0].kind,
        atlas_graph::api::ChangeKind::Deleted
    );
}

#[tokio::test]
async fn a_branch_can_be_compared_against_a_ref() {
    let dir = repo();
    git(dir.path(), &["checkout", "-q", "-b", "feature"]);
    write(
        dir.path(),
        "src/store.rs",
        "pub struct Store;\npub fn go() {}\n",
    );
    git(dir.path(), &["add", "-A"]);
    git(dir.path(), &["commit", "-q", "-m", "change store"]);

    // Committed, so the working tree is clean and only a ref
    // comparison can see it.
    let clean = impact_of(&dir, None, 3).await;
    assert!(clean.changed.is_empty());

    let report = impact_of(&dir, Some("main"), 3).await;
    assert_eq!(report.changed.len(), 1);
    assert_eq!(report.changed[0].path, "src/store.rs");
    assert_eq!(impacted(&report, "src/api.rs"), Some(2));
}

#[tokio::test]
async fn a_file_edited_since_the_index_is_reported_as_such() {
    let dir = repo();
    let store = store().await;
    Indexer::new(store.clone())
        .reindex("demo", dir.path())
        .await
        .unwrap();

    // Edited after indexing: the graph now predates the change, and a
    // reader trusting the radius has to be told.
    write(
        dir.path(),
        "src/store.rs",
        "pub struct Store;\npub fn go() {}\n",
    );

    let changes = changed_files(dir.path(), None).unwrap();
    let report = ImpactAnalyser::new(store)
        .analyse("demo", dir.path(), &changes, 3)
        .await
        .unwrap();

    assert_eq!(
        report.evidence.changed_since_indexed, 1,
        "a stale file was reported as current"
    );
    // The radius still works: who imports store comes from service's
    // own import, which did not change.
    assert_eq!(impacted(&report, "src/service.rs"), Some(1));
}

#[tokio::test]
async fn reindexing_clears_the_staleness() {
    let dir = repo();
    write(
        dir.path(),
        "src/store.rs",
        "pub struct Store;\npub fn go() {}\n",
    );

    let report = impact_of(&dir, None, 3).await;
    assert_eq!(
        report.evidence.changed_since_indexed, 0,
        "a file indexed after the edit was still called stale"
    );
}

#[tokio::test]
async fn declared_layers_and_modules_name_everything_involved() {
    let dir = repo();
    write(
        dir.path(),
        "atlas.layers.toml",
        r#"
[[module]]
name = "core"
path = "src"

[[layer]]
name = "storage"
paths = ["src/store.rs"]

[[layer]]
name = "surface"
paths = ["src/api.rs", "src/service.rs"]
depends_on = ["storage"]
"#,
    );
    git(dir.path(), &["add", "-A"]);
    git(dir.path(), &["commit", "-q", "-m", "declare layers"]);
    write(
        dir.path(),
        "src/store.rs",
        "pub struct Store;\npub fn go() {}\n",
    );

    let report = impact_of(&dir, None, 3).await;

    assert_eq!(report.changed[0].layer.as_deref(), Some("storage"));
    // surface is named even though nothing in it changed: it is
    // reached, which is the point of asking.
    assert!(
        report.layers_touched.contains(&"surface".to_owned()),
        "{:?}",
        report.layers_touched
    );
    assert!(report.modules_touched.contains(&"core".to_owned()));
}

#[tokio::test]
async fn a_directory_that_is_not_a_repository_says_so() {
    let dir = TempDir::new().unwrap();
    write(dir.path(), "src/a.rs", "fn a() {}\n");

    let err = changed_files(dir.path(), None).unwrap_err();
    // An empty change list here would read as "nothing has changed".
    assert!(
        err.to_string().contains("not inside a git working tree"),
        "{err}"
    );
}

#[tokio::test]
async fn an_unknown_ref_reports_gits_own_message() {
    let dir = repo();
    let err = changed_files(dir.path(), Some("no-such-ref")).unwrap_err();
    assert!(
        err.to_string().contains("no-such-ref"),
        "the error does not name the ref that was wrong: {err}"
    );
}

#[tokio::test]
async fn a_hub_change_reports_how_many_depend_on_it() {
    let dir = TempDir::new().unwrap();
    write(dir.path(), "src/shared.rs", "pub struct Shared;\n");
    for i in 0..5 {
        write(
            dir.path(),
            &format!("src/user{i}.rs"),
            "use crate::shared::Shared;\n",
        );
    }
    git(dir.path(), &["init", "-q", "-b", "main"]);
    git(dir.path(), &["config", "user.email", "t@example.com"]);
    git(dir.path(), &["config", "user.name", "Test"]);
    git(dir.path(), &["add", "-A"]);
    git(dir.path(), &["commit", "-q", "-m", "initial"]);

    write(
        dir.path(),
        "src/shared.rs",
        "pub struct Shared;\npub fn x() {}\n",
    );
    let report = impact_of(&dir, None, 3).await;

    assert_eq!(
        report.changed[0].imported_by, 5,
        "the fan-in of a widely-used file was not reported"
    );
    assert_eq!(report.impact.len(), 5);
}
