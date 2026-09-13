//! Schemas for the `graph.*` tools.
//!
//! Split by family so adding a tool touches one small file
//! instead of a 900-line one every branch also edits.

use serde_json::{Value, json};

use super::ToolName;

/// Every memory tool takes an explicit `scope` (Type 1 /
/// Type 2 split). The scoping field that goes with it is required and
/// the other one is rejected — the wire shape mirrors the CHECK
/// constraint in the schema rather than restating it loosely.
/// Project Map. The descriptions steer deliberately: an agent that
/// reaches for `find` first uses this as grep with extra steps, so
/// each tool says what it is for relative to the others.
pub(super) fn graph_schema(t: ToolName) -> Value {
    let project = json!({ "type": "string", "description": "the project these nodes belong to" });

    match t {
        ToolName::GraphReindex => json!({
            "name": t.as_str(),
            "description": "Build or rebuild a project's map from disk. Walks the tree (respecting .gitignore, skipping target/node_modules and binaries), extracts files, markdown sections and their relations, and replaces whatever was indexed before. Returns counts including edges_unresolved — targets that named nothing real, which are kept but not pointed anywhere.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "project": project,
                    "project_root": { "type": "string", "description": "absolute path to the project on this machine" }
                },
                "required": ["project", "project_root"]
            }
        }),
        ToolName::GraphOverview => json!({
            "name": t.as_str(),
            "description": "START HERE in an unfamiliar project. Counts of files and sections, edges by kind, the most-referenced files (hubs — where to look first), files nothing points at, and the modules the project declared as its own subdivisions. Costs one call and usually replaces several rounds of listing directories.",
            "inputSchema": {
                "type": "object",
                "properties": { "project": project },
                "required": ["project"]
            }
        }),
        ToolName::GraphFind => json!({
            "name": t.as_str(),
            "description": "Full-text search across file contents, names and markdown headings. Returns nodes with an excerpt, not whole files — use it to decide what to open. Prefer this over grepping the filesystem: it searches prose and code together and returns section-level hits for documentation.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "project": project,
                    "query": { "type": "string", "description": "FTS5 syntax; a malformed query returns no results rather than an error" },
                    "limit": { "type": "integer", "description": "default 20, capped at 100" }
                },
                "required": ["project", "query"]
            }
        }),
        ToolName::GraphNode => json!({
            "name": t.as_str(),
            "description": "One node by its identity — a file path like \"src/auth.rs\", or a markdown section like \"docs/api.md#authentication\".",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "project": project,
                    "fqn": { "type": "string", "description": "the node identity, as returned by find or overview" }
                },
                "required": ["project", "fqn"]
            }
        }),
        ToolName::GraphRelated => json!({
            "name": t.as_str(),
            "description": "Everything touching a node, both directions: what it imports, links to and mentions, and what points back at it. This is the impact question — what else is involved if this changes. Each outgoing edge reports whether it resolved; an unresolved one names a target the index never found.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "project": project,
                    "fqn": { "type": "string" }
                },
                "required": ["project", "fqn"]
            }
        }),
        ToolName::GraphOutline => json!({
            "name": t.as_str(),
            "description": "A markdown file's headings in order, with excerpts. Read this before opening a long document — it is the table of contents, and usually enough to know which section to actually read.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "project": project,
                    "file_path": { "type": "string", "description": "path relative to the project root" }
                },
                "required": ["project", "file_path"]
            }
        }),
        ToolName::GraphFindings => json!({
            "name": t.as_str(),
            "description": "Specific, located observations about a project's structure: import cycles, files nothing references, files many things depend on, unusually long files, directories with no README. Each finding names the files it is about, so it can be acted on rather than merely read.\n\nThis is deliberately NOT a score. There is no number to make go up, because a single figure would hide which of these you are actually looking at, and would be computed from heuristics that can be wrong.\n\nRead `evidence` before trusting any of it: findings about dependencies are computed ONLY from imports that were resolved to a file in this project. `unresolved_imports` counts the ones that were not — third-party packages, and anything the extractor failed to follow. A project where that number dwarfs `resolved_imports` has a dependency graph too sparse for the cycle and reference findings to mean much, and they should be read as suggestions of where to look rather than conclusions. Findings are ordered most-severe first.\n\nOne kind is measured rather than guessed: `layer_violation` is an import crossing a boundary the project declared closed in its own `atlas.layers.toml`, and it is the only finding reported at `error`. Atlas never infers layers — a project that declares none gets no layer findings at all. When such a project exists, read `layer_check_incomplete` too: it says how much of the project the check could see, because an absence of violations from a check that saw half the files is not compliance.",
            "inputSchema": {
                "type": "object",
                "properties": { "project": project },
                "required": ["project"]
            }
        }),
        ToolName::GraphChanges => json!({
            "name": t.as_str(),
            "description": "What the current changes reach. Reads git for what has changed, then follows imports BACKWARDS to find every file that depends on them. Call this before editing, and before reviewing someone else's branch: it answers \"what else is involved\" without opening a single file.\n\nWith no `against`, it reports everything not yet committed — staged, unstaged and untracked. Pass a ref (`main`, a tag, a SHA) to ask what a whole branch changes instead.\n\nEach changed file carries `imported_by` (how many files import it directly) and its declared `layer` when the project declares layers; `impact` lists what was reached, with `distance` 1 meaning a direct importer. `modules_touched` and `layers_touched` cover everything involved, changed or merely reached.\n\nThere is deliberately NO risk score. A single label would hide which of several unrelated facts produced it and invite acting on the label rather than the thing.\n\nRead `evidence` before relying on the radius: it is computed from resolved imports only. `changed_not_in_graph` counts changed files the graph has never seen, whose radius is unknown rather than empty. `changed_since_indexed` counts files edited since the last index — reverse dependents survive that, because they come from other files' imports, but an import added or removed by the change itself is invisible until a reindex.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "project": project,
                    "project_root": { "type": "string", "description": "absolute path to the project on this machine" },
                    "against": { "type": "string", "description": "git ref to compare against; omit for uncommitted changes" },
                    "depth": { "type": "integer", "description": "how many hops of dependents to follow (default 3, max 10)" }
                },
                "required": ["project", "project_root"]
            }
        }),
        ToolName::GraphWatch | ToolName::GraphUnwatch | ToolName::GraphWatchStatus => {
            graph_watch_schema(t, &project)
        }
        other => unreachable!("{other:?} is not a graph tool"),
    }
}

