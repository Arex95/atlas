# MCP tools

The 40 tools an agent can call over `POST /api/mcp`. This page is the
readable version; the machine-readable one is `tools/list` on a running
server, which is generated from the same source and cannot drift from
it:

```bash
curl -s -X POST http://127.0.0.1:4000/api/mcp \
  -H 'Content-Type: application/json' \
  -H "Authorization: Bearer $ATLAS_MCP_TOKEN" \
  -d '{"jsonrpc":"2.0","id":1,"method":"tools/list"}' | jq '.result.tools[].name'
```

In the parameter tables below, **bold** means required.

## Identity

Every request authenticates with one of two credentials, and the
difference between them is the identity they carry.

| Credential | Resolves to | Where it comes from |
|---|---|---|
| **Session token** | that session, and its owner | returned once by `sessions.create` |
| **`ATLAS_MCP_TOKEN`** | the reserved owner `local` | your configuration |

**You may act on the sessions you own, and no others.** Every tool that
names a session — `sessions.*`, `terminal.*`, `afg.start_run`,
`messaging.read_inbox` — checks that the caller owns it, and a session
belonging to somebody else is reported as **not found** rather than as
forbidden, so ids cannot be probed for.

Scoped by *owner*, not by session: Atlas exists to let one developer's
agents drive one another, so a session's token reaches that developer's
other sessions. It reaches nobody else's.

**No tool takes an `owner_id`.** Anything touching personal state acts
on the owner behind the credential, and sending an `owner_id` anyway is
a parameter error — rejected rather than ignored, so a client written
against the older contract fails loudly instead of quietly acting as
somebody it did not name.

`sessions.create` returns `session_token` **once**. Only its hash is
stored, so it cannot be recovered afterwards; if it is lost, create a
new session. Deleting a session revokes its token.

There is no mode switch here. A standalone install has one developer
and no accounts, so everything it owns belongs to `local` and the
shared token is all it ever needs. A team server issues real user ids,
which `local` is never one of, so the shared token reaches nobody's
data there — a developer's rows arrive by authenticated sync, under
their own id.

::: warning The shared token is a machine credential, not a person's
It says a client is permitted. It does not say who. Give an agent a
session token when you want its calls attributed to a developer.
:::

One limitation worth knowing before you build on this surface:

- **Reads are poll-based.** `messaging.read_inbox` and
  `terminal.read_output` page forward from a cursor you pass back.
  There is no push over MCP; the SSE streams on the HTTP surface are
  for people watching, not agents working.

## Tracker

Issues live in the external tracker, not in Atlas — see
[Connect a tracker](/guide/tracker). Reads
come from a local mirror when one is active; writes always go to the
real tracker and refresh the mirror immediately, so a write is never
visible locally before it is real.

| Tool | Parameters | |
|---|---|---|
| `tracker.list_issues` | **project**, status, labels, updated_after, milestone | List issues. |
| `tracker.get_issue` | **project**, **id** | One issue by tracker id. |
| `tracker.list_relations` | **project**, **id** | `blocks` / `blocked_by` / `relates_to` for an issue. |
| `tracker.create_issue` | **project**, **title**, description, labels | Create an issue. |
| `tracker.update_status` | **project**, **id**, **status** | `open` or `closed`. |
| `tracker.close_issue` | **project**, **id** | Sugar over `update_status`. |
| `tracker.plan_progress` | **project**, status, labels, updated_after, milestone | Counts acceptance-criteria checkboxes. |

`plan_progress` counts `- [ ]` / `- [x]` lines under an
`## Acceptance criteria` heading — **raw criteria, ticked over total,
not issue count**. An issue with sixteen criteria and one ticked is 1/16
here, not "0 of 1 issues done". That is the number that reflects real
progress on a plan.

**Pass `milestone` to ask about a roadmap.** A milestone is the
tracker's own name for "the thing we are shipping next", so filtering by
one turns `plan_progress` into the completion of that plan rather than
of everything open. Matched on the milestone's title — what a person
types and what the tracker shows them. A title no milestone has returns
`0/0`, not the whole project.

## Messaging

The coordination bus between sessions — see
[Coordination](/guide/coordination).

