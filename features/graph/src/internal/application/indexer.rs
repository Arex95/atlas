//! Turns a directory into a graph.
//!
//! The pipeline, and the reason for the order:
//!
//! 1. Walk the tree.
//! 2. Read each file once, hand it to every extractor that handles it.
//! 3. Run the `mentions` post-pass, which needs every filename before
//!    it can look for one.
//! 4. Assign ids, then resolve edge targets against the nodes that now
//!    exist.
//! 5. Replace the project's graph in one transaction.
//!
//! **Resolution lives here, not in extractors.** Only this stage has
//! seen every file, so only it can tell a target that names a real
//! node from a string that merely looks like one. Keeping that out of
//! extractors is what makes adding one a small, local change.
//!
//! **Full rebuild, not incremental.** Every run replaces the whole
//! project graph. The manifest needed for incremental updates is
//! written anyway, because the watcher slice needs it and backfilling
//! it later would mean a reindex nobody asked for.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::internal::domain::{
    EdgePredicate, ExtractedEdge, ExtractedNode, FileExtraction, GraphError, IndexStats,
    LAYERS_FILE, LayersFile, NodeKind,
};
use crate::internal::infrastructure::extract::imports;
use crate::internal::infrastructure::store::{EdgeRow, FileRow, NodeRow};
use crate::internal::infrastructure::{
    Extractor, FileContext, GraphStore, default_extractors, discover,
};

/// Shortest basename a `mentions` edge will consider. Below this the
/// heuristic connects files that merely share a word — `id.rs` would
/// match every sentence containing "id".
const MIN_MENTION_STEM: usize = 4;

pub struct Indexer {
    store: GraphStore,
    extractors: Vec<Box<dyn Extractor>>,
}

impl Indexer {
    #[must_use]
    pub fn new(store: GraphStore) -> Self {
        Self {
            store,
            extractors: default_extractors(),
        }
    }

    /// Indexes `root` under `project`, replacing whatever was there.
    ///
    /// # Errors
    /// `EmptyProject` or `RootNotADirectory` on bad input, `Walk` on
    /// an unreadable tree, `Storage` on any SQL failure.
    pub async fn reindex(&self, project: &str, root: &Path) -> Result<IndexStats, GraphError> {
        let started = std::time::Instant::now();
        if project.trim().is_empty() {
            return Err(GraphError::EmptyProject);
        }

        let discovered = discover(root)?;
        let mut stats = IndexStats {
            files_discovered: discovered.len(),
            ..IndexStats::default()
        };

        let mut extraction = FileExtraction::default();
        let mut manifest = Vec::new();
        let mut contents: Vec<(String, String)> = Vec::new();

        for discovered_file in &discovered {
            let Ok(source) = std::fs::read_to_string(&discovered_file.absolute_path) else {
                // Unreadable after passing the walker — a permission
                // change or a delete between listing and reading. One
                // file is not worth failing the run.
                stats.files_skipped += 1;
                continue;
            };
            let hash = blake3::hash(source.as_bytes()).to_hex().to_string();

            let context = FileContext {
                relative_path: &discovered_file.relative_path,
                extension: &discovered_file.extension,
                content: &source,
                content_hash: &hash,
            };

            let before = extraction.nodes.len();
            for extractor in &self.extractors {
                if extractor.handles(&context) {
                    extraction.merge(extractor.extract(&context));
                }
            }

            manifest.push(FileRow {
                file_path: discovered_file.relative_path.clone(),
                content_hash: hash,
                extension: discovered_file.extension.clone(),
                node_count: i64::try_from(extraction.nodes.len() - before).unwrap_or(i64::MAX),
            });
            contents.push((discovered_file.relative_path.clone(), source));
            stats.files_indexed += 1;
        }

        extraction
            .edges
            .extend(mentions(&extraction.nodes, &contents));

        let (nodes, edges) = assign_and_resolve(&extraction, &mut stats);

        stats.nodes = nodes.len();
        stats.edges = edges.len();

        self.store
            .replace_project_graph(project, &nodes, &edges, &manifest)
            .await?;

        // Captured here rather than read when findings are asked for,
        // so the analyser answers from the same snapshot the graph was
        // built from — and so the watcher keeps the declaration current
        // for free, a change to it being a change to a file like any
        // other.
        let (layers_source, layers_error) = read_layers(root);
        self.store
            .replace_project_layers(project, layers_source.as_deref(), layers_error.as_deref())
            .await?;

        stats.elapsed_ms = started.elapsed().as_millis();
        tracing::info!(
            project,
            files = stats.files_indexed,
            nodes = stats.nodes,
            edges = stats.edges,
            unresolved = stats.edges_unresolved,
            ms = stats.elapsed_ms,
            "project map reindexed",
        );
        Ok(stats)
    }
}

