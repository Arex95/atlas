//! Project Map tools — how an agent navigates a repository without
//! reading it.
//!
//! The intended order is `overview` to see the shape, `find` to
//! locate something, then `node` / `related` / `outline` to move
//! around from there. The tool descriptions say so, because an agent
//! that reaches for `find` first tends to grep-by-another-name
//! instead of using the structure.

use atlas_graph::api::{
    Analyser, GraphNode, GraphStore, GraphWatcher, IMPACT_DEFAULT_DEPTH, ImpactAnalyser, Indexer,
    changed_files,
};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::internal::infrastructure::jsonrpc::{JsonRpcError, code, graph_error_to_jsonrpc};

/// Enough hits to be useful, few enough not to flood a context window
/// — which is the whole reason this exists instead of grep.
const DEFAULT_FIND_LIMIT: i64 = 20;
const MAX_FIND_LIMIT: i64 = 100;

#[derive(Deserialize)]
struct ReindexParams {
    project: String,
    project_root: String,
}

#[derive(Deserialize)]
struct ProjectParams {
    project: String,
}

#[derive(Deserialize)]
struct FindParams {
    project: String,
    query: String,
    #[serde(default)]
    limit: Option<i64>,
}

#[derive(Deserialize)]
struct FqnParams {
    project: String,
    fqn: String,
}

#[derive(Deserialize)]
struct OutlineParams {
    project: String,
    file_path: String,
}

fn invalid_params(msg: impl Into<String>) -> JsonRpcError {
    JsonRpcError::new(code::INVALID_PARAMS, msg)
}

pub async fn reindex(store: &GraphStore, params: Value) -> Result<Value, JsonRpcError> {
    let params: ReindexParams =
        serde_json::from_value(params).map_err(|e| invalid_params(format!("bad params: {e}")))?;

    let stats = Indexer::new(store.clone())
        .reindex(&params.project, std::path::Path::new(&params.project_root))
        .await
        .map_err(|e| graph_error_to_jsonrpc(&e))?;

    Ok(serde_json::to_value(stats).unwrap_or(Value::Null))
}

pub async fn overview(store: &GraphStore, params: Value) -> Result<Value, JsonRpcError> {
    let params: ProjectParams =
        serde_json::from_value(params).map_err(|e| invalid_params(format!("bad params: {e}")))?;

    let overview = store
        .overview(&params.project)
        .await
        .map_err(|e| graph_error_to_jsonrpc(&e))?;

    Ok(json!({
        "files": overview.files,
        "sections": overview.sections,
        "edges_by_predicate": overview.edges_by_predicate.iter()
            .map(|(p, n)| json!({ "predicate": p, "count": n }))
            .collect::<Vec<_>>(),
        "edges_unresolved": overview.edges_unresolved,
        "hubs": overview.hubs.iter()
            .map(|(fqn, n)| json!({ "fqn": fqn, "referenced_by": n }))
            .collect::<Vec<_>>(),
        "orphan_files": overview.orphan_files,
        // What the project declared as its own subdivisions, so an
        // agent can scope its next question instead of asking about
        // the whole repository. Empty means the project declared none.
        "modules": overview.modules.iter().map(|m| json!({
            "name": m.name,
            "path": m.path,
            "description": m.description,
        })).collect::<Vec<_>>(),
    }))
}

pub async fn find(store: &GraphStore, params: Value) -> Result<Value, JsonRpcError> {
    let params: FindParams =
        serde_json::from_value(params).map_err(|e| invalid_params(format!("bad params: {e}")))?;

    let limit = params
        .limit
        .unwrap_or(DEFAULT_FIND_LIMIT)
        .clamp(1, MAX_FIND_LIMIT);

    let hits = store
        .find(&params.project, &params.query, limit)
        .await
        .map_err(|e| graph_error_to_jsonrpc(&e))?;

    Ok(json!({ "results": hits.iter().map(summary).collect::<Vec<_>>() }))
}

pub async fn node(store: &GraphStore, params: Value) -> Result<Value, JsonRpcError> {
    let params: FqnParams =
        serde_json::from_value(params).map_err(|e| invalid_params(format!("bad params: {e}")))?;

    let node = store
        .node(&params.project, &params.fqn)
        .await
        .map_err(|e| graph_error_to_jsonrpc(&e))?;

    Ok(serde_json::to_value(&node).unwrap_or(Value::Null))
}

pub async fn related(store: &GraphStore, params: Value) -> Result<Value, JsonRpcError> {
    let params: FqnParams =
        serde_json::from_value(params).map_err(|e| invalid_params(format!("bad params: {e}")))?;

    let related = store
        .related(&params.project, &params.fqn)
        .await
        .map_err(|e| graph_error_to_jsonrpc(&e))?;

    Ok(json!({
        "node": summary(&related.node),
        "outgoing": related.outgoing.iter().map(|e| json!({
            "predicate": e.predicate.as_str(),
            "target": e.dst_fqn,
            // An agent needs to know an edge points at something that
            // was never found, rather than following it and getting a
            // not-found it cannot interpret.
            "resolved": e.dst_id.is_some(),
            "line": e.line,
        })).collect::<Vec<_>>(),
        "incoming": related.incoming.iter().map(|e| json!({
            "predicate": e.predicate.as_str(),
            "from_file": e.file_path,
            "line": e.line,
        })).collect::<Vec<_>>(),
    }))
}