| Tool | Parameters | |
|---|---|---|
| `messaging.send_message` | **project**, **from**, **payload**, to, type, correlation_id, reply_to | Omit `to` to broadcast. |
| `messaging.read_inbox` | **project**, **for**, since, limit | Broadcasts plus direct messages, oldest first. |

`read_inbox` returns every broadcast on the project plus every message
addressed to `for`. Pass the last seen message id as `since` to page
forward.

**An address that names a session is private to that session's owner.**
Workflow runs dispatch every `task` to a session id, so that is where
the sensitive traffic is; asking for somebody else's returns an empty
list rather than an error, because distinguishing "not yours" from "no
such session" would answer whether a session id exists to anyone who
asked.

Any other address is a **free-form label** — the deliberate design of
this bus, which predates the session registry — and a label has no
owner. Two agents that agree on a name share a channel, and anyone on
the project can read it. Use a session id for anything that should not
be.

## Sessions

A session is the registration of "someone is working on this project,
on this branch, in this directory" — metadata, with no process attached
until `terminal.spawn`.

| Tool | Parameters | |
|---|---|---|
| `sessions.create` | **project**, **remote_url**, **branch**, **relative_path**, agent_kind, title, resume_command | Register a session. Returns `session_token` once. |
| `sessions.list` | **project**, status | Your own sessions only. |
| `sessions.get` | **id** | Yours; anyone else's is a not-found. |
| `sessions.update_status` | **id**, **status** | `active` or `archived`. |
| `sessions.set_resume_command` | **id**, command | What to run when a terminal spawns. Omit `command` to clear. |

Every read and write is scoped to an owner, and **a session owned by
someone else reports the same not-found error as an id that does not
exist**. Distinguishing them would confirm to a caller that a session
they may not see is there.

`relative_path` is deliberately relative: it is resolved against each
machine's own `ATLAS_WORKSPACE_ROOT`, so the same session row means
"the Atlas checkout" on a laptop that keeps it at `~/dev` and on a
desktop that keeps it at `/srv/code`.

## Terminal

Real PTYs through `portable-pty` — a shell process, not a transcript.

| Tool | Parameters | |
|---|---|---|
| `terminal.spawn` | **session_id** | Idempotent: spawning a running session returns the existing process. |
| `terminal.write` | **session_id**, **input** | Raw stdin. |
| `terminal.read_output` | **session_id**, since_offset | Returns `next_offset` to page forward. |
| `terminal.close` | **session_id** | Kills the process and drops it from the pool. |
| `terminal.restore` | **session_id** | Makes the workspace exist on this machine. |

`terminal.restore` is what makes a session portable: if its resolved path
is missing, it clones from the session's own `remote_url` and `branch`.
A no-op when the path is already there.

It **does not resolve missing git credentials** — it reports them
distinctly, as `kind: credentials_missing` under error code `-32010`,
separately from a generic clone failure. Detect and report, do not
resolve: a server that silently acquires credentials on your behalf is
a worse problem than a clone that failed and said why.

## Workflow runs (AFG)

Declarative workflows whose nodes only advance when their acceptance
criteria pass.

| Tool | Parameters | |
|---|---|---|
| `afg.register_workflow` | **project**, **project_root**, source_path, yaml | Provide `source_path` **or** `yaml`, not both. A `source_path` workflow is re-read from disk on every `start_run`, so the committed file stays the source of truth; re-registering is only needed to change which file it points at. |
| `afg.start_run` | **workflow_id**, **session_id**, target_session_id | Dispatches the first runnable node as a `task` message. |
| `afg.submit_task_result` | **run_id**, **node_id**, payload | Runs the node's gates, then advances, retries or fails. Only the session the node was dispatched to may call it. |
| `afg.get_run` | **run_id** | Status plus the full event timeline. |
| `afg.list_runs` | **workflow_id**, status | |

### Scoping what a node's agent may call

A node may name the tools its agent is expected to use:

```yaml
nodes:
  - id: check
    title: Run the tests
    instructions: run them and report
    allowedTools:
      - tracker.*        # a family
      - notes.write      # an exact name
```

While that node is dispatched to a session, any other tool that
session calls is refused. `afg.submit_task_result` and `afg.get_run`
are always permitted — without them a scoped node is a trap, because
the agent could not report that it finished and the run would stall.