/// Reads a project's declaration file, as (source, `parse_error`).
///
/// A malformed declaration must not fail indexing — the rest of the
/// graph is still worth having, and a project cannot be left
/// unsearchable by a typo in a file that is optional in the first
/// place. It must not pass silently either, so the reason is carried
/// forward and reported as a finding.
fn read_layers(root: &Path) -> (Option<String>, Option<String>) {
    let Ok(source) = std::fs::read_to_string(root.join(LAYERS_FILE)) else {
        // Absent is the ordinary case, not an error.
        return (None, None);
    };
    match LayersFile::parse(&source) {
        Ok(_) => (Some(source), None),
        Err(e) => (Some(source), Some(e.to_string())),
    }
}

/// Assigns an id to every node, then resolves edge targets against
/// them.
///
/// One pass, because an edge may point at a node that appears later in
/// the list — the whole map has to exist before any target is looked
/// up.
fn assign_and_resolve(
    extraction: &FileExtraction,
    stats: &mut IndexStats,
) -> (Vec<NodeRow>, Vec<EdgeRow>) {
    let mut by_fqn: HashMap<&str, String> = HashMap::new();
    let nodes: Vec<NodeRow> = extraction
        .nodes
        .iter()
        .map(|n| {
            let id = ulid::Ulid::new().to_string();
            by_fqn.insert(n.fqn.as_str(), id.clone());
            NodeRow {
                id,
                fqn: n.fqn.clone(),
                name: n.name.clone(),
                kind: n.kind,
                extension: n.extension.clone(),
                file_path: n.file_path.clone(),
                start_line: n.start_line,
                end_line: n.end_line,
                excerpt: n.excerpt.clone(),
                content_hash: n.content_hash.clone(),
                search_text: n.search_text.clone(),
            }
        })
        .collect();

    let by_suffix = suffix_index(&nodes);
    let project_segments = project_segments(&nodes);

    let mut edges = Vec::with_capacity(extraction.edges.len());
    for edge in &extraction.edges {
        // An edge whose *source* is unknown is a bug in an extractor,
        // not a fact about the project — drop it rather than store a
        // dangling row.
        let Some(src_id) = by_fqn.get(edge.src_fqn.as_str()).cloned() else {
            continue;
        };
        let dst_id = resolve_target(edge, &by_fqn, &by_suffix, &project_segments);

        if dst_id.is_some() {
            stats.edges_resolved += 1;
        } else {
            stats.edges_unresolved += 1;
        }

        edges.push(EdgeRow {
            src_id,
            dst_id,
            dst_fqn: edge.dst_fqn.clone(),
            predicate: edge.predicate,
            file_path: edge.file_path.clone(),
            line: edge.line,
        });
    }

    (nodes, edges)
}

