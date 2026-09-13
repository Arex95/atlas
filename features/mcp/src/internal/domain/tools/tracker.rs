//! Schemas for the `tracker.*` tools.
//!
//! Split by family so adding a tool touches one small file
//! instead of a 900-line one every branch also edits.

use serde_json::{Value, json};

use super::ToolName;

pub(super) fn tracker_schema(t: ToolName) -> Value {
    match t {
        ToolName::TrackerListIssues => json!({
            "name": t.as_str(),
            "description": "List issues in the given project. Reads from the local mirror when active, otherwise from the underlying tracker.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "project": { "type": "string", "description": "owner/repo" },
                    "status":  { "type": "string", "enum": ["open", "closed"] },
                    "labels":  { "type": "array", "items": { "type": "string" } },
                    "updated_after": { "type": "string", "format": "date-time" },
                    "milestone": { "type": "string", "description": "milestone title — the tracker's own name for a roadmap" }
                },
                "required": ["project"]
            }
        }),
        ToolName::TrackerGetIssue => json!({
            "name": t.as_str(),
            "description": "Fetch a single issue by its tracker id.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "project": { "type": "string", "description": "owner/repo" },
                    "id":      { "type": "string" }
                },
                "required": ["project", "id"]
            }
        }),
        ToolName::TrackerListRelations => json!({
            "name": t.as_str(),
            "description": "List issue relations (blocks / blocked_by / relates_to) for the given issue.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "project": { "type": "string", "description": "owner/repo" },
                    "id":      { "type": "string" }
                },
                "required": ["project", "id"]
            }
        }),
        ToolName::TrackerCreateIssue => json!({
            "name": t.as_str(),
            "description": "Create a new issue in the given project. Always writes to the real tracker; the mirror (if active) is refreshed immediately.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "project":     { "type": "string", "description": "owner/repo" },
                    "title":       { "type": "string" },
                    "description": { "type": "string" },
                    "labels":      { "type": "array", "items": { "type": "string" } }
                },
                "required": ["project", "title"]
            }
        }),
        ToolName::TrackerUpdateStatus => json!({
            "name": t.as_str(),
            "description": "Change an issue's status to open or closed. Always writes to the real tracker; the mirror (if active) is refreshed immediately.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "project": { "type": "string", "description": "owner/repo" },
                    "id":      { "type": "string" },
                    "status":  { "type": "string", "enum": ["open", "closed"] }
                },
                "required": ["project", "id", "status"]
            }
        }),
        ToolName::TrackerCloseIssue => json!({
            "name": t.as_str(),
            "description": "Close an issue. Sugar over tracker.update_status with status=closed.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "project": { "type": "string", "description": "owner/repo" },
                    "id":      { "type": "string" }
                },
                "required": ["project", "id"]
            }
        }),
        ToolName::TrackerPlanProgress => json!({
            "name": t.as_str(),
            "description": "Count acceptance-criteria checkboxes (- [ ] / - [x]) under a '## Acceptance criteria' heading across issues matching the filter. Not issue count — raw criteria, ticked over total.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "project": { "type": "string", "description": "owner/repo" },
                    "status":  { "type": "string", "enum": ["open", "closed"] },
                    "labels":  { "type": "array", "items": { "type": "string" } },
                    "updated_after": { "type": "string", "format": "date-time" },
                    "milestone": { "type": "string", "description": "milestone title — the tracker's own name for a roadmap" }
                },
                "required": ["project"]
            }
        }),
        other => unreachable!("{other:?} is not a tracker tool"),
    }
}
