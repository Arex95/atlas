# Atlas

**Several agents, one way of working, nothing taken on trust.**

An agent will tell you the task is done. Atlas is built so that saying
it is not enough: the acceptance criteria are executed by the runtime,
never self-reported, and the reason for a failure goes back into the
agent's context so the node retries. And the rules are not one
developer's discipline — a workflow is a graph declared in the
repository, read from the same file on every machine on the team.

Underneath that: sessions that survive the machine they were started
on, real PTYs, and a bus the agents coordinate over directly.

Local-first, and it stays that way — it works standalone with no
account and no server, nothing leaves your machine until you point it
at one you host, and there is no inbound command channel to take over.

📖 **[Documentation](https://arex95.github.io/atlas/)** ·
[Connect an agent](https://arex95.github.io/atlas/guide/connecting-agents) ·
[HTTP API](https://arex95.github.io/atlas/reference/http-api) ·
[MCP tools](https://arex95.github.io/atlas/reference/mcp-tools)

```bash
cp .env.example .env       # set ATLAS_MCP_TOKEN
just run                   # http://127.0.0.1:4000
just connect-agent         # prints ready-to-paste config for your agent CLI
```

## Example

An agent, over MCP, registering a session and opening a real shell in it:

```jsonc
// sessions.create
{ "project": "your-org/your-project", "owner_id": "01M1TD4…",
  "remote_url": "git@github.com:you/your-project.git",
  "branch": "main", "relative_path": "your-org/your-project" }

// terminal.spawn → a real PTY, resolved against this machine's workspace root
{ "session_id": "01M1T52M…" }
```

If that directory does not exist on this machine — a second laptop, a
fresh container — `terminal.restore` clones it from the session's own
`remote_url` first. The session travels; the checkout is rebuilt where
it lands.

## What it covers

- **Sessions.** Who is working on what, where. Stored with a *relative*
  path resolved per machine, so the same session means the right
  directory on a laptop that keeps code in `~/dev` and a server that
  keeps it in `/srv`.
- **Terminals.** Real PTYs through `portable-pty`, not transcripts —
  spawn, write, read, close, and restore a workspace that is missing.
- **Coordination.** A message bus between sessions: broadcasts to
  everyone on a project, direct messages to one.
- **Workflow runs.** Declarative graphs whose nodes dispatch to an
  agent and advance only when acceptance criteria pass — on failure the
  reason is injected back into the agent's context and the node retries.
  Watchable live over SSE.
- **Agent memory.** Two buckets: what the agent knows about a *project*
  (shared) and about a *person* (private, and it stays that way even on
  the team server).
- **Tracker bridge.** Issues stay in GitLab or GitHub — Atlas holds no
  tasks table — with a local read mirror so an agent's reads are fast
  and its writes are real.
- **Team mode, optionally.** Your own server, your own accounts. Three
  sync cadences per developer: manual, polled, or live over SSE.

## What it leaves alone

No interface — the frontend is a separate repository. No task database:
issues live in the tracker you already use. No credential store: tracker
tokens are read from where you already keep them, the way an SSH key is,
and Atlas never persists one.

And no cloud. Atlas is single-tenant and self-hosted by design, not as a
step toward a hosted tier.

## Requirements

| | |
|---|---|
| Rust | `1.88` or newer (`rust-version` in `Cargo.toml`; the image pins the same) |
| Docker | `27`+, to run the image |
| [`just`](https://github.com/casey/just) | for the recipes below |

Configuration is entirely environment variables — see
[`.env.example`](./.env.example) and the
[full reference](https://arex95.github.io/atlas/reference/environment).

## Repository layout

```
bin/atlas-server/     entry point — composes the features, holds no logic
features/             one crate per business area
  afg/                workflow runs, gates, retries
  auth/               accounts, sessions, GitLab OAuth
  mcp/                the JSON-RPC surface agents call
  memory/             agent memory, project and personal buckets
  messaging/          the inter-session coordination bus
  sessions/           session registry and ownership
  sync/               focus / auto / live replication
  terminal/           the PTY pool
  tracker/            external tracker port plus its local mirror
docs/                 the documentation site (VitePress)
```

Each feature keeps its own `internal::{domain,application,infrastructure}`
and exposes one `api` module; nothing reaches past that boundary.

## Development

```bash
just              # list every recipe
just check        # fmt + clippy + build + test — the same gates CI runs
just run          # debug binary
just image        # build the release image locally
just up           # docker compose up --build
pnpm -C docs dev  # the documentation site
```

`just check` is the gate. Clippy runs with `-D warnings` on both stable
and the pinned MSRV, because a lint that only fires on one of them fires
in CI instead of on your machine.

## Contributing

1. An issue exists, with acceptance criteria written **before** the work.
2. `git fetch origin` and read both logs before branching.
3. Branch `<type>/<issue>-<short>`.
4. One branch, one coherent change. If it needs an "and" to describe, it
   is two.
5. `just check` green before pushing.
6. The pull request copies the acceptance criteria and ticks them with
   evidence — commands run and output seen, not "tested locally".
7. Verify against something running, not only against tests.

**By opening a pull request you agree that your contribution may be
released under the licence above and under a commercial licence.**
Without that, a single merged patch would make it impossible to license
the project to a company later without tracking down its author. If you
would rather not, say so in the pull request and it can be discussed —
better before the work than after.

**Documentation ships in the same commit as the change.** A new
endpoint updates the HTTP reference, a new tool updates the MCP
reference, a new variable updates both the environment reference and
`.env.example`, and a new user-facing capability gets a guide page.
Never a follow-up commit — deferred documentation cannot be audited.

Anything that constrains work beyond its own task gets written down
where the constraint lives: [`features/README.md`](./features/README.md)
for the shape every crate follows, a `docs/concepts/` page for anything
a reader would otherwise be surprised by. A decision with no written
rejection gets re-litigated six months later by someone who was not in
the room.

## Security

Known limits of the current design — one shared MCP token with no
scoping, terminals that run real commands — are written down in
[SECURITY.md](./SECURITY.md), along with where to report a
vulnerability. Read it before exposing a deployment.

## License

**[PolyForm Noncommercial 1.0.0](./LICENSE)** — free for any
noncommercial purpose: your own machine, research, teaching, a
nonprofit. Read, fork, modify, and run it; the whole source is here.

**Commercial use needs a separate licence.** Using Atlas in or for a
business, or offering it to anyone else as a product or a service, is
not covered by the licence above. If that is what you want, open an
issue titled `commercial licence` and we will sort it out — the answer
is yes, it just needs its own terms.

This is source-available, not open source: a licence that forbids
commercial use cannot be OSI-approved, and saying otherwise would be
misleading. Everything else about the project is unchanged — the code,
the history and the reasoning are all public.