Omitting `allowedTools` scopes nothing, so a workflow written before
this existed behaves exactly as it did. The scope belongs to the node,
not to the session: when the run completes, the agent has every tool
back.

::: danger This is not containment
A node is dispatched to a session whose agent holds a **real
terminal**. It can do anything its user can do by typing, and a list
of tool names does not change that. The only boundary is the operating
system — see [Trust model](/concepts/trust-model).

What this limits is **accidents**: a node that says "run the tests"
has no business closing an issue, whether it decided to through a
misread instruction, a hallucinated plan, or another agent's message.
Treating it as a security control would be relying on something that
was never one.
:::

**Only the session a node was dispatched to may answer for it.** The
credential decides — see [Identity](#identity) — and a caller that is
not that session gets a not-found rather than a forbidden, so it cannot
learn that a run it may not touch exists. The shared `ATLAS_MCP_TOKEN`
identifies no session and can therefore never submit a result.

**Gates run where the workflow was registered**, in the `project_root`
given to `afg.register_workflow` and recorded then. It is not a
parameter of `submit_task_result`, and sending it is an error: an
acceptance criterion of type `shell` executes there, so a caller able
to name the directory would be choosing where somebody else's commands
ran.

`submit_task_result` is where the loop actually turns: it runs the
node's acceptance criteria in order; on pass it advances or completes
the run, on failure it **retries with the failure injected back into
the agent's context** up to the node's `max_retries`, and fails the run
after that. A node with no criteria always passes — the gate is opt-in
per node, so "no criteria" means "nothing to check", not "checked and
fine".

Dispatch travels over the messaging bus as a `task` message. The target
session's owner discovers it by reading their own inbox, like any other
coordination message — there is no injection into a PTY.

`afg.get_run` is a point-in-time snapshot. To watch a run as it moves,
a person's client streams
[`GET /api/afg/runs/{run_id}/events`](/reference/http-api#workflow-runs-api-afg).

## Sync

The client half of the three propagation modes — see
[Sync](/concepts/sync). These tools
act **against a remote team server**, so they take that server's URL
and a session token issued *by it* — not the local `ATLAS_MCP_TOKEN`.

| Tool | Parameters | |
|---|---|---|
| `sync.sessions_now` | **remote_url**, **bearer_token** | One push-then-pull pass of sessions. |
| `sync.memory_now` | **remote_url**, **bearer_token** | One pass of agent memory. |
| `sync.notes_now` | **remote_url**, **bearer_token** | One pass of your notes. |
| `sync.set_mode` | **mode**, remote_url, bearer_token, interval_secs | `focus`, `auto` or `live`. |
| `sync.status` | — | Current mode plus the last pass's outcome. |

The three modes:

- **`focus`** — the default. Nothing syncs until asked. No background
  loop, no network noise. `set_mode` with `focus` stops whatever was
  running.
- **`auto`** — a background loop every `interval_secs`.
- **`live`** — holds an SSE stream open to the remote and runs a pass
  the moment it reports a change, reconnecting on its own with backoff.
  No interval to configure.

Calling `set_mode` again replaces the running loop rather than adding a
second one. The configuration is held **only in process memory and is
never persisted** — a restart drops back to `focus` and `auto`/`live`
must be re-armed. That is deliberate: the alternative is a file on disk
holding a credential for someone else's server.

`sync.status` reports `stream_connected` in `live` mode, so a silently
dropped connection is distinguishable from a quiet team.

## Notes

A developer's own notes (Type 1 / Type 2 split: these are **Type 2**).
Personal by construction — there is no shared variant, so there is
nothing to select and no owner to name.

| Tool | Parameters | |
|---|---|---|
| `notes.write` | **name**, **body** | Replaces whatever was under that name. |
| `notes.read` | **name** | |
| `notes.list` | — | Yours, newest first, bodies included. |
| `notes.delete` | **name** | Errors if there was nothing there. |

Addressed by a name you choose rather than a generated id, so writing
is idempotent and a terminal can reach a note without copying an id
around. **A scratchpad is simply the note called `scratchpad`** — one
concept, not two.

An empty `body` is allowed: clearing a scratchpad without deleting it
is a normal thing to want. Deleting something that was never there is
an error rather than a silent success, because the usual cause is a
typo in the name.

```bash
atlas call notes.write name=scratchpad "body=half an idea"
atlas call notes.list
```

::: tip Nothing here takes an owner
`owner_id` is not a parameter of any of these, and sending one is a
parameter error. The owner comes from the credential — see
[Identity](#identity). Somebody else's note is reported as *not found*
rather than forbidden, so a name cannot be probed for.
:::

## Project Map

An index of what is in a project and how it relates, so an agent can
orient itself without reading files until it finds its bearings.

| Tool | Parameters | |
|---|---|---|
| `graph.reindex` | **project**, **project_root** | Build or rebuild from disk. |
| `graph.overview` | **project** | Counts, hubs, orphans. Start here. |
| `graph.find` | **project**, **query**, limit | Full-text over content, names and headings. |
| `graph.node` | **project**, **fqn** | One node by identity. |
| `graph.related` | **project**, **fqn** | Everything touching it, both directions. |
| `graph.outline` | **project**, **file_path** | A markdown file's headings. |
| `graph.findings` | **project** | Located observations about the structure. |
| `graph.changes` | **project**, **project_root**, against, depth | What the current changes reach. |
| `graph.watch` | **project**, **project_root** | Index, then keep indexing as files change. |
| `graph.unwatch` | **project** | Stop. Not an error if it was not watched. |
| `graph.watch_status` | — | What is watched, and how each is doing. |

Nodes are **files and markdown sections**; a section's identity is
`docs/api.md#authentication`. Edges are `contains` (structure), `links`
(a markdown link), `imports` (an import statement) and `mentions` (one
file naming another in its text).

Nothing here is language-aware — six universal patterns over any text
file — which is why a repository is indexed the day it appears, in
whatever it is written in. The cost is precision, paid deliberately:

- **An import that does not name a file in the project is left
  unresolved**, not pointed at a guess. `related` reports `resolved`
  per edge, so an agent can tell "this points outside the project"
  from "this points at nothing".
- **An import that names a symbol resolves to the file defining it.**
  `use crate::internal::infrastructure::AfgStore` names a type, and no
  file is called `AfgStore`; trailing segments are dropped until
  something resolves. Importing a symbol *is* a dependency on the file
  that defines it, and the shortest path naming a real file is the
  closest a file-level graph can come to saying so. The two rules below
  bound it: a shortened path that lands on a package the project does
  not contain is refused, and one matching several files ambiguously
  still resolves to nothing.
- **An import naming a package the project does not contain is left
  unresolved.** `use axum::extract::…` will not be pointed at a local
  `extract/` module just because the names line up. The root segment
  of the import has to name a directory in the project — or be
  relative (`crate`, `self`, `super`, `./`) — before a suffix match is
  allowed to resolve it at all.
- **Module paths resolve by suffix, narrowed by the segments the
  suffix dropped, and only then by proximity.** `use
  atlas_auth::api::…` matches an `api` file in every crate of a
  workspace; the dropped `atlas_auth` names the auth crate, so that
  candidate wins even when another is nearer. Proximity decides only
  when the dropped segments name nothing — `use crate::internal::domain`
  — which is what a module system does. Two candidates the rules
  cannot separate resolve to nothing rather than to a coin flip.
- **`mentions` is a heuristic** and the only predicate that can be
  wrong: unique basenames of four characters or more, matched as whole
  words. It will miss things and occasionally connect two files that
  merely share a word. Treat it as a hint, not a fact.

Indexing this repository — 220 files — produces 392 nodes and 1674
edges in about 400 ms, resolving 209 of its 213 imports that point
inside the project.

### Keeping it fresh

`graph.reindex` is a one-shot. On a project being worked on, prefer
`graph.watch`: it indexes once, then reindexes automatically when files
change. A stale map is worse than none, because an agent trusts what it
reads and acts on a repository that has since moved.

Three behaviours worth knowing:

- **Build output does not trigger anything.** Changes under `target/`,
  `node_modules/` and `.git/` are filtered before they reach the
  debounce. Without that, a `cargo build` would reindex continuously
  for as long as it runs.
- **Bursts collapse.** A save emits several events and a branch switch
  emits thousands; they are coalesced into one reindex after 500 ms of
  quiet.
- **Watching is held in memory only**, like `auto` and `live` sync. A
  server restart watches nothing until asked again.

`graph.watch_status` reports how many automatic reindexes each watch
has run, and the last one's outcome — a `last_error` that persists
means the map is drifting from the disk.

### Findings

`graph.findings` reports specific, located observations about a
project's structure. Each names the files it is about, so it can be
acted on rather than merely read.

| Kind | Severity | What it means |
|---|---|---|
| `import_cycle` | warning | Files that import each other, directly or through a chain. None can be understood, tested or moved without the others. |
| `unreferenced_file` | info | A source file that neither imports anything in the project nor is imported by it. Possibly dead, possibly reached another way. |
| `widely_depended_on` | info | Imported by eight or more files. Not a defect — a widely-used type *should* have high fan-in — but changing one reaches further than changing a leaf. |
| `large_file` | info | Over 600 lines. Sometimes right, often a module that grew into two. |
| `undocumented_directory` | info | Holds source files and no README. |
| `layer_violation` | **error** | An import crossing a boundary the project declared closed. The only finding measured against a rule rather than a heuristic. |
| `layer_check_incomplete` | info | How much of the project the layer check could see. Emitted whenever it saw less than all of it. |
| `layers_file_unreadable` | warning | The declaration could not be used, so no rule was checked. |

**There is deliberately no score.** A composite number invites making
the number go up, hides which finding you are actually looking at, and
would here be computed largely from `mentions` — the one predicate
documented as able to be wrong.

Only **resolved `imports`** edges count as dependencies. Not
`mentions`, not `links`, not `contains`. Every report therefore carries
an `evidence` object, and it should be read before the findings are
trusted:

```json
{ "files": 220, "resolved_imports": 146, "unresolved_imports": 348 }
```

`unresolved_imports` counts imports naming something outside the
project. Where that number dwarfs `resolved_imports`, the dependency
graph is too sparse for the cycle and reference findings to conclude
much, and they should be read as places to look rather than as
verdicts. Findings are ordered most-severe first.

Two ways to be wrong are worth naming, because both were real:

- A finding that describes a file the extractor never read is
  meaningless. `unreferenced_file` and `large_file` therefore consider
  only files in a language whose imports are actually extracted — a
  `.gitignore` "importing nothing" is not a finding.
- An import resolved to the *wrong* file is worse than one left
  unresolved, because it manufactures structure that is not there. The
  resolution rules above exist for that reason; before them, every
  cross-crate `use atlas_x::api::…` in this workspace resolved back
  into its own crate and reported two import cycles that did not
  exist.

### What a change reaches

`graph.changes` reads git for what has changed, then follows imports
**backwards** to every file that depends on them. It is the question
worth asking before editing, and before reviewing someone else's
branch: *what else is involved?* — answered without opening a file.

With no `against`, it reports everything not yet committed: staged,
unstaged and untracked. Pass a ref — `main`, a tag, a SHA — to ask what
a whole branch changes instead. `depth` bounds how many hops of
dependents to follow (default 3, max 10); on a real repository
everything reaches everything eventually through a shared error type,
and a report naming half the codebase is one nobody reads.

Each changed file carries `imported_by`, its declared `layer` where the
project declares layers, and `in_graph`. `impact` lists what was
reached with `distance`, 1 being a direct importer. `modules_touched`
and `layers_touched` cover everything involved, changed **or merely
reached** — a change confined to one layer that reaches three is
exactly what you want to know before starting.

**There is deliberately no risk score.** A single red/amber/green label
would hide which of several unrelated facts produced it, and invite
acting on the label instead of the thing.

Read `evidence` before relying on the radius:

- `changed_not_in_graph` — changed files the graph has never seen. A
  file created a moment ago has an *unknown* radius, not an empty one.
- `changed_since_indexed` — files edited since the last index. Reverse
  dependents survive that, because they come from *other* files'
  imports; but an import the change itself added or removed is
  invisible until a reindex. `graph.watch` keeps this at zero.

::: warning Requires git, and git must agree to read the repository
The working tree is read by running `git`, so that the answer matches
what you see in your own terminal. Two things follow.

**git must be installed** where the server runs. The Docker image
includes it.

**A repository owned by another user is refused**, with git's own
message: `detected dubious ownership`. This happens whenever a
container mounts a host checkout, because the files belong to the host
user and the server runs as `atlas`. Atlas does **not** pass
`-c safe.directory` on your behalf: that protection exists because a
repository owned by someone else carries configuration able to run
commands, and `git status` honours it. Add the exception yourself if
the repository is yours:

```yaml
environment:
  GIT_CONFIG_COUNT: "1"
  GIT_CONFIG_KEY_0: safe.directory
  GIT_CONFIG_VALUE_0: /repo
```
:::

### Declaring modules and layers

A project may describe its own structure in an **`atlas.layers.toml`**
at its root. The file is optional; a project without one is indexed,
searched and analysed exactly the same, it simply gets no layer
findings. **Atlas never infers layers from directory names.** A guessed
architecture reported at error severity is worse than no finding.

```toml
[[module]]
name = "auth"
path = "features/auth"
description = "identity, sessions, OAuth linking"

[[layer]]
name = "domain"
paths = ["features/*/src/internal/domain/**"]

[[layer]]
name = "application"
paths = ["features/*/src/internal/application/**"]
depends_on = ["domain", "infrastructure"]
```

**Modules** are curated subdivisions. They appear in `graph.overview`
so an agent can scope its next question instead of asking about a whole
repository. Curated rather than derived: every directory is a candidate
subdivision, and a list of all of them is a directory listing.

**Layers** are path globs plus what each may import from. Omitting
`depends_on` means depending on nothing, which is the strictest reading
and the right default for a domain layer. A layer may always import
from itself. Where two layers match the same file, the first
declaration wins.

The declaration is read when the project is **indexed**, so the watcher
keeps it current for free and findings never touch the filesystem.
Delete the file and the next index stops applying its rules — a project
is never judged against a rule it removed.

A declaration that cannot be used is reported as
`layers_file_unreadable` rather than ignored, and never fails indexing:
a typo in an optional file must not leave a project unsearchable.
Rejected loudly, too — an unknown dependency, a duplicate layer, a
layer with no paths, a pattern that will not compile. Each names the
declaration at fault, because a broken rule that quietly checks nothing
reports silence, and silence reads as compliance.

::: warning An absence of violations is not proof of compliance
The check sees only imports the graph resolved, between files a layer
claims. `layer_check_incomplete` reports both numbers, and names any
layer whose patterns matched no file at all — a pattern matching
nothing produces no violations and is otherwise indistinguishable from
a clean layer.
:::

**Declare what the project is, not what it should be.** This
repository's own first draft declared that the application layer may
not import infrastructure and produced 22 violations, because its
routers use their crate's store directly — a port was built only where
one earned its keep. A declaration stating an aspiration turns every
finding into noise and teaches whoever reads it to ignore the rest.

## Agent memory

Two buckets — see [State and privacy](/concepts/state-and-privacy) for
why, and [HTTP API](/reference/http-api#sync-api-sync) for how they
replicate.

| Tool | Parameters | |
|---|---|---|
| `memory.remember` | **scope**, **key**, **value**, project | Overwrites whatever was under that key. |
| `memory.recall` | **scope**, **key**, project | |
| `memory.list` | **scope**, project | One bucket. |
| `memory.forget` | **scope**, **key**, project | Errors if there was nothing there. |

`scope` is `"project"` or `"personal"`:

| `scope` | Requires | Meaning |
|---|---|---|
| `project` | `project` | What the agent knows **about a project** — Type 1, shared with everyone on it. |
| `personal` | nothing | What it knows **about a person** — Type 2, private to them. Always *your own* bucket, taken from your credential. |

There is no way to address another developer's personal memory, by
design. Supplying the wrong field is an error, not a default:
`"personal"` with a `project`, or with an `owner_id`, fails to parse. The same rule is a CHECK constraint in the
database, so a malformed row cannot be written by any path, not just by
this one.

**There is no call that returns both buckets.** Personal data is read
through a method with the owner filter baked in rather than left to the
caller — and a `list_all` would put that filter straight back in the
caller's hands.

Personal memory is keyed by owner alone rather than by owner and
project: personal state belongs to a developer working on *any* project.
What an agent learns about how you like to work is not a fact about the
repository you happened to be in when it learned it.
