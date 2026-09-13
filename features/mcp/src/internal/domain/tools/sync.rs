//! Schemas for the `sync.*` tools.
//!
//! Split by family so adding a tool touches one small file
//! instead of a 900-line one every branch also edits.

use serde_json::{Value, json};

use super::ToolName;

pub(super) fn sync_schema(t: ToolName) -> Value {
    match t {
        ToolName::SyncSessionsNow => json!({
            "name": t.as_str(),
            "description": "Focus-mode sync: push every locally-owned session to a team server, then pull back anything newer for that owner. Last-write-wins by updated_at. No background loop — this runs once, on request.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "remote_url": { "type": "string", "description": "base URL of the team atlas-server, e.g. https://atlas.example.internal" },
                    "bearer_token": { "type": "string", "description": "an atlas-auth session token for the remote server (from /api/auth/login there)" },
                },
                "required": ["remote_url", "bearer_token"]
            }
        }),
        ToolName::SyncMemoryNow => json!({
            "name": t.as_str(),
            "description": "Run one push-then-pull pass of agent memory against a remote atlas-server. Project memory replicates freely; personal memory only under the authenticated owner — the remote forces that regardless of what is sent, so this cannot write into another developer's memory.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "remote_url":   { "type": "string", "description": "base URL of the remote's sync surface, e.g. https://atlas.example.com/api/sync/" },
                    "bearer_token": { "type": "string", "description": "an atlas-auth session token for the remote server" },
                },
                "required": ["remote_url", "bearer_token"]
            }
        }),
        ToolName::SyncNotesNow => json!({
            "name": t.as_str(),
            "description": "One push-then-pull pass of your notes against a remote Atlas. Notes are personal throughout, so both legs are stored under you — by this client and by the remote independently — and a remote that is wrong or compromised cannot make this machine file another developer's notes as your own.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "remote_url": { "type": "string", "description": "base URL of the remote server's sync API" },
                    "bearer_token": { "type": "string", "description": "your login token on that server, not the shared MCP token" }
                },
                "required": ["remote_url", "bearer_token"]
            }
        }),
        ToolName::SyncSetMode => json!({
            "name": t.as_str(),
            "description": "Switch the sync propagation mode. \"focus\" stops any running background loop (the default — sync only on an explicit ask). \"auto\" starts a background loop calling the same push-then-pull pass on a fixed interval. \"live\" instead holds an SSE stream open to the remote and runs a pass the moment it reports a change, reconnecting on its own if the connection drops — no interval to configure. Calling this again with a new config replaces the running loop rather than adding a second one. The config is held only in this process's memory — it does not survive a restart.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "mode":          { "type": "string", "enum": ["focus", "auto", "live"] },
                    "remote_url":    { "type": "string", "description": "required when mode is \"auto\" or \"live\"" },
                    "bearer_token":  { "type": "string", "description": "required when mode is \"auto\" or \"live\"" },
                    "interval_secs": { "type": "integer", "description": "required when mode is \"auto\"; \"live\" has no interval" }
                },
                "required": ["mode"]
            }
        }),
        ToolName::SyncStatus => json!({
            "name": t.as_str(),
            "description": "Current sync mode plus the outcome of the most recent pass (pushed/pulled counts, or the last error if the most recent attempt failed). In \"live\" mode also reports stream_connected, so a silently dropped connection is distinguishable from a quiet team.",
            "inputSchema": {
                "type": "object",
                "properties": {},
                "required": []
            }
        }),
        other => unreachable!("{other:?} is not a sync tool"),
    }
}
