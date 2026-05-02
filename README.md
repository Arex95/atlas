# Atlas

**Terminal Orchestrator for humans and AI agents.**

Atlas spawns real PTY-backed terminals (bash/zsh) in your browser, lets AI agents inhabit them, and gives them tools to talk to each other through a shared workspace and MCP layer. The primary unit of interaction is the terminal — AI is just a layer that plugs in.

[![License](https://img.shields.io/badge/license-MIT-green)](./LICENSE)

---

## What's inside

- **PTY-backed terminals** in the browser (xterm.js + portable-pty), with broadcast channels so multiple tabs share one PTY and survive reload.
- **Cross-session orchestration** — agents in different terminals exchange messages through Socket.io rooms.
- **MCP server** (JSON-RPC 2.0) at `/api/mcp` — 29 tools covering projects, sessions, memory, tasks, documents, reminders, skills, notifications, filesystem, and global context.
- **Session-scoped data** — each session has private memory, documents, tasks, and reminders visible in the session dashboard. Project-scoped data is shared across all sessions of a project.
- **Project isolation** — agents cannot read or write resources from other projects.
- **Global Context** — memory, skills, and prompts shared across all projects and readable by every agent.
- **Atlas CLI** (`atlas-*` helpers) injected into every spawned shell's `$PATH`.
- **Path-traversal protection** on every filesystem-facing endpoint.
- **Project / session dashboards** with tasks, reminders, timeline (Gantt), documents, memory, and a Cmd+K global search palette.

---

## Stack

| Layer | Tech |
|---|---|
| Backend | Rust — Axum 0.7 + sqlx 0.8 + socketioxide 0.18 + portable-pty |
| Frontend | Vue 3 + Vite + Pinia + xterm.js + Tailwind 4 |
| CLI | Rust + Clap |
| DB | SQLite (single file, auto-created) via sqlx |
| Workspace | pnpm (TS packages) + Cargo (Rust) |

---

## Installation

Atlas is **local-first**. Clone the repo, build once, run one command.

### Prerequisites

| Tool | Version |
|---|---|
| [Rust](https://rustup.rs) | 1.85+ |
| [Node.js](https://nodejs.org) | 22+ |
| [pnpm](https://pnpm.io/installation) | 10+ |

### Clone and install

```bash
git clone https://github.com/Arex95/atlas.git
cd atlas
cp .env.example .env
cp apps/web/.env.example apps/web/.env
pnpm install
make install
```

`make install` builds the Vue frontend (`apps/web/dist/`) and compiles the Rust server in release mode. This takes ~5–10 minutes the first time while Cargo downloads and compiles dependencies. Subsequent builds are fast.

### Run

```bash
make start
```

Open **<http://localhost:4000>**. The server serves the frontend and the API from the same port — no separate process needed.

That's it. No Docker, no database setup, no background services.

---

## First steps

1. Click `+` in the sidebar header → create a project (point `Root Path` at any directory you own).
2. Expand the project → click `+` to spawn an AI session inside it.
3. The PTY runs in the Rust backend and streams over Socket.io. Type freely.
4. Use **Cmd+K** (or **Ctrl+K**) to open the global search palette.
5. Click the globe icon in the sidebar footer to open the **Global Context** — shared memory, skills, and prompts visible to all agents.

---

## Development mode

For hot-reload during development, run backend and frontend in parallel:

```bash
make dev
```

This starts:
- `cargo run -p atlas-server` on `:4000`
- `pnpm --filter web dev` on `:3000` (Vite HMR)

Open <http://localhost:3000> in dev mode. The frontend proxies API calls to `:4000` via `VITE_SERVER_URL`.

---

## Makefile reference

| Command | What it does |
|---|---|
| `make install` | Build frontend + compile Rust server (run once after cloning or after any code change) |
| `make start` | Start the production server — frontend + API on `:4000` |
| `make dev` | Dev mode: Vite on `:3000` + `cargo run` on `:4000` in parallel |
| `make build-web` | Rebuild the Vue frontend only |
| `make build-server` | Recompile the Rust server only (release) |
| `make check` | Zero-warning gate: clippy + vue-tsc + vite build |
| `make clean` | Remove `target/` and `apps/web/dist/` |

> **Note on migrations:** SQL migrations are embedded into the binary at compile time. Every time you add a migration file, run `make build-server` before `make start`.

---

## Configuration

Copy `.env.example` to `.env` and edit as needed. All variables are optional for local development.

| Variable | Default | Notes |
|---|---|---|
| `PORT` | `4000` | Port the server listens on |
| `DATABASE_URL` | `sqlite://./atlas-data/atlas.db` | Created automatically on first run |
| `WEB_ORIGIN` | `http://localhost:3000` | CORS allow-origin for dev mode |
| `ATLAS_API_TOKEN` | unset | When set, all REST API routes require `Authorization: Bearer <token>`. Generate with `openssl rand -hex 32`. Leave unset for local dev. |
| `ATLAS_MCP_TOKEN` | unset | When set, `/api/mcp` requires `Authorization: Bearer <token>`. Forwarded into every PTY's env automatically. |
| `ATLAS_SERVER_URL` | `http://localhost:4000` | URL injected into PTYs so CLI helpers can reach the server |

Frontend (`apps/web/.env`):

| Variable | Default | Notes |
|---|---|---|
| `VITE_SERVER_URL` | `http://localhost:4000` | Backend URL the browser talks to |
| `VITE_API_TOKEN` | unset | Must match `ATLAS_API_TOKEN` when auth is enabled |

---

## Architecture

```
                          ┌────────────────────┐
                          │ Browser (Vue + xterm)│
                          └──────────┬──────────┘
                                     │ HTTP + Socket.io
                                     ▼
              ┌──────────────────────────────────────┐
              │   atlas-server :4000  (Rust / Axum)  │
              │                                      │
              │   /api/*  ──► handlers ──► sqlx       │
              │   /        ──► apps/web/dist/ (SPA)  │
              │   socketioxide ──► TerminalManager   │
              │   /api/mcp ──► MCP dispatcher (29)   │
              └──────────────────┬───────────────────┘
                                 │ portable-pty
                        ┌────────▼────────┐
                        │   bash / zsh    │
                        │   $PATH +=      │
                        │   atlas-* CLI   │
                        │   ATLAS_*_ID    │
                        └────────┬────────┘
                                 │ JSON-RPC → /api/mcp
                        ┌────────▼─────────┐
                        │  29 MCP tools    │
                        └───────────────────┘
```

Three communication paths:
1. **Browser ↔ server** — REST for CRUD; Socket.io for terminal I/O, cross-session messages, and notifications.
2. **Inside-PTY agent ↔ server** — `atlas-*` CLI helpers call MCP via HTTP using the injected token.
3. **Server broadcast** — `TerminalManager` emits events to Socket.io rooms on PTY activity.

---

## MCP tools (29 total)

Agents running inside Atlas sessions have access to:

| Category | Tools |
|---|---|
| Projects & files | `list_projects`, `read_file`, `update_project` |
| Sessions & messaging | `list_sessions`, `send_message`, `read_inbox`, `get_history`, `save_message` |
| Memory | `list_memory`, `get_memory`, `set_memory` |
| Documents | `list_documents`, `read_document`, `create_document`, `write_document`, `delete_document` |
| Tasks | `list_tasks`, `create_task`, `update_task`, `delete_task` |
| Reminders | `list_reminders`, `create_reminder`, `update_reminder` |
| Skills | `list_skills`, `run_skill` |
| Notifications | `create_notification` |
| Global context (read-only) | `global_list_memory`, `global_list_skills`, `global_list_prompts` |

Tools that write data default to **session scope** (private to the calling session). Pass `scope="project"` + `projectId` for data shared across all sessions of a project.

---

## Connecting via MCP

Inside any spawned session, or from your machine:

```bash
npx -y mcp-remote http://localhost:4000/api/mcp
```

The server returns `serverInfo.instructions` on initialize — MCP clients auto-inject these into the agent context so it knows how to use all 29 tools correctly.

---

## API examples

```bash
# Create a project
curl -X POST http://localhost:4000/api/projects \
  -H "Content-Type: application/json" \
  -d '{"name":"My Project","slug":"my-project","rootPath":"/home/me/code/my-project"}'

# List active sessions
curl http://localhost:4000/api/sessions/active

# MCP — list all tools
curl -X POST http://localhost:4000/api/mcp \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":1,"method":"tools/list"}'
```

---

## Troubleshooting

**`make install` fails on Rust compilation** — make sure Rust 1.85+ is installed: `rustup update stable`.

**Port 4000 already in use** — `lsof -i :4000` to find the PID, then `kill <PID>`.

**Frontend shows blank page after `make start`** — the `dist/` may be stale. Run `make build-web` then `make start` again.

**Server won't start — `Invalid WEB_ORIGIN`** — check `.env` for trailing whitespace on `WEB_ORIGIN`.

**API returns 401** — set matching `ATLAS_API_TOKEN` in `.env` and `VITE_API_TOKEN` in `apps/web/.env`, or leave both unset for local dev.

**MCP returns 401** — set `ATLAS_MCP_TOKEN` in `.env`, or leave unset to disable MCP auth.

**Database — clean slate:**
```bash
rm -f apps/atlas-server/atlas-data/atlas.db
make start   # auto-migrates on startup
```

**`target/` is huge** — `cargo clean` removes all Rust build artifacts (~2–4 GB is normal for a Rust project).

---

## Contributing

See [CONTRIBUTING.md](./CONTRIBUTING.md).

## License

MIT — see [LICENSE](./LICENSE).