/// Finds the node an edge points at, or `None`.
///
/// Imports get three extra chances, because an import rarely names a
/// path exactly:
///
/// * **Extension and index candidates** — `./auth` may have meant
///   `./auth.ts` or `./auth/mod.rs`.
/// * **Suffix match** — `crate::internal::domain` names no path from
///   the project root, but exactly one file may *end* with
///   `internal/domain`. Resolved only when exactly one does;
///   ambiguity is treated as failure, not as a coin flip.
/// * **Dropping trailing segments** — see [`resolve_path`].
///
/// Everything else must name a node exactly. Guessing more widely
/// would resolve more edges and some of them would be wrong, which is
/// the trade this graph does not make.
fn resolve_target(
    edge: &ExtractedEdge,
    by_fqn: &HashMap<&str, String>,
    by_suffix: &HashMap<String, Vec<(String, String)>>,
    project_segments: &HashSet<String>,
) -> Option<String> {
    if let Some(id) = by_fqn.get(edge.dst_fqn.as_str()) {
        return Some(id.clone());
    }
    if edge.predicate != EdgePredicate::Imports {
        return None;
    }

    // Most imports name a symbol, not a file: `use
    // crate::internal::infrastructure::AfgStore` points at a type, and
    // no file is called `AfgStore`. Trailing segments are dropped one
    // at a time until something resolves.
    //
    // This is sound for a file-level graph rather than a concession to
    // it. Whatever `AfgStore` turns out to be, importing it *is* a
    // dependency on the file that defines it, and the shortest path
    // that names a real file is the closest this graph can come to
    // saying so. The alternative is what was happening before: 72 of
    // this repository's 213 internal imports invisible, and every
    // finding computed as though those dependencies did not exist.
    //
    // Over-reaching is bounded by the two rules `resolve_path` already
    // applies. A path shortened to something third-party (`axum`,
    // `std`) is refused by the external-root check rather than
    // resolved locally, and a shortened path matching several files
    // ambiguously still resolves to nothing.
    let mut path = edge.dst_fqn.as_str();
    loop {
        if let Some(id) = resolve_path(path, &edge.file_path, by_fqn, by_suffix, project_segments) {
            return Some(id);
        }
        // Stop before the path is a single bare segment: at that point
        // it names a package or nothing, and both are handled above.
        match path.rsplit_once('/') {
            Some((parent, _)) if parent.contains('/') => path = parent,
            _ => return None,
        }
    }
}

/// One resolution attempt for one exact path.
fn resolve_path(
    path: &str,
    from_file: &str,
    by_fqn: &HashMap<&str, String>,
    by_suffix: &HashMap<String, Vec<(String, String)>>,
    project_segments: &HashSet<String>,
) -> Option<String> {
    for candidate in imports::candidates(path) {
        if let Some(id) = by_fqn.get(candidate.as_str()) {
            return Some(id.clone());
        }
    }
    suffix_match(path, from_file, by_suffix, project_segments)
}

/// Longest-suffix lookup, breaking ties first on the segments the
/// suffix had to drop and only then on proximity to the importer.
///
/// `crate::internal::domain` becomes `crate/internal/domain`, whose
/// first segment names nothing; `internal/domain` then matches every
/// file ending that way. In a workspace where nine crates share an
/// internal layout that is nine candidates, and rejecting all of them
/// would leave every module-path import in the repository unresolved.
///
/// Proximity alone is not enough to choose between them, and getting
/// this wrong is worse than leaving an edge unresolved: `use
/// atlas_auth::api::…` from inside the sync crate suffix-matches
/// `api` in every crate, and the *nearest* of those is sync's own
/// `api.rs` — so a cross-crate import resolves back to its own crate
/// and manufactures a dependency cycle that does not exist.
///
/// The dropped segments are what distinguishes them. `atlas_auth`
/// names the auth crate, and only one candidate sits under a path
/// segment saying so. Matching those first, and falling back to
/// proximity only when they distinguish nothing, keeps local
/// resolution working for `crate::…` — where the dropped segment
/// genuinely names nothing — without letting it hijack an import
/// that was explicit about where it pointed.
fn suffix_match(
    target: &str,
    from_file: &str,
    by_suffix: &HashMap<String, Vec<(String, String)>>,
    project_segments: &HashSet<String>,
) -> Option<String> {
    let segments: Vec<&str> = target.split('/').filter(|s| !s.is_empty()).collect();

    for start in 0..segments.len() {
        let candidate = segments[start..].join("/");
        let Some(matches) = by_suffix.get(&candidate) else {
            continue;
        };
        let dropped = &segments[..start];
        // Checked before the single-candidate case, not only inside the
        // tie-break: `axum::extract` leaves exactly one candidate here
        // — this project's own `extract/` module — so a fast path that
        // returns a lone match unexamined fabricates the edge without
        // ever consulting the prefix that says it is third-party.
        if points_outside(dropped, project_segments) {
            return None;
        }
        return match matches.as_slice() {
            [] => None,
            [(id, _)] => Some(id.clone()),
            many => best_match(many, dropped, from_file),
        };
    }
    None
}

