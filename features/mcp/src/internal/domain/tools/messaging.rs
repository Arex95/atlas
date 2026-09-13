//! Schemas for the `messaging.*` tools.
//!
//! Split by family so adding a tool touches one small file
//! instead of a 900-line one every branch also edits.

use serde_json::{Value, json};

use super::ToolName;

pub(super) fn messaging_schema(t: ToolName) -> Value {
    match t {
        ToolName::MessagingSendMessage => json!({
            "name": t.as_str(),
            "description": "Send a coordination-protocol message. Omit 'to' to broadcast to every session working on the project; set it to address one session directly.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "project":        { "type": "string", "description": "owner/repo" },
                    "from":           { "type": "string", "description": "the sending session's own identifier" },
                    "to":             { "type": "string", "description": "omit to broadcast; set to address one session directly" },
                    "type":           { "type": "string", "description": "free-form message kind, defaults to \"message\"" },
                    "payload":        { "description": "arbitrary JSON body" },
                    "correlation_id": { "type": "string" },
                    "reply_to":       { "type": "string", "description": "id of the message this replies to" }
                },
                "required": ["project", "from", "payload"]
            }
        }),
        ToolName::MessagingReadInbox => json!({
            "name": t.as_str(),
            "description": "Read every broadcast plus every direct message addressed to 'for' in the given project, oldest first. Poll-based — pass the last seen message id as 'since' to page forward.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "project": { "type": "string", "description": "owner/repo" },
                    "for":     { "type": "string", "description": "the reading session's own identifier" },
                    "since":   { "type": "string", "description": "a previously-seen message id; excludes it and everything before it" },
                    "limit":   { "type": "integer", "description": "default 50, capped at 200" }
                },
                "required": ["project", "for"]
            }
        }),
        other => unreachable!("{other:?} is not a messaging tool"),
    }
}
