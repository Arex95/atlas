//! Integration tests for Project Map against a real filesystem and a
//! real `SQLite` database. Every fixture is written to a temp dir and
//! walked for real — nothing here mocks a file.

use std::path::Path;

use atlas_graph::api::{EdgePredicate, GraphStore, Indexer, NodeKind, SqlitePool, run_migrations};
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

/// A small project exercising every extractor at once.
fn fixture() -> TempDir {
    let dir = TempDir::new().unwrap();
    let root = dir.path();

    write(
        root,
        "README.md",
        "# Overview\n\nThe entry point is [the server](src/server.rs).\n\n\
         ## Setup\n\nRun it.\n\n\
         ### Prerequisites\n\nRust.\n\n\
         ## Usage\n\nSee [setup](#setup).\n",
    );
    write(
        root,
        "src/server.rs",
        "use crate::config;\nuse std::sync::Arc;\n\nfn main() {}\n",
    );
    write(root, "src/config.rs", "pub struct Config;\n");
    write(root, "src/lonely.rs", "pub fn nobody_calls_me() {}\n");
    dir
}

#[tokio::test]
async fn indexes_files_and_markdown_sections() {
    let store = store().await;
    let dir = fixture();
    let stats = Indexer::new(store.clone())
        .reindex("demo", dir.path())
        .await
        .unwrap();

    assert_eq!(stats.files_indexed, 4);

    let overview = store.overview("demo").await.unwrap();
    assert_eq!(overview.files, 4);
    // Overview, Setup, Prerequisites, Usage.
    assert_eq!(overview.sections, 4);

    let node = store.node("demo", "src/server.rs").await.unwrap();
    assert_eq!(node.kind, NodeKind::File);
    assert_eq!(node.extension, "rs");
    assert!(node.excerpt.is_some());
}

#[tokio::test]
async fn markdown_sections_nest_by_heading_level() {
    let store = store().await;
    let dir = fixture();
    Indexer::new(store.clone())
        .reindex("demo", dir.path())
        .await
        .unwrap();

    let outline = store.outline("demo", "README.md").await.unwrap();
    let names: Vec<&str> = outline.iter().map(|n| n.name.as_str()).collect();
    assert_eq!(names, vec!["Overview", "Setup", "Prerequisites", "Usage"]);

    // "Prerequisites" (###) is contained by "Setup" (##), not by the
    // file and not by "Overview".
    let setup = store.related("demo", "README.md#setup").await.unwrap();
    let contained: Vec<&str> = setup
        .outgoing
        .iter()
        .filter(|e| e.predicate == EdgePredicate::Contains)
        .map(|e| e.dst_fqn.as_str())
        .collect();
    assert_eq!(contained, vec!["README.md#prerequisites"]);
}

#[tokio::test]
async fn a_markdown_link_becomes_a_resolved_edge() {
    let store = store().await;
    let dir = fixture();
    Indexer::new(store.clone())
        .reindex("demo", dir.path())
        .await
        .unwrap();

    let server = store.related("demo", "src/server.rs").await.unwrap();
    let linked_from: Vec<&str> = server
        .incoming
        .iter()
        .filter(|e| e.predicate == EdgePredicate::Links)
        .map(|e| e.dst_fqn.as_str())
        .collect();
    assert!(
        !linked_from.is_empty(),
        "the README link to src/server.rs produced no edge"
    );
    assert!(
        server
            .incoming
            .iter()
            .any(|e| e.predicate == EdgePredicate::Links && e.dst_id.is_some()),
        "the link edge should have resolved to a real node"
    );
}

#[tokio::test]
async fn a_relative_import_resolves_to_the_file_it_names() {
    let store = store().await;
    let dir = TempDir::new().unwrap();
    write(dir.path(), "src/a.ts", "import { b } from './b';\n");
    write(dir.path(), "src/b.ts", "export const b = 1;\n");

    Indexer::new(store.clone())
        .reindex("demo", dir.path())
        .await
        .unwrap();

    let a = store.related("demo", "src/a.ts").await.unwrap();
    let import = a
        .outgoing
        .iter()
        .find(|e| e.predicate == EdgePredicate::Imports)
        .expect("no import edge");
    // './b' had no extension; the indexer tried the candidates.
    assert_eq!(import.dst_fqn, "src/b");
    assert!(
        import.dst_id.is_some(),
        "the import should have resolved to src/b.ts"
    );
}

