# Getting started

By the end of this page an agent CLI on your machine is calling Atlas
and getting real answers back.

Everything here is Mode 1: no account, no server, no network. That is
not a trial — it is the whole product, and nothing you set up now has
to be redone if you add a team server later.

## Requirements

| | |
|---|---|
| Rust | `1.88` or newer, to build from source |
| Docker | `27`+, if you would rather run the image |
| [`just`](https://github.com/casey/just) | for the recipes below |

## Run it

```bash
git clone git@github.com:Arex95/atlas.git
cd atlas-server
cp .env.example .env
```

Open `.env` and set one value:

```bash
ATLAS_MCP_TOKEN=some-long-random-string
```

That token gates the MCP endpoint. It is required — the server refuses
to start without it rather than coming up unprotected. Generate one
with `openssl rand -hex 32` if you have no preference.

Then:

```bash
just run
```

```
INFO atlas_server: atlas-server listening version="0.1.0"
     addr=127.0.0.1:4000 tracker=None mirror="off" mcp="on"
```

Check it from another shell:

```bash
curl -s http://127.0.0.1:4000/health
# {"status":"ok","version":"0.1.0"}
```

That endpoint runs a real query against the database rather than
answering as soon as the process is up, so `ok` means the storage
works too.

::: tip Port already taken?
`4000` is only the default. Set `ATLAS_LISTEN_ADDR=127.0.0.1:4001` in
`.env` — Atlas refuses to start on a busy port rather than quietly
choosing another, and the error names the variable. See
[Environment variables](/reference/environment#atlas-listen-addr).
:::

### Or with Docker

```bash
cp .env.example .env    # same one value to set
just up
```

Compose reads that same `.env`, keeps the database on a named volume,
and publishes port 4000. `just down` tears it down and drops the
volume.

## Connect an agent

```bash
just connect-agent
```

This prints ready-to-paste configuration for Claude Code, Codex CLI and
Cursor, filled in with the address and token it resolved. Add
`--redact` before pasting the output anywhere but your own terminal.

The full walkthrough, including what to check when a tool call fails,
is in [Connecting an agent](/guide/connecting-agents).

## Check that it works

Ask your agent something only Atlas can answer:

> list my sessions in your-org/your-project

It should call `sessions.list` and come back with an empty list —
which is the right answer, and proves the round trip. If instead it
says it has no such tool, the CLI has not picked up the config; if it
reports an authentication failure, the token in the CLI and the one in
`.env` disagree.

## What you have now

Thirty-one tools, all working, with no server and no account:

- **Sessions** — register who is working where, and on which branch
- **Terminals** — spawn a real shell in a session and drive it
- **Messaging** — broadcasts and direct messages between sessions
- **Workflow runs** — nodes that advance only when their checks pass
- **Agent memory** — two buckets, one about the project and one about you
- **Sync** — inert until you point it at a server

The full list, with parameters, is in
[MCP tools](/reference/mcp-tools).

## Where to go next

- [Sessions and terminals](/guide/sessions-and-terminals) — the loop
  you will actually use day to day
- [Workflow runs](/guide/workflow-runs) — tasks that verify themselves
- [Connect a tracker](/guide/tracker) — read and write real issues
- [Team mode](/guide/team-mode) — when a second machine or a second
  person shows up
- [Why Atlas](/concepts/why-atlas) — what it decided and what it left
  alone
