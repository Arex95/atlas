//! Schemas for the `notes.*` tools.
//!
//! Split by family so adding a tool touches one small file
//! instead of a 900-line one every branch also edits.

use serde_json::{Value, json};

use super::ToolName;

/// The personal-note tools.
///
/// None of them takes an owner or a scope. A note is personal by
/// construction — there is no shared variant — so the only thing a
/// caller could name is somebody else.
pub(super) fn notes_schema(t: ToolName) -> Value {
    const NOTE: &str = "A note of your own: personal, private to you, and not shared with anyone on any project. Addressed by a name you choose, so writing the same name twice rewrites one note rather than making two. A scratchpad is simply the note called \"scratchpad\".";

    match t {
        ToolName::NotesWrite => json!({
            "name": t.as_str(),
            "description": format!("Write a note, replacing whatever was under that name. {NOTE}"),
            "inputSchema": {
                "type": "object",
                "properties": {
                    "name": { "type": "string", "description": "what you address it by; at most 200 characters" },
                    "body": { "type": "string", "description": "may be empty, which clears a note without deleting it" }
                },
                "required": ["name", "body"]
            }
        }),
        ToolName::NotesRead => json!({
            "name": t.as_str(),
            "description": format!("Read one note by name. {NOTE}"),
            "inputSchema": {
                "type": "object",
                "properties": { "name": { "type": "string" } },
                "required": ["name"]
            }
        }),
        ToolName::NotesList => json!({
            "name": t.as_str(),
            "description": format!("Every note you have, most recently written first, bodies included. {NOTE}"),
            "inputSchema": { "type": "object", "properties": {}, "required": [] }
        }),
        ToolName::NotesDelete => json!({
            "name": t.as_str(),
            "description": format!("Delete one note by name. Errors if there was nothing there, because the usual cause is a typo. {NOTE}"),
            "inputSchema": {
                "type": "object",
                "properties": { "name": { "type": "string" } },
                "required": ["name"]
            }
        }),
        other => unreachable!("{other:?} is not a notes tool"),
    }
}