#[tokio::test]
async fn an_unresolvable_target_is_kept_but_left_unresolved() {
    let store = store().await;
    let dir = TempDir::new().unwrap();
    write(dir.path(), "src/a.ts", "import { x } from './missing';\n");

    let stats = Indexer::new(store.clone())
        .reindex("demo", dir.path())
        .await
        .unwrap();
    assert_eq!(stats.edges_unresolved, 1);

    let a = store.related("demo", "src/a.ts").await.unwrap();
    let edge = a.outgoing.first().expect("edge was dropped entirely");
    assert_eq!(edge.dst_fqn, "src/missing");
    assert!(
        edge.dst_id.is_none(),
        "an unresolved target must not be pointed at a guess"
    );
}

#[tokio::test]
async fn a_dependency_import_produces_no_edge_at_all() {
    let store = store().await;
    let dir = TempDir::new().unwrap();
    write(dir.path(), "src/a.ts", "import React from 'react';\n");

    let stats = Indexer::new(store.clone())
        .reindex("demo", dir.path())
        .await
        .unwrap();
    // 'react' names a dependency, not a file here. Emitting an edge
    // that can never resolve would be noise in every project.
    assert_eq!(stats.edges, 0);
}

#[tokio::test]
async fn headings_inside_a_code_fence_are_not_sections() {
    let store = store().await;
    let dir = TempDir::new().unwrap();
    write(
        dir.path(),
        "doc.md",
        "# Real\n\n```bash\n# not a heading\necho hi\n```\n\n## Also real\n",
    );

    Indexer::new(store.clone())
        .reindex("demo", dir.path())
        .await
        .unwrap();

    let outline = store.outline("demo", "doc.md").await.unwrap();
    let names: Vec<&str> = outline.iter().map(|n| n.name.as_str()).collect();
    assert_eq!(names, vec!["Real", "Also real"]);
}

#[tokio::test]
async fn repeated_headings_get_distinct_identities() {
    let store = store().await;
    let dir = TempDir::new().unwrap();
    write(dir.path(), "doc.md", "## Notes\n\na\n\n## Notes\n\nb\n");

    Indexer::new(store.clone())
        .reindex("demo", dir.path())
        .await
        .unwrap();

    let outline = store.outline("demo", "doc.md").await.unwrap();
    assert_eq!(outline.len(), 2, "the second heading overwrote the first");
    assert_eq!(outline[0].fqn, "doc.md#notes");
    assert_eq!(outline[1].fqn, "doc.md#notes-2");
}

#[tokio::test]
async fn gitignored_and_blacklisted_paths_are_skipped() {
    let store = store().await;
    let dir = TempDir::new().unwrap();
    write(dir.path(), ".gitignore", "ignored.rs\n");
    write(dir.path(), "kept.rs", "fn kept() {}\n");
    write(dir.path(), "ignored.rs", "fn ignored() {}\n");
    write(
        dir.path(),
        "node_modules/dep/index.js",
        "module.exports = 1;\n",
    );
    write(dir.path(), "target/debug/build.rs", "fn generated() {}\n");
    write(dir.path(), "Cargo.lock", "# lockfile\n");

    let stats = Indexer::new(store.clone())
        .reindex("demo", dir.path())
        .await
        .unwrap();

    // kept.rs and .gitignore itself. Nothing else.
    assert_eq!(stats.files_indexed, 2, "walker let something through");
    assert!(store.node("demo", "kept.rs").await.is_ok());
    assert!(store.node("demo", "ignored.rs").await.is_err());
    assert!(
        store
            .node("demo", "node_modules/dep/index.js")
            .await
            .is_err()
    );
    assert!(store.node("demo", "target/debug/build.rs").await.is_err());
}

#[tokio::test]
async fn reindexing_twice_produces_the_same_graph() {
    let store = store().await;
    let dir = fixture();
    let indexer = Indexer::new(store.clone());

    let first = indexer.reindex("demo", dir.path()).await.unwrap();
    let after_first = store.overview("demo").await.unwrap();

    let second = indexer.reindex("demo", dir.path()).await.unwrap();
    let after_second = store.overview("demo").await.unwrap();

    assert_eq!(first.nodes, second.nodes);
    assert_eq!(first.edges, second.edges);
    assert_eq!(after_first.files, after_second.files);
    assert_eq!(after_first.sections, after_second.sections);
    assert_eq!(
        after_first.edges_by_predicate, after_second.edges_by_predicate,
        "a second index changed the graph"
    );
}

