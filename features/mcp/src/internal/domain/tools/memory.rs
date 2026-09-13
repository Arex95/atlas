//! Schemas for the `memory.*` tools.
//!
//! Split by family so adding a tool touches one small file
//! instead of a 900-line one every branch also edits.

use serde_json::{Value, json};

use super::ToolName;

pub(super) fn memory_schema(t: ToolName) -> Value {
    const SCOPE_NOTE: &str = "\"project\" is memory about a project, shared with everyone on it; \"personal\" is memory about one developer, private to them. Supply `project` for the first; the second takes no owner — personal scope always means YOUR OWN bucket, identified by the token this request authenticated with. There is no way to address another developer's personal memory, deliberately.";

    match t {
        ToolName::MemoryRemember => json!({
            "name": t.as_str(),
            "description": format!("Store a value the agent should remember, replacing whatever was under that key. {SCOPE_NOTE}"),
            "inputSchema": {
                "type": "object",
                "properties": {
                    "scope":    { "type": "string", "enum": ["project", "personal"] },
                    "project":  { "type": "string", "description": "required when scope is \"project\"" },
                    "key":      { "type": "string" },
                    "value":    { "description": "arbitrary JSON" }
                },
                "required": ["scope", "key", "value"]
            }
        }),
        ToolName::MemoryRecall => json!({
            "name": t.as_str(),
            "description": format!("Read back one remembered value by key. {SCOPE_NOTE}"),
            "inputSchema": {
                "type": "object",
                "properties": {
                    "scope":    { "type": "string", "enum": ["project", "personal"] },
                    "project":  { "type": "string", "description": "required when scope is \"project\"" },
                    "key":      { "type": "string" }
                },
                "required": ["scope", "key"]
            }
        }),
        ToolName::MemoryList => json!({
            "name": t.as_str(),
            "description": format!("List everything remembered in one bucket, by key. There is deliberately no call that returns both buckets at once. {SCOPE_NOTE}"),
            "inputSchema": {
                "type": "object",
                "properties": {
                    "scope":    { "type": "string", "enum": ["project", "personal"] },
                    "project":  { "type": "string", "description": "required when scope is \"project\"" },
                },
                "required": ["scope"]
            }
        }),
        ToolName::MemoryForget => json!({
            "name": t.as_str(),
            "description": format!("Delete one remembered value by key. Errors if there was nothing there. {SCOPE_NOTE}"),
            "inputSchema": {
                "type": "object",
                "properties": {
                    "scope":    { "type": "string", "enum": ["project", "personal"] },
                    "project":  { "type": "string", "description": "required when scope is \"project\"" },
                    "key":      { "type": "string" }
                },
                "required": ["scope", "key"]
            }
        }),
        other => unreachable!("{other:?} is not a memory tool"),
    }
}
