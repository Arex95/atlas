//! Schemas for the `afg.*` tools.
//!
//! Split by family so adding a tool touches one small file
//! instead of a 900-line one every branch also edits.

use serde_json::{Value, json};

use super::ToolName;

pub(super) fn afg_schema(t: ToolName) -> Value {
    match t {
        ToolName::AfgRegisterWorkflow => json!({
            "name": t.as_str(),
            "description": "Register (or update, by name) a declarative workflow for a project. Provide either source_path (relative to project_root, read from disk) or yaml (inline body) — not both.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "project":      { "type": "string", "description": "owner/repo" },
                    "project_root": { "type": "string", "description": "absolute path used to resolve source_path and as the shell-gate default cwd" },
                    "source_path":  { "type": "string", "description": "path to the workflow YAML, relative to project_root" },
                    "yaml":         { "type": "string", "description": "inline YAML body; wins over source_path if both are given" }
                },
                "required": ["project", "project_root"]
            }
        }),
        ToolName::AfgStartRun => json!({
            "name": t.as_str(),
            "description": "Start a new run of a registered workflow: creates the run, dispatches the first runnable node as a 'task' message.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "workflow_id":       { "type": "string" },
                    "session_id":        { "type": "string", "description": "the initiating session; also the default dispatch target" },
                    "target_session_id": { "type": "string", "description": "overrides which session receives the task, if different from session_id" }
                },
                "required": ["workflow_id", "session_id"]
            }
        }),
        ToolName::AfgSubmitTaskResult => json!({
            "name": t.as_str(),
            "description": "Report the result of the run's current node. Runs the node's acceptance-criteria gates in order; on pass advances to the next node or completes the run, on failure retries with injected feedback (up to the node's max_retries) or fails the run.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "run_id":       { "type": "string" },
                    "node_id":      { "type": "string" },
                    "project_root": { "type": "string", "description": "absolute path used as the shell gate's default cwd" },
                    "payload":      { "description": "arbitrary JSON the agent is reporting back; schema-validate gates validate this" }
                },
                "required": ["run_id", "node_id", "project_root"]
            }
        }),
        ToolName::AfgGetRun => json!({
            "name": t.as_str(),
            "description": "Fetch a run's status plus its full node-event timeline, oldest first — a point-in-time snapshot. To watch a run as it advances instead, a human-facing client streams GET /api/afg/runs/{run_id}/events (SSE, atlas-auth bearer), which replays this same timeline and then continues live.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "run_id": { "type": "string" }
                },
                "required": ["run_id"]
            }
        }),
        ToolName::AfgListRuns => json!({
            "name": t.as_str(),
            "description": "List runs for a workflow, optionally filtered by status.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "workflow_id": { "type": "string" },
                    "status":      { "type": "string", "enum": ["pending", "running", "failed", "completed"] }
                },
                "required": ["workflow_id"]
            }
        }),
        other => unreachable!("{other:?} is not an afg tool"),
    }
}