/// The three tools that control automatic reindexing, split out
/// because the navigation schemas above already fill a screen.
pub(super) fn graph_watch_schema(t: ToolName, project: &Value) -> Value {
    match t {
        ToolName::GraphWatch => json!({
            "name": t.as_str(),
            "description": "Index a project and keep it indexed: reindexes automatically when files change, so the map does not go stale between questions. Prefer this over graph.reindex when the project is being worked on — a stale map is worse than none, because an agent trusts what it reads. The initial index completes before this returns. Changes under target/, node_modules/ and .git/ are ignored, so a build does not trigger a reindex storm. Watching is held in memory only: a server restart watches nothing until asked again.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "project": project,
                    "project_root": { "type": "string", "description": "absolute path to the project on this machine" }
                },
                "required": ["project", "project_root"]
            }
        }),
        ToolName::GraphUnwatch => json!({
            "name": t.as_str(),
            "description": "Stop watching a project. The graph stays as it was, it simply stops updating. Not an error if the project was not being watched.",
            "inputSchema": {
                "type": "object",
                "properties": { "project": project },
                "required": ["project"]
            }
        }),
        ToolName::GraphWatchStatus => json!({
            "name": t.as_str(),
            "description": "What is being watched, with how many automatic reindexes each has run and the outcome of the last one. A last_error that persists means the map is drifting from the disk.",
            "inputSchema": { "type": "object", "properties": {}, "required": [] }
        }),
        other => unreachable!("{other:?} is not a graph tool"),
    }
}