pub async fn outline(store: &GraphStore, params: Value) -> Result<Value, JsonRpcError> {
    let params: OutlineParams =
        serde_json::from_value(params).map_err(|e| invalid_params(format!("bad params: {e}")))?;

    let sections = store
        .outline(&params.project, &params.file_path)
        .await
        .map_err(|e| graph_error_to_jsonrpc(&e))?;

    Ok(json!({
        "sections": sections.iter().map(|s| json!({
            "fqn": s.fqn,
            "name": s.name,
            "start_line": s.start_line,
            "end_line": s.end_line,
            "excerpt": s.excerpt,
        })).collect::<Vec<_>>(),
    }))
}

pub async fn findings(store: &GraphStore, params: Value) -> Result<Value, JsonRpcError> {
    let params: ProjectParams =
        serde_json::from_value(params).map_err(|e| invalid_params(format!("bad params: {e}")))?;

    let report = Analyser::new(store.clone())
        .analyse(&params.project)
        .await
        .map_err(|e| graph_error_to_jsonrpc(&e))?;

    Ok(json!({
        "findings": report.findings.iter().map(|f| json!({
            "kind": f.kind,
            "severity": f.severity,
            // Prose naming the places involved — a finding an agent
            // cannot locate is one it reports to a human and moves on
            // from. `nodes` carries the same places machine-readably.
            "detail": f.detail,
            "nodes": f.nodes,
        })).collect::<Vec<_>>(),
        // Deliberately alongside the findings rather than behind
        // another call: an agent that has to ask a second time for
        // what the first answer was computed from will not ask.
        "evidence": {
            "files": report.evidence.files,
            "resolved_imports": report.evidence.resolved_imports,
            "unresolved_imports": report.evidence.unresolved_imports,
        },
    }))
}

#[derive(Deserialize)]
struct ChangesParams {
    project: String,
    project_root: String,
    /// A git ref to compare against. Omitted means the working tree:
    /// everything not yet committed.
    #[serde(default)]
    against: Option<String>,
    #[serde(default)]
    depth: Option<usize>,
}

pub async fn changes(store: &GraphStore, params: Value) -> Result<Value, JsonRpcError> {
    let params: ChangesParams =
        serde_json::from_value(params).map_err(|e| invalid_params(format!("bad params: {e}")))?;

    let root = std::path::Path::new(&params.project_root);
    let changed =
        changed_files(root, params.against.as_deref()).map_err(|e| graph_error_to_jsonrpc(&e))?;

    let report = ImpactAnalyser::new(store.clone())
        .analyse(
            &params.project,
            root,
            &changed,
            params.depth.unwrap_or(IMPACT_DEFAULT_DEPTH),
        )
        .await
        .map_err(|e| graph_error_to_jsonrpc(&e))?;

    Ok(json!({
        "changed": report.changed.iter().map(|c| json!({
            "path": c.path,
            "kind": c.kind,
            "renamed_from": c.renamed_from,
            // A file the graph has never seen has no radius, and an
            // empty one would read as "this change reaches nothing".
            "in_graph": c.in_graph,
            "layer": c.layer,
            "imported_by": c.imported_by,
        })).collect::<Vec<_>>(),
        "impact": report.impact.iter().map(|i| json!({
            "path": i.path,
            "distance": i.distance,
        })).collect::<Vec<_>>(),
        "modules_touched": report.modules_touched,
        "layers_touched": report.layers_touched,
        "depth": report.depth,
        "evidence": {
            "resolved_imports": report.evidence.resolved_imports,
            "unresolved_imports": report.evidence.unresolved_imports,
            "changed_not_in_graph": report.evidence.changed_not_in_graph,
            "changed_since_indexed": report.evidence.changed_since_indexed,
        },
    }))
}

/// A node without its full search text — the excerpt is there to help
/// an agent decide whether to open the file, not to substitute for
/// reading it.
fn summary(node: &GraphNode) -> Value {
    json!({
        "fqn": node.fqn,
        "name": node.name,
        "kind": node.kind,
        "file_path": node.file_path,
        "start_line": node.start_line,
        "end_line": node.end_line,
        "excerpt": node.excerpt,
    })
}

#[derive(Deserialize)]
struct WatchParams {
    project: String,
    project_root: String,
}

pub async fn watch(watcher: &GraphWatcher, params: Value) -> Result<Value, JsonRpcError> {
    let params: WatchParams =
        serde_json::from_value(params).map_err(|e| invalid_params(format!("bad params: {e}")))?;

    let stats = watcher
        .watch(&params.project, std::path::Path::new(&params.project_root))
        .await
        .map_err(|e| graph_error_to_jsonrpc(&e))?;

    Ok(json!({ "watching": true, "initial_index": stats }))
}

pub async fn unwatch(watcher: &GraphWatcher, params: Value) -> Result<Value, JsonRpcError> {
    let params: ProjectParams =
        serde_json::from_value(params).map_err(|e| invalid_params(format!("bad params: {e}")))?;

    // Not an error when nothing was watched: a caller cleaning up
    // should not have to check first.
    Ok(json!({ "was_watching": watcher.unwatch(&params.project).await }))
}

pub async fn watch_status(watcher: &GraphWatcher) -> Value {
    json!({
        "watching": watcher.status().await.iter().map(|w| json!({
            "project": w.project,
            "root": w.root,
            "reindexes": w.reindexes,
            "last_index": w.last_stats,
            "last_error": w.last_error,
        })).collect::<Vec<_>>(),
    })
}
