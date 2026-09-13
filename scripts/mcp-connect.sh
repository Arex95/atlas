#!/bin/sh
# Prints ready-to-paste MCP configuration for connecting an agent CLI
# (Claude Code, Codex CLI, Cursor) to a running atlas-server instance.
# Never edits the CLI's own config; only prints what to paste.

set -eu

usage() {
    cat <<'EOF'
Usage: mcp-connect.sh [--url URL] [--token TOKEN] [--redact] [--help]

Options:
  --url URL      Atlas server base URL (default: resolved from
                  ATLAS_LISTEN_ADDR or 127.0.0.1:4000)
  --token TOKEN  MCP bearer token (default: ATLAS_MCP_TOKEN env var)
  --redact       Replace the token with <ATLAS_MCP_TOKEN> in the output,
                  safe for pasting into chat or an issue
  --help         Show this message

Precedence: CLI flags > ATLAS_* env vars > built-in defaults.
EOF
}

url=""
token=""
redact=0

while [ $# -gt 0 ]; do
    case "$1" in
        --url)
            [ $# -ge 2 ] || { echo "mcp-connect.sh: --url requires a value" >&2; exit 1; }
            url="$2"
            shift 2
            ;;
        --token)
            [ $# -ge 2 ] || { echo "mcp-connect.sh: --token requires a value" >&2; exit 1; }
            token="$2"
            shift 2
            ;;
        --redact)
            redact=1
            shift
            ;;
        --help)
            usage
            exit 0
            ;;
        *)
            echo "mcp-connect.sh: unknown argument: $1" >&2
            usage >&2
            exit 1
            ;;
    esac
done

if [ -z "$url" ]; then
    listen_addr="${ATLAS_LISTEN_ADDR:-127.0.0.1:4000}"
    case "$listen_addr" in
        0.0.0.0:*) listen_addr="127.0.0.1:${listen_addr#0.0.0.0:}" ;;
    esac
    url="http://${listen_addr}"
fi

if [ -z "$token" ]; then
    token="${ATLAS_MCP_TOKEN:-}"
fi

if [ -z "$token" ]; then
    cat >&2 <<EOF
mcp-connect.sh: ATLAS_MCP_TOKEN is not set.

Set it before running this script, e.g.:
  export ATLAS_MCP_TOKEN="$(head -c 24 /dev/urandom | base64 | tr -d '=+/' 2>/dev/null || echo '<generate-one>')"

The same value must be exported to atlas-server's environment.
EOF
    exit 1
fi

health_url="${url}/health"
if ! curl -fsS --max-time 3 "$health_url" >/dev/null 2>&1; then
    echo "mcp-connect.sh: is atlas-server running? tried ${health_url}" >&2
    exit 1
fi

mcp_url="${url}/api/mcp"
shown_token="$token"
if [ "$redact" -eq 1 ]; then
    shown_token="<ATLAS_MCP_TOKEN>"
fi

cat <<EOF
# Atlas is healthy at ${url}
# MCP endpoint: ${mcp_url}

## Claude Code

    claude mcp add --transport http atlas ${mcp_url} \\
      --header "Authorization: Bearer ${shown_token}"

## Codex CLI

Add to ~/.codex/config.toml:

    [mcp_servers.atlas]
    url = "${mcp_url}"
    bearer_token = "${shown_token}"

## Cursor

Add to .cursor/mcp.json (project) or ~/.cursor/mcp.json (global):

    {
      "mcpServers": {
        "atlas": {
          "url": "${mcp_url}",
          "headers": {
            "Authorization": "Bearer ${shown_token}"
          }
        }
      }
    }
EOF