#[tokio::test]
async fn two_projects_on_one_server_never_see_each_other() {
    let store = store().await;
    let a = TempDir::new().unwrap();
    write(a.path(), "only-in-a.rs", "fn a() {}\n");
    let b = TempDir::new().unwrap();
    write(b.path(), "only-in-b.rs", "fn b() {}\n");

    let indexer = Indexer::new(store.clone());
    indexer.reindex("project-a", a.path()).await.unwrap();
    indexer.reindex("project-b", b.path()).await.unwrap();

    assert!(store.node("project-a", "only-in-a.rs").await.is_ok());
    assert!(store.node("project-a", "only-in-b.rs").await.is_err());
    assert_eq!(store.overview("project-a").await.unwrap().files, 1);

    // And reindexing one does not wipe the other.
    indexer.reindex("project-a", a.path()).await.unwrap();
    assert_eq!(store.overview("project-b").await.unwrap().files, 1);

    let hits = store.find("project-b", "only", 10).await.unwrap();
    assert!(hits.iter().all(|n| n.project == "project-b"));
}

#[tokio::test]
async fn search_finds_nodes_by_content_and_by_name() {
    let store = store().await;
    let dir = TempDir::new().unwrap();
    write(
        dir.path(),
        "src/authentication.rs",
        "fn verify_bearer_token() {}\n",
    );
    write(dir.path(), "src/other.rs", "fn unrelated() {}\n");

    Indexer::new(store.clone())
        .reindex("demo", dir.path())
        .await
        .unwrap();

    let by_content = store.find("demo", "verify_bearer_token", 10).await.unwrap();
    assert!(
        by_content.iter().any(|n| n.fqn == "src/authentication.rs"),
        "content search missed the file containing the term"
    );

    let by_name = store.find("demo", "authentication", 10).await.unwrap();
    assert!(by_name.iter().any(|n| n.fqn == "src/authentication.rs"));
}

#[tokio::test]
async fn a_malformed_search_returns_nothing_rather_than_failing() {
    let store = store().await;
    let dir = fixture();
    Indexer::new(store.clone())
        .reindex("demo", dir.path())
        .await
        .unwrap();

    // FTS5 rejects this at execution. An agent searching for a
    // fragment of code should get no results, not an error.
    let hits = store.find("demo", "foo(", 10).await.unwrap();
    assert!(hits.is_empty());
}

#[tokio::test]
async fn overview_reports_hubs_and_orphans() {
    let store = store().await;
    let dir = fixture();
    Indexer::new(store.clone())
        .reindex("demo", dir.path())
        .await
        .unwrap();

    let overview = store.overview("demo").await.unwrap();
    assert!(
        overview.hubs.iter().any(|(fqn, _)| fqn == "src/server.rs"),
        "the file the README links to should rank as a hub"
    );
    assert!(
        overview
            .edges_by_predicate
            .iter()
            .any(|(p, n)| p == "contains" && *n > 0)
    );
}

#[tokio::test]
async fn a_module_path_import_resolves_by_unique_suffix() {
    let store = store().await;
    let dir = TempDir::new().unwrap();
    // Rust: `use crate::internal::domain` names no path from the root,
    // but exactly one file ends with `internal/domain`.
    write(
        dir.path(),
        "features/graph/src/lib.rs",
        "use crate::internal::domain;\n",
    );
    write(
        dir.path(),
        "features/graph/src/internal/domain/mod.rs",
        "pub struct Thing;\n",
    );

    Indexer::new(store.clone())
        .reindex("demo", dir.path())
        .await
        .unwrap();

    let lib = store
        .related("demo", "features/graph/src/lib.rs")
        .await
        .unwrap();
    let import = lib
        .outgoing
        .iter()
        .find(|e| e.predicate == EdgePredicate::Imports)
        .expect("no import edge for a module path");
    assert!(
        import.dst_id.is_some(),
        "crate::internal::domain should have resolved to the mod.rs that ends with it"
    );
}