/// Narrows candidates by the dropped segments, then by proximity.
fn best_match(
    candidates: &[(String, String)],
    dropped: &[&str],
    from_file: &str,
) -> Option<String> {
    let named: Vec<&(String, String)> = candidates
        .iter()
        .filter(|(_, fqn)| names_any(fqn, dropped))
        .collect();

    match named.as_slice() {
        // The dropped segments named none of them. The caller has
        // already established the import points inside the project, so
        // this is a relative path whose prefix names nothing by design
        // — `crate::…` — and proximity is exactly right for it.
        [] => nearest(candidates, from_file),
        [only] => Some(only.0.clone()),
        // Several candidates named by the prefix: proximity again, but
        // now among the ones the import actually pointed at.
        many => {
            let narrowed: Vec<(String, String)> = many.iter().map(|c| (*c).clone()).collect();
            nearest(&narrowed, from_file)
        }
    }
}

/// Whether the import's root segment names something this project
/// does not contain — in which case it belongs to a dependency and
/// must not resolve to a local file.
///
/// Relative roots (`crate`, `self`, `super`, `.`, `..`) name nothing
/// by design and are internal by definition.
fn points_outside(dropped: &[&str], project_segments: &HashSet<String>) -> bool {
    const RELATIVE_ROOTS: &[&str] = &["crate", "self", "super", ".", "..", ""];

    let Some(root) = dropped.first() else {
        return false;
    };
    if RELATIVE_ROOTS.contains(root) {
        return false;
    }
    // A root spelled like a package (`atlas_auth`, `@scope/pkg`) counts
    // as internal if any of its parts names a directory here.
    !root
        .split(['_', '-', '.', '@'])
        .any(|part| project_segments.contains(part))
}

/// Whether any dropped segment names a directory on the candidate's
/// path.
///
/// A package name is compared by its parts, so `atlas_auth` matches a
/// path segment `auth`: the prefix belongs to the workspace, not to
/// the module, and every ecosystem spells that differently
/// (`atlas-auth`, `atlas_auth`, `@atlas/auth`). Parts shorter than
/// three characters are ignored — `io`, `os` and the like match
/// something in almost any repository.
fn names_any(fqn: &str, dropped: &[&str]) -> bool {
    const MIN_PART: usize = 3;

    let path: Vec<&str> = fqn.split('/').collect();
    dropped
        .iter()
        .flat_map(|segment| segment.split(['_', '-', '.', '@']))
        .filter(|part| part.len() >= MIN_PART)
        .any(|part| {
            path.iter().any(|seg| {
                // The last segment carries an extension; compare the
                // stem so `auth` matches `auth.rs`.
                seg.rsplit_once('.').map_or(*seg, |(stem, _)| stem) == part
            })
        })
}

/// The candidate sharing the most leading path segments with
/// `from_file`, or `None` if two share equally many.
fn nearest(candidates: &[(String, String)], from_file: &str) -> Option<String> {
    let from: Vec<&str> = from_file.split('/').collect();

    let scored: Vec<(usize, &String)> = candidates
        .iter()
        .map(|(id, fqn)| {
            let shared = fqn
                .split('/')
                .zip(from.iter())
                .take_while(|(a, b)| a == *b)
                .count();
            (shared, id)
        })
        .collect();

    let best = scored.iter().map(|(n, _)| *n).max()?;
    let mut winners = scored.iter().filter(|(n, _)| *n == best);
    let first = winners.next()?;
    // Two candidates equally close: nothing distinguishes them, and
    // guessing is the one thing this resolver does not do.
    if winners.next().is_some() {
        return None;
    }
    Some(first.1.clone())
}

/// Every directory and file stem appearing anywhere in the project.
///
/// Used to tell an import that points inside the project from one that
/// points at a dependency, without needing to read a manifest for
/// every ecosystem.
fn project_segments(nodes: &[NodeRow]) -> HashSet<String> {
    nodes
        .iter()
        .flat_map(|n| n.fqn.split('/'))
        .map(|seg| {
            seg.rsplit_once('.')
                .map_or(seg, |(stem, _)| stem)
                .to_owned()
        })
        .filter(|seg| !seg.is_empty())
        .collect()
}

