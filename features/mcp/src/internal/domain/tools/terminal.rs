//! Schemas for the `terminal.*` tools.
//!
//! Split by family so adding a tool touches one small file
//! instead of a 900-line one every branch also edits.

use serde_json::{Value, json};

use super::ToolName;

pub(super) fn terminal_schema(t: ToolName) -> Value {
    match t {
        ToolName::TerminalSpawn => json!({
            "name": t.as_str(),
            "description": "Spawn a real shell process bound to a registered session, resolved against this machine's workspace_root. Spawning an already-running session returns the existing process, not an error.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "session_id": { "type": "string" }
                },
                "required": ["session_id"]
            }
        }),
        ToolName::TerminalWrite => json!({
            "name": t.as_str(),
            "description": "Write raw input to a running PTY's stdin.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "session_id": { "type": "string" },
                    "input":      { "type": "string" }
                },
                "required": ["session_id", "input"]
            }
        }),
        ToolName::TerminalReadOutput => json!({
            "name": t.as_str(),
            "description": "Read output appended since 'since_offset' (default 0). Poll-based, not push — pass the returned next_offset back on the next call to page forward.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "session_id":   { "type": "string" },
                    "since_offset": { "type": "integer", "description": "byte offset from a previous read_output; default 0" }
                },
                "required": ["session_id"]
            }
        }),
        ToolName::TerminalClose => json!({
            "name": t.as_str(),
            "description": "Kill the PTY process bound to this session and remove it from the pool.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "session_id": { "type": "string" }
                },
                "required": ["session_id"]
            }
        }),
        ToolName::TerminalRestore => json!({
            "name": t.as_str(),
            "description": "Make the session's workspace exist on this machine, cloning from its remote_url/branch if the resolved path is missing. A no-op if the path already exists. Does not resolve missing git credentials, only reports them distinctly (kind: credentials_missing) so a human can fix them.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "session_id": { "type": "string" }
                },
                "required": ["session_id"]
            }
        }),
        other => unreachable!("{other:?} is not a terminal tool"),
    }
}