#[tokio::test]
async fn a_module_path_prefers_the_candidate_nearest_the_importer() {
    let store = store().await;
    let dir = TempDir::new().unwrap();
    // Two crates share an internal layout, which is the normal shape of
    // a workspace. `use crate::internal::domain` from inside `a` means
    // a's copy — that is what `crate` means, and proximity is how a
    // language-agnostic resolver can tell.
    write(dir.path(), "a/src/lib.rs", "use crate::internal::domain;\n");
    write(
        dir.path(),
        "a/src/internal/domain/mod.rs",
        "pub struct A;\n",
    );
    write(
        dir.path(),
        "b/src/internal/domain/mod.rs",
        "pub struct B;\n",
    );

    Indexer::new(store.clone())
        .reindex("demo", dir.path())
        .await
        .unwrap();

    let lib = store.related("demo", "a/src/lib.rs").await.unwrap();
    let import = lib
        .outgoing
        .iter()
        .find(|e| e.predicate == EdgePredicate::Imports)
        .expect("no import edge");
    assert!(import.dst_id.is_some(), "the nearer candidate should win");

    // And it is a's, not b's: the node it points at lives under a/.
    let a_domain = store
        .related("demo", "a/src/internal/domain/mod.rs")
        .await
        .unwrap();
    assert!(
        a_domain
            .incoming
            .iter()
            .any(|e| e.predicate == EdgePredicate::Imports && e.file_path == "a/src/lib.rs"),
        "the import resolved to the wrong crate's copy"
    );
}

#[tokio::test]
async fn a_genuinely_equidistant_module_path_resolves_to_nothing() {
    let store = store().await;
    let dir = TempDir::new().unwrap();
    // The importer is equally far from both candidates, so proximity
    // cannot break the tie. Picking either would be a coin flip.
    write(dir.path(), "main.rs", "use crate::internal::domain;\n");
    write(
        dir.path(),
        "a/src/internal/domain/mod.rs",
        "pub struct A;\n",
    );
    write(
        dir.path(),
        "b/src/internal/domain/mod.rs",
        "pub struct B;\n",
    );

    Indexer::new(store.clone())
        .reindex("demo", dir.path())
        .await
        .unwrap();

    let main = store.related("demo", "main.rs").await.unwrap();
    let import = main
        .outgoing
        .iter()
        .find(|e| e.predicate == EdgePredicate::Imports)
        .expect("the edge should still exist, unresolved");
    assert!(
        import.dst_id.is_none(),
        "a true tie must not be resolved to one of the candidates"
    );
}

#[tokio::test]
async fn a_bare_dependency_name_is_still_ignored() {
    let store = store().await;
    let dir = TempDir::new().unwrap();
    write(dir.path(), "src/a.rs", "use serde;\nuse std::sync::Arc;\n");

    let stats = Indexer::new(store.clone())
        .reindex("demo", dir.path())
        .await
        .unwrap();
    // `serde` is one segment — a dependency. `std::sync::Arc` has
    // several, so it becomes an unresolved edge rather than a wrong
    // one: nothing in the project ends with sync/Arc.
    let a = store.related("demo", "src/a.rs").await.unwrap();
    assert!(
        a.outgoing
            .iter()
            .filter(|e| e.predicate == EdgePredicate::Imports)
            .all(|e| e.dst_id.is_none()),
        "a dependency import must never resolve to a project file"
    );
    assert!(stats.edges_resolved == 0 || stats.edges_unresolved > 0);
}

/// The shape that produced the bug these two tests exist for: several
/// crates with an identical internal layout, so every module path
/// suffix-matches in all of them.
fn workspace() -> TempDir {
    let dir = TempDir::new().unwrap();
    for crate_name in ["auth", "sync"] {
        write(
            dir.path(),
            &format!("features/{crate_name}/src/api.rs"),
            "pub use crate::internal::application::router;\n",
        );
        write(
            dir.path(),
            &format!("features/{crate_name}/src/internal/application/router.rs"),
            "pub fn routes() {}\n",
        );
    }
    dir
}

#[tokio::test]
async fn a_cross_crate_import_resolves_to_the_named_crate() {
    let store = store().await;
    let dir = workspace();
    // The importer sits in `sync`, so the *nearest* `api.rs` is its
    // own. The import names auth, and that has to win — resolving it
    // to sync's own api.rs would invent a dependency cycle where
    // there is none.
    write(
        dir.path(),
        "features/sync/src/internal/application/router.rs",
        "use atlas_auth::api::{AuthStore, User};\n",
    );

    Indexer::new(store.clone())
        .reindex("demo", dir.path())
        .await
        .unwrap();

    let auth_api = store
        .related("demo", "features/auth/src/api.rs")
        .await
        .unwrap();
    assert!(
        auth_api
            .incoming
            .iter()
            .any(|e| e.predicate == EdgePredicate::Imports
                && e.file_path == "features/sync/src/internal/application/router.rs"),
        "the cross-crate import did not reach the crate it named"
    );

    let sync_api = store
        .related("demo", "features/sync/src/api.rs")
        .await
        .unwrap();
    assert!(
        !sync_api
            .incoming
            .iter()
            .any(|e| e.predicate == EdgePredicate::Imports
                && e.file_path == "features/sync/src/internal/application/router.rs"),
        "the import resolved back into its own crate"
    );
}