/// Every trailing path fragment of every file, mapped to the nodes
/// that end that way.
///
/// Built once per index because a suffix lookup per edge over every
/// node would be quadratic on a large repository.
fn suffix_index(nodes: &[NodeRow]) -> HashMap<String, Vec<(String, String)>> {
    let mut index: HashMap<String, Vec<(String, String)>> = HashMap::new();

    for node in nodes.iter().filter(|n| n.kind == NodeKind::File) {
        let stripped = node
            .fqn
            .rsplit_once('.')
            .map_or(node.fqn.as_str(), |(p, _)| p);
        let segments: Vec<&str> = stripped.split('/').collect();

        let mut forms = vec![stripped.to_owned()];
        // `a/b/mod.rs` is what `a::b` means, so register the parent
        // path too — otherwise every Rust module import misses.
        if let Some(parent) = segments
            .last()
            .filter(|last| matches!(**last, "mod" | "index" | "__init__"))
            .and(Some(&segments[..segments.len() - 1]))
            .filter(|p| !p.is_empty())
        {
            forms.push(parent.join("/"));
        }

        for form in forms {
            let parts: Vec<&str> = form.split('/').collect();
            for start in 0..parts.len() {
                let suffix = parts[start..].join("/");
                let bucket = index.entry(suffix).or_default();
                if !bucket.iter().any(|(id, _)| id == &node.id) {
                    bucket.push((node.id.clone(), node.fqn.clone()));
                }
            }
        }
    }

    index
}

/// The `mentions` post-pass: one file naming another in its text.
///
/// Runs here rather than in an extractor because it needs every
/// filename in the project before it can look for one — exactly the
/// cross-file knowledge extractors are not allowed.
///
/// Deliberately conservative, because this is the one heuristic
/// predicate and a wrong edge here is indistinguishable from a real
/// one to whoever follows it:
///
/// * only basenames unique across the project — two `mod.rs` would
///   make every mention ambiguous, so neither is considered;
/// * only stems of [`MIN_MENTION_STEM`] characters or more;
/// * a match must not be part of a longer word, so `auth` does not
///   match `author`.
fn mentions(nodes: &[ExtractedNode], contents: &[(String, String)]) -> Vec<ExtractedEdge> {
    let mut seen: HashMap<&str, Option<&str>> = HashMap::new();
    for node in nodes.iter().filter(|n| n.kind == NodeKind::File) {
        let Some(base) = node.fqn.rsplit('/').next() else {
            continue;
        };
        let stem = base.split('.').next().unwrap_or(base);
        if stem.len() < MIN_MENTION_STEM {
            continue;
        }
        // Second sighting marks it ambiguous rather than removing it,
        // so a third does not resurrect it.
        seen.entry(stem)
            .and_modify(|slot| *slot = None)
            .or_insert(Some(node.fqn.as_str()));
    }

    let unique: HashMap<&str, &str> = seen
        .into_iter()
        .filter_map(|(stem, target)| target.map(|t| (stem, t)))
        .collect();

    let mut out = Vec::new();
    for (path, content) in contents {
        let mut already: HashSet<&str> = HashSet::new();
        for (stem, target) in &unique {
            if *target == path.as_str() || already.contains(*target) {
                continue;
            }
            if let Some(line) = line_of_word(content, stem) {
                already.insert(target);
                out.push(ExtractedEdge {
                    src_fqn: path.clone(),
                    dst_fqn: (*target).to_owned(),
                    predicate: EdgePredicate::Mentions,
                    file_path: path.clone(),
                    line,
                });
            }
        }
    }
    out
}

/// The first line containing `word` as a whole word, 1-indexed.
fn line_of_word(content: &str, word: &str) -> Option<i64> {
    for (index, line) in content.lines().enumerate() {
        let mut from = 0;
        while let Some(found) = line[from..].find(word) {
            let start = from + found;
            let end = start + word.len();
            let before_ok = start == 0
                || !line[..start]
                    .chars()
                    .next_back()
                    .is_some_and(|c| c.is_alphanumeric() || c == '_');
            let after_ok = line[end..]
                .chars()
                .next()
                .is_none_or(|c| !(c.is_alphanumeric() || c == '_'));
            if before_ok && after_ok {
                return Some(i64::try_from(index + 1).unwrap_or(i64::MAX));
            }
            from = end;
        }
    }
    None
}
