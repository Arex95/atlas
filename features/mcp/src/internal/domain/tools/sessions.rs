//! Schemas for the `sessions.*` tools.
//!
//! Split by family so adding a tool touches one small file
//! instead of a 900-line one every branch also edits.

use serde_json::{Value, json};

use super::ToolName;

pub(super) fn sessions_schema(t: ToolName) -> Value {
    match t {
        ToolName::SessionsCreate => json!({
            "name": t.as_str(),
            "description": "Register a new session — metadata only, no PTY spawned.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "project":       { "type": "string", "description": "owner/repo" },
                    "remote_url":    { "type": "string", "description": "git remote URL" },
                    "branch":        { "type": "string" },
                    "relative_path": { "type": "string", "description": "path relative to this machine's workspace_root" },
                    "agent_kind":    { "type": "string", "description": "defaults to \"bash\"" },
                    "title":         { "type": "string" }
                },
                "required": ["project", "remote_url", "branch", "relative_path"]
            }
        }),
        ToolName::SessionsList => json!({
            "name": t.as_str(),
            "description": "List sessions registered for a project, scoped to one owner.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "project":  { "type": "string", "description": "owner/repo" },
                    "status":   { "type": "string", "enum": ["active", "archived"] }
                },
                "required": ["project"]
            }
        }),
        ToolName::SessionsGet => json!({
            "name": t.as_str(),
            "description": "Fetch a single session by its id, scoped to one owner. A session owned by someone else reports the same not-found error as an unknown id.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "id":       { "type": "string" },
                },
                "required": ["id"]
            }
        }),
        ToolName::SessionsSetResumeCommand => json!({
            "name": t.as_str(),
            "description": "Record what to run in this session's terminal each time one is spawned, so an agent CLI can rehydrate its own context. Atlas does not persist terminal scrollback — replaying a buffer would give a human something to read and give the model nothing — so the CLI is asked to restore itself instead: `claude --resume <id>`, or the equivalent.\n\nSet it after the first terminal, not at creation: a CLI usually names its own session id only once it has started. Omit `command` to clear it and go back to a bare shell.\n\nThe command is typed into the shell exactly as you would have typed it. Atlas does not interpret it or check whether it worked. It runs only on a real spawn, never on the idempotent repeat, so polling `terminal.spawn` will not re-run it.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "id": { "type": "string" },
                    "command": { "type": "string", "description": "omit to clear" }
                },
                "required": ["id"]
            }
        }),
        ToolName::SessionsUpdateStatus => json!({
            "name": t.as_str(),
            "description": "Change a session's status to active or archived. Scoped to one owner — updating another owner's session reports not-found.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "id":       { "type": "string" },
                    "status":   { "type": "string", "enum": ["active", "archived"] }
                },
                "required": ["id", "status"]
            }
        }),
        other => unreachable!("{other:?} is not a sessions tool"),
    }
}