#[tokio::test]
async fn a_third_party_import_never_resolves_to_a_local_file() {
    let store = store().await;
    let dir = TempDir::new().unwrap();
    // The project has its own `extract` module, and `axum::extract`
    // ends the same way. Proximity alone would happily connect them.
    write(
        dir.path(),
        "src/infrastructure/extract/mod.rs",
        "pub fn go() {}\n",
    );
    write(
        dir.path(),
        "src/router.rs",
        // A braced import reduces to exactly `axum/extract`, which is
        // the shape that matters: it leaves a single suffix candidate,
        // so a resolver checking only ambiguous matches lets it
        // through. `State` on the end would have hidden the bug.
        "use axum::extract::{Query, State};\nuse crate::infrastructure::extract;\n",
    );

    Indexer::new(store.clone())
        .reindex("demo", dir.path())
        .await
        .unwrap();

    let extract = store
        .related("demo", "src/infrastructure/extract/mod.rs")
        .await
        .unwrap();
    let incoming: Vec<_> = extract
        .incoming
        .iter()
        .filter(|e| e.predicate == EdgePredicate::Imports)
        .collect();
    // The local import must still resolve; only the axum one is
    // refused. Both come from the same file, so counting is enough.
    assert_eq!(
        incoming.len(),
        1,
        "expected only the crate-relative import to resolve, got {incoming:?}"
    );
}

#[tokio::test]
async fn an_import_naming_a_symbol_resolves_to_the_file_defining_it() {
    let store = store().await;
    let dir = TempDir::new().unwrap();
    // No file is called `AfgStore`. The dependency on the module that
    // defines it is real all the same, and this is the only way a
    // file-level graph can express it.
    write(
        dir.path(),
        "src/internal/infrastructure/mod.rs",
        "pub struct AfgStore;\n",
    );
    write(
        dir.path(),
        "src/api.rs",
        "pub use crate::internal::infrastructure::AfgStore;\n",
    );

    Indexer::new(store.clone())
        .reindex("demo", dir.path())
        .await
        .unwrap();

    let infra = store
        .related("demo", "src/internal/infrastructure/mod.rs")
        .await
        .unwrap();
    assert!(
        infra
            .incoming
            .iter()
            .any(|e| e.predicate == EdgePredicate::Imports && e.file_path == "src/api.rs"),
        "a symbol import left the dependency invisible"
    );
}

#[tokio::test]
async fn shortening_never_reaches_a_third_party_package() {
    let store = store().await;
    let dir = TempDir::new().unwrap();
    write(dir.path(), "src/extract/mod.rs", "pub fn go() {}\n");
    // Shortened, `axum::extract::State` becomes `axum/extract` — which
    // is exactly the shape that used to resolve onto the local
    // `extract/` module. Dropping segments must not walk an import
    // back into the project.
    write(dir.path(), "src/router.rs", "use axum::extract::State;\n");

    Indexer::new(store.clone())
        .reindex("demo", dir.path())
        .await
        .unwrap();

    let extract = store.related("demo", "src/extract/mod.rs").await.unwrap();
    assert!(
        !extract
            .incoming
            .iter()
            .any(|e| e.predicate == EdgePredicate::Imports),
        "shortening resolved a third-party import to a local file"
    );
}

#[tokio::test]
async fn shortening_stops_rather_than_guessing_between_candidates() {
    let store = store().await;
    let dir = TempDir::new().unwrap();
    // Two equally-near `handlers` modules. Shortening reaches an
    // ambiguous path, and ambiguity still resolves to nothing.
    write(dir.path(), "one/handlers/mod.rs", "pub struct Thing;\n");
    write(dir.path(), "two/handlers/mod.rs", "pub struct Thing;\n");
    write(dir.path(), "app.rs", "use handlers::Thing;\n");

    Indexer::new(store.clone())
        .reindex("demo", dir.path())
        .await
        .unwrap();

    let app = store.related("demo", "app.rs").await.unwrap();
    assert!(
        app.outgoing
            .iter()
            .filter(|e| e.predicate == EdgePredicate::Imports)
            .all(|e| e.dst_id.is_none()),
        "an ambiguous shortened path was resolved to one of the candidates"
    );
}
