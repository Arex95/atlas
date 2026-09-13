# Connecting an agent CLI to Atlas

Atlas exposes its tracker read tools over MCP (JSON-RPC 2.0, protocol
`2025-06-18`) at `POST /api/mcp`. A single bearer token — `ATLAS_MCP_TOKEN`
— gates every call to that endpoint; anyone who holds it can invoke
every tool the server exposes. There is no per-tool or per-project
scoping yet, so treat it as you would any other credential: never in a
repository, never in a chat message, and never in a build argument —
those end up in the image history, readable by anyone who can pull it.

Run `just connect-agent` (or `scripts/mcp-connect.sh`) against a
running instance to get the exact snippet for your CLI, filled in with
the resolved URL and token. Add `--redact` before pasting the output
anywhere outside your own machine.

## Claude Code

Tested with Claude Code CLI, docs: <https://docs.claude.com/en/docs/claude-code/mcp>.

```bash
claude mcp add --transport http atlas http://127.0.0.1:4000/api/mcp \
  --header "Authorization: Bearer $ATLAS_MCP_TOKEN"
```

Probe: ask the agent *"list my open issues in your-org/your-project"*. It
should call `tracker.list_issues` and render the result.

If it fails:
- **401 / auth error** — token mismatch. Confirm `ATLAS_MCP_TOKEN` is
  identical in the server's environment and in the header Claude Code
  sent (`claude mcp list` shows the registered header).
- **Connection refused / timeout** — wrong URL, or `atlas-server` is
  not listening on that address. Check with `curl http://.../health`.
- **Tool call returns an empty list with no error** — the tracker is
  configured but the mirror has not synced yet, or `project` does not
  match `owner/repo` exactly.

## Codex CLI

Tested with Codex CLI, docs: <https://developers.openai.com/codex/cli>.

Add to `~/.codex/config.toml`:

```toml
[mcp_servers.atlas]
url = "http://127.0.0.1:4000/api/mcp"
bearer_token = "..."
```

Probe: same question, *"list my open issues in your-org/your-project"*.

If it fails, same three checks as above — auth, URL/reachability,
tracker state (`ATLAS_TRACKER_KIND` unset or `disabled` means every
tool call succeeds but returns nothing).

## Cursor

Tested with Cursor, docs: <https://docs.cursor.com/context/mcp>.

Add to `.cursor/mcp.json` (project-scoped) or `~/.cursor/mcp.json`
(global):

```json
{
  "mcpServers": {
    "atlas": {
      "url": "http://127.0.0.1:4000/api/mcp",
      "headers": {
        "Authorization": "Bearer ..."
      }
    }
  }
}
```

Probe: same question. Cursor surfaces MCP tool-call failures in its
own output panel rather than the chat — check there first.

## Running on a host you own (Mode 2)

Everything above assumes `atlas-server` on `localhost`. Pointed at a
host you run instead — a box in the office, a NAS, a VPS, a machine
under a desk:

- Swap `127.0.0.1:4000` for that host's address or hostname in every
  snippet above.
- Terminate TLS at a reverse proxy (Caddy, nginx, Traefik) in front of
  `atlas-server` — the server itself speaks plain HTTP. Without TLS,
  `ATLAS_MCP_TOKEN` and every tracker payload cross the network in the
  clear.
- The bearer token is transport-security-critical: it is the only
  thing standing between "on this network" and "can call every MCP
  tool". Do not expose the port directly to the internet; put it
  behind the proxy and, if reachable outside your LAN, behind
  authentication at that layer too.
