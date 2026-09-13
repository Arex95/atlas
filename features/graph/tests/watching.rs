//! Integration tests for the auto-reindexing watcher.
//!
//! Every one of these writes real files and waits for a real
//! filesystem event. There is no way to test a watcher without doing
//! that — a mocked event proves the handler runs, not that the
//! watcher was ever wired to the disk.

use std::path::Path;
use std::time::Duration;

use atlas_graph::api::{GraphStore, GraphWatcher, SqlitePool, run_migrations};
use tempfile::TempDir;

/// Long enough for the 500 ms debounce plus a full reindex, short
/// enough that a broken watcher fails the suite quickly rather than
/// hanging it.
const SETTLE: Duration = Duration::from_secs(6);

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

/// Polls until the project has `expected` file nodes, or gives up.
///
/// Polling the *assertion* rather than sleeping a fixed time: a fast
/// machine finishes in a fraction of the budget, and a slow one still
/// passes.
async fn wait_for_files(store: &GraphStore, project: &str, expected: i64) -> i64 {
    let deadline = tokio::time::Instant::now() + SETTLE;
    loop {
        let files = store.overview(project).await.unwrap().files;
        if files == expected || tokio::time::Instant::now() >= deadline {
            return files;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

#[tokio::test]
async fn watching_indexes_once_up_front() {
    let store = store().await;
    let dir = TempDir::new().unwrap();
    write(dir.path(), "a.rs", "fn a() {}\n");

    let watcher = GraphWatcher::new(store.clone());
    let stats = watcher.watch("demo", dir.path()).await.unwrap();

    // The initial index completes before `watch` returns, so a caller
    // that got Ok knows the graph is current.
    assert_eq!(stats.files_indexed, 1);
    assert_eq!(store.overview("demo").await.unwrap().files, 1);
}

#[tokio::test]
async fn a_new_file_reaches_the_graph_without_being_asked() {
    let store = store().await;
    let dir = TempDir::new().unwrap();
    write(dir.path(), "a.rs", "fn a() {}\n");

    let watcher = GraphWatcher::new(store.clone());
    watcher.watch("demo", dir.path()).await.unwrap();
    assert_eq!(store.overview("demo").await.unwrap().files, 1);

    // Nothing below calls reindex. If the count moves, the watcher
    // moved it.
    write(dir.path(), "b.rs", "fn b() {}\n");

    assert_eq!(
        wait_for_files(&store, "demo", 2).await,
        2,
        "a new file never reached the graph"
    );
}

#[tokio::test]
async fn an_edit_reaches_the_graph() {
    let store = store().await;
    let dir = TempDir::new().unwrap();
    write(dir.path(), "doc.md", "# One\n");

    let watcher = GraphWatcher::new(store.clone());
    watcher.watch("demo", dir.path()).await.unwrap();
    assert_eq!(store.overview("demo").await.unwrap().sections, 1);

    write(dir.path(), "doc.md", "# One\n\n## Two\n");

    let deadline = tokio::time::Instant::now() + SETTLE;
    loop {
        let sections = store.overview("demo").await.unwrap().sections;
        if sections == 2 {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "an edit never reached the graph (sections stayed at {sections})"
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

#[tokio::test]
async fn a_deleted_file_leaves_the_graph() {
    let store = store().await;
    let dir = TempDir::new().unwrap();
    write(dir.path(), "a.rs", "fn a() {}\n");
    write(dir.path(), "b.rs", "fn b() {}\n");

    let watcher = GraphWatcher::new(store.clone());
    watcher.watch("demo", dir.path()).await.unwrap();
    assert_eq!(store.overview("demo").await.unwrap().files, 2);

    std::fs::remove_file(dir.path().join("b.rs")).unwrap();

    assert_eq!(
        wait_for_files(&store, "demo", 1).await,
        1,
        "a deleted file stayed in the graph"
    );
    assert!(store.node("demo", "b.rs").await.is_err());
}

#[tokio::test]
async fn churn_under_an_ignored_directory_triggers_nothing() {
    let store = store().await;
    let dir = TempDir::new().unwrap();
    write(dir.path(), "a.rs", "fn a() {}\n");

    let watcher = GraphWatcher::new(store.clone());
    watcher.watch("demo", dir.path()).await.unwrap();

    // What a `cargo build` looks like from here. A watcher that does
    // not filter would reindex for as long as the build runs.
    for i in 0..40 {
        write(
            dir.path(),
            &format!("target/debug/artifact-{i}.o"),
            "binary-ish\n",
        );
    }
    write(
        dir.path(),
        "node_modules/dep/index.js",
        "module.exports=1;\n",
    );

    tokio::time::sleep(Duration::from_secs(2)).await;

    let status = watcher.status().await;
    let demo = status.first().expect("the watch disappeared");
    assert_eq!(
        demo.reindexes, 0,
        "ignored churn triggered {} reindexes",
        demo.reindexes
    );
    assert_eq!(store.overview("demo").await.unwrap().files, 1);
}

#[tokio::test]
async fn a_burst_of_changes_collapses_into_one_reindex() {
    let store = store().await;
    let dir = TempDir::new().unwrap();
    write(dir.path(), "a.rs", "fn a() {}\n");

    let watcher = GraphWatcher::new(store.clone());
    watcher.watch("demo", dir.path()).await.unwrap();

    // A branch switch, roughly. Written back to back so they land
    // inside one debounce window.
    for i in 0..25 {
        write(dir.path(), &format!("file-{i}.rs"), "fn f() {}\n");
    }

    assert_eq!(wait_for_files(&store, "demo", 26).await, 26);

    let status = watcher.status().await;
    let demo = status.first().unwrap();
    assert!(
        demo.reindexes <= 3,
        "25 files in one burst caused {} reindexes; the debounce is not collapsing them",
        demo.reindexes
    );
}

#[tokio::test]
async fn unwatching_stops_the_updates() {
    let store = store().await;
    let dir = TempDir::new().unwrap();
    write(dir.path(), "a.rs", "fn a() {}\n");

    let watcher = GraphWatcher::new(store.clone());
    watcher.watch("demo", dir.path()).await.unwrap();
    assert!(watcher.unwatch("demo").await);
    assert!(watcher.status().await.is_empty());

    write(dir.path(), "b.rs", "fn b() {}\n");
    tokio::time::sleep(Duration::from_secs(2)).await;

    assert_eq!(
        store.overview("demo").await.unwrap().files,
        1,
        "the graph kept updating after unwatch"
    );
    // Unwatching again is not an error — a caller cleaning up should
    // not have to check first.
    assert!(!watcher.unwatch("demo").await);
}

#[tokio::test]
async fn watching_twice_replaces_rather_than_doubles() {
    let store = store().await;
    let dir = TempDir::new().unwrap();
    write(dir.path(), "a.rs", "fn a() {}\n");

    let watcher = GraphWatcher::new(store.clone());
    watcher.watch("demo", dir.path()).await.unwrap();
    watcher.watch("demo", dir.path()).await.unwrap();

    assert_eq!(
        watcher.status().await.len(),
        1,
        "the second watch was added alongside the first"
    );

    write(dir.path(), "b.rs", "fn b() {}\n");
    assert_eq!(wait_for_files(&store, "demo", 2).await, 2);

    // Two watchers would each reindex the same change.
    let demo = watcher.status().await;
    assert!(
        demo[0].reindexes <= 2,
        "one change caused {} reindexes — a leaked watcher is still running",
        demo[0].reindexes
    );
}

#[tokio::test]
async fn two_projects_are_watched_independently() {
    let store = store().await;
    let a = TempDir::new().unwrap();
    let b = TempDir::new().unwrap();
    write(a.path(), "a.rs", "fn a() {}\n");
    write(b.path(), "b.rs", "fn b() {}\n");

    let watcher = GraphWatcher::new(store.clone());
    watcher.watch("project-a", a.path()).await.unwrap();
    watcher.watch("project-b", b.path()).await.unwrap();
    assert_eq!(watcher.status().await.len(), 2);

    write(a.path(), "a2.rs", "fn a2() {}\n");
    assert_eq!(wait_for_files(&store, "project-a", 2).await, 2);

    // b was not touched and must not have been reindexed into
    // something else's shape.
    assert_eq!(store.overview("project-b").await.unwrap().files, 1);
    assert!(store.node("project-b", "b.rs").await.is_ok());

    watcher.unwatch("project-a").await;
    assert_eq!(watcher.status().await.len(), 1);
}
