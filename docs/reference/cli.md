# CLI

> **Scope:** the `atlas` command — what it can reach, and how it
> decides who you are.
> **Status:** current · **Updated:** 2026-09-10

`atlas` drives a running server from a terminal. It is a client of the
same MCP surface agents use, not a second way in, so anything it can do
an agent can do — and a change that breaks agents breaks this too
rather than being hidden behind a shared type.

## Identity

**From the environment, never from a flag.**

| Variable | Resolves to |
|---|---|
| `ATLAS_SESSION_TOKEN` | that session, and its owner |
| `ATLAS_MCP_TOKEN` | the reserved `local` owner — a permitted client, nobody in particular |

The session token wins when both are set. There is deliberately no
`--token`: a flag invites passing somebody else's, and identity here is
a property of the credential rather than something a caller states. See
[Identity](/reference/mcp-tools#identity).

`atlas whoami` says which one is in use and proves it works by using
it, rather than reporting what an environment variable claims.

### Inside a terminal Atlas spawned

Nothing to configure. Every terminal is handed its own credential and
told where the server is:

| Variable | |
|---|---|
| `ATLAS_SESSION_TOKEN` | minted for this terminal; resolves to its session and owner |
| `ATLAS_SESSION_ID` | the session it belongs to |
| `ATLAS_URL` | where to call Atlas back |

So an agent working there can run `atlas whoami`, `atlas msg`, `atlas
call` — and every call is attributable to that session rather than to
a shared secret.

`ATLAS_MCP_TOKEN` is **removed** from that environment. A terminal with
a specific identity should not also hold the shared one, which on a
team server reaches nobody's data and on a personal install is simply
the weaker credential.

A session may hold several tokens — one from `sessions.create`, one per
terminal. They are equivalent, and deleting the session revokes all of
them at once.

## Reaching everything

```bash
atlas tools                      # every tool the server exposes
atlas call <tool> key=value ...  # run one
```

`call` is the reason the CLI can claim to exercise all of Atlas: it
reaches every tool, **including ones added after this binary was
built**. Everything else is a shortcut for what gets typed often, and
adds no capability.

A value that parses as JSON is sent as JSON, anything else as a string
— so `limit=10` is a number, `payload={"a":1}` is an object, and
`title=my session` needs no quoting gymnastics. Only the first `=`
splits, so a value may contain more. When a value is too awkward for
that, pass the whole object:

```bash
atlas call memory.remember --json-args '{"scope":"personal","key":"k","value":{"a":1}}'
```

## Shortcuts

Each is `call` with the arguments filled in and the result rendered.
They add no capability.

**Sessions and messages**

```bash
atlas ls --project your-org/your-project       # your sessions
atlas msg --project p --to <session> "text"
atlas msg --project p "broadcast text"
atlas inbox --project p --session <id>
atlas whoami
```

**Issues**

```bash
atlas issues --project p                      # id, status, milestone, title
atlas issues --project p --status open
atlas issues --project p --label feature --label backend
atlas issues --project p --milestone v1
atlas issues --project p --board              # a column per status
atlas issue --project p 142                   # one, with its body
```

`--board` groups the same issues into columns rather than rows. It is a
board without cards to drag, which is the point: an issue's status
changes because a command or an agent changed it, and dragging was
never the part carrying the meaning.

**Progress**

```bash
atlas progress --project p                    # everything open
atlas progress --project p --milestone v1     # one roadmap
```

Acceptance criteria ticked over total — raw criteria, not issue count.
Issues with no criteria are left out of the breakdown, because they
contributed nothing to the number and cannot move it.

**Workflow runs**

```bash
atlas runs --workflow <workflow-id>
atlas runs --workflow <id> --status failed
atlas run <run-id>                            # state, then every node event
```

`atlas run` prints the run's event trail in order — `enter`, `exec`,
`gate_pass`, `gate_fail`, `retry`, `advance`, `complete` — which is the
record of what the runtime decided rather than what an agent reported.

**Project map**

```bash
atlas findings --project p
atlas findings --project p --severity error
```

Findings always print what they were drawn from — how many files, how
many imports resolved and how many did not. "No findings" from a graph
that resolved a tenth of its imports is a weaker statement than the
same words from a dense one, and the line is there so the two cannot be
mistaken for each other. `--severity` filters in the CLI; the tool
itself returns everything.

## Output

A table when stdout is a terminal, JSON when it is not — so a pipeline
gets JSON without asking for it, and reading it by eye gets a table.
`--json` forces JSON either way.

Exit status is 0 only when the call succeeded. A tool's own failure
arrives from MCP as a *successful* JSON-RPC response carrying
`isError`, which the CLI turns into a non-zero exit — otherwise a shell
script would think a failed call worked.

## Where it lives

The container image ships it at `/usr/local/bin/atlas`. Built from
source it is `target/release/atlas`.

```bash
export ATLAS_URL=http://127.0.0.1:4000   # the default
```
