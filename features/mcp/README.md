# atlas-mcp

Exposes the tracker port from `atlas-tracker` to AI agents over the
Model Context Protocol (MCP) as JSON-RPC 2.0 tools. The crate ships
an `axum::Router` and a state builder; the binary mounts it under
`/api/mcp`, terminates HTTP, and holds the auth token.

## Protocol shape

- Transport: HTTP POST, one request → one response. Notifications
  (JSON-RPC requests without `id`) get HTTP 204. Batches are
  refused with `-32600` (a later issue when a caller demands them).
- Protocol version: `2025-06-18`. Clients that request a newer
  version get this one back and continue on its shape.

## Methods

| Method | Purpose |
|---|---|
| `initialize` | Server info + declared capabilities (only `tools`). |
| `tools/list` | The three tools below with input JSON Schemas. |
| `tools/call` | Dispatch by tool name. |

Anything else → `-32601 method not found`.

## Tools

| Name | Params | Returns |
|---|---|---|
| `tracker.list_issues` | `{ project, status?, labels?, updated_after? }` | array of issues |
| `tracker.get_issue` | `{ project, id }` | issue |
| `tracker.list_relations` | `{ project, id }` | array of relations |

`project` is `owner/repo`; anything else fails with `-32602`. Issue
fields on the wire are the tracker-agnostic domain shape from
`atlas-tracker` — no vendor fields.

## Error mapping

`TrackerError` never surfaces raw; it maps to JSON-RPC codes:

| Source | Code | `data.kind` |
|---|---|---|
| `TrackerError::NotFound` | `-32000` | `not_found` |
| `TrackerError::Unauthorized` | `-32001` | `unauthorized` |
| `TrackerError::RateLimited` | `-32002` | `rate_limited` (+ `retry_after_secs`) |
| `TrackerError::Disabled` | `-32003` | `disabled` |
| `TrackerError::Transport` / `Malformed` | `-32603` internal | — |

## Configuration

Read by the binary, not this crate.

| Variable | Required when | Purpose |
|---|---|---|
| `ATLAS_MCP_TOKEN` | Always in production | Bearer token compared with `subtle::ConstantTimeEq`. Startup fails if unset. |
| `ATLAS_MCP_TOKEN_ALLOW_UNSET` | Dev only | `=1` boots the server with a placeholder token and a `WARN` line. Never enable in production. |

`ATLAS_LISTEN_ADDR` (default `0.0.0.0:4000`) is a binary-level env,
not scoped to this crate.

## Scope of this crate today

- Read-only: the three tracker tools reflect the read-only state
  of the port after `#4`. Writes land under a later issue.
- No SSE / Streamable HTTP: one POST → one response.
- No auth beyond the shared Bearer token — per-user OAuth is a
  Mode 2 concern in a different issue.
- No `resources` / `prompts` MCP primitives yet — only `tools`.
