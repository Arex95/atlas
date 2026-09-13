# Changelog

Notable changes to Atlas. Format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions
follow [semantic versioning](https://semver.org/spec/v2.0.0.html) once
there is a release to version.

Pre-1.0: nothing is released yet, so everything below is unreleased and
the surfaces described in the documentation may still change.

## [Unreleased]

### Tracker (Mode 3)

- Read-only `IssueTracker` port with a GitLab adapter, behind a
  hexagonal boundary so a second tracker is an adapter rather than a
  rewrite
- Local read mirror with delta pulls; writes always go to the real
  tracker and refresh the mirror immediately, so a write is never
  visible locally before it is real
- Write operations: create an issue, change its status, close it
- `plan_progress`, counting acceptance-criteria checkboxes rather than
  issues — the number that moves as work actually gets done
- Milestones: carried on every issue, mirrored, and selectable in the
  issue filter, so `plan_progress` can answer about one roadmap instead
  of about everything open
- Nested GitLab subgroups in project references
  (`group/subgroup/project`)
- Webhook receiver at `POST /api/webhooks/tracker/gitlab`, so a change
  in the tracker reaches the mirror in seconds instead of at the next
  poll. Mounted only when `ATLAS_TRACKER_WEBHOOK_SECRET` is set, and it
  re-fetches the issue rather than believing the payload — GitLab's
  token echo authenticates the sender, not the body

### Agents and coordination

- A workflow registered from `source_path` is re-read from disk every
  time a run starts, so the committed file is the source of truth
  rather than a copy taken at registration. A teammate who pulls a
  changed workflow is held to it on their next run without
  re-registering. A run pins the spec it started with, so an edit never
  rewrites the criteria an agent is already being judged against, and a
  file that will not parse fails the start instead of falling back to
  the last version that worked. Convention: `.atlas/workflows/`

- MCP endpoint at `POST /api/mcp` — JSON-RPC 2.0, bearer-authenticated
  in constant time, 31 tools
- `connect-agent` script printing ready-to-paste configuration for
  Claude Code, Codex CLI and Cursor
- CLI shortcuts for issues, progress, workflow runs and project-map
  findings — `atlas issues --board` renders a board as columns, and
  `atlas run` prints a run's node-event trail. Enough to exercise
  Atlas from a terminal without reading raw JSON
- Inter-session messaging: broadcasts to a project, direct messages to
  a session, poll-based inboxes
- Agent memory in two buckets — about a project (shared) and about a
  person (private) — with the classification enforced by a database
  constraint rather than by convention

### Project Map

- A content-centric index of a project: files and markdown sections as
  nodes, `contains` / `links` / `mentions` / `imports` as edges
- Gitignore-aware discovery that also refuses `target/`,
  `node_modules/`, lockfiles, binaries and oversized files whatever the
  ignore rules say
- Extraction behind a port, so a precise per-language extractor is an
  adapter rather than a rewrite
- Resolution that leaves an unresolvable target NULL rather than
  pointing it at a guess: an import naming a symbol resolves to the
  file defining it by dropping trailing segments until one names a
  real file, an import whose root segment names no directory in the
  project is treated as third-party and never resolved locally, module paths are matched by suffix and narrowed by
  the segments the suffix dropped, and proximity to the importer
  breaks a tie only when those name nothing
- Nine MCP tools for agents to navigate without reading every file
- `graph.changes`: what the current changes reach. Reads git for what
  changed — uncommitted by default, or a whole branch against a ref —
  then follows imports backwards to every dependent, naming the
  declared modules and layers involved whether the change is in them or
  merely reaches them. No risk score, deliberately; and the report says
  which changed files the graph has never seen and which were edited
  since it was built, because a radius computed from a graph that
  predates the edit answers a slightly older question
- Optional `atlas.layers.toml` in the project being described, declaring
  its curated modules and its architectural layers. Modules surface in
  `graph.overview` so an agent can scope a question; layers produce
  `layer_violation` — the only error-severity finding, and the only one
  measured against a rule the project wrote down rather than a
  heuristic. Layers are never inferred, an unusable declaration is
  reported rather than ignored and never fails indexing, and every
  report states how much of the project the check could see, because an
  absence of violations from a partial check is not compliance
- `graph.findings`: located observations — import cycles, unreferenced
  files, widely-depended-on files, large files, undocumented
  directories — computed only from resolved `imports` and published
  with the evidence they were drawn from. Deliberately no composite
  score: the graph's one heuristic predicate outnumbers its real
  dependency edges by roughly six to one, and a number derived from
  that would launder a guess
- A file watcher that keeps the graph current: build output filtered
  before it can trigger anything, bursts debounced into one pass, and
  watches held in memory only

### Security

- **"Continue with GitHub"**, alongside GitLab and under the same
  link-only rule: consent attaches an identity to an existing account
  or fails, and never creates one.

  GitHub's profile email is deliberately ignored. It is not guaranteed
  to be verified, and an account is found by matching that address — so
  believing it would let anyone claim an account by adding its address
  to their own GitHub profile. The address comes from the account's
  email list instead, and only an entry that is both primary and
  verified is accepted, with no fallback.

  The provider is now a typed value rather than a string spliced into
  SQL, so a third provider is an adapter and a variant rather than
  another copy of the login path.

- **A terminal is unreachable to anyone but its owner.** `terminal.write`
  took any session id and wrote to that session's PTY, which is
  arbitrary command execution in another developer's shell; `spawn`,
  `read_output`, `close` and `restore` were equally unchecked. Verified
  against a real container: one session wrote a command into another's
  shell and it ran, and its output read back. A PTY now records its
  owner when it is spawned, so every later call is authorised in memory
  without a database round trip per keystroke.

- **Reading an inbox addressed by a session id, and starting a workflow
  run on a session, both require owning that session.** Starting a run
  dispatches a `task` into the session's inbox, so an unscoped check let
  any caller inject work into another developer's agent.

  All three are scoped by **owner**, not by session: Atlas exists to let
  one developer's agents drive one another, so a session's token reaches
  that developer's other sessions and nobody else's.

- **A workflow run can only be advanced by the session its node was
  dispatched to.** `afg.submit_task_result` used to take `run_id`,
  `node_id` and `project_root` and check none of them against anything:
  any credential could complete any run, and the `project_root` it named
  became the working directory of the shell that run's acceptance gates
  execute in. Verified against a real container — an unrelated session
  completed another's run and chose where `bash` ran.

  The run now records the session each node was dispatched to, retries
  included, and a result from anyone else is a not-found rather than a
  forbidden, so a caller cannot learn that a run it may not touch
  exists. `project_root` is recorded once, when the workflow is
  registered, and is no longer a parameter — the choice is removed
  rather than checked.

- **MCP calls now carry an identity the server establishes, not one the
  caller asserts.** `sessions.create` mints a session token, shown once
  and stored only as a SHA-256 hash; the MCP endpoint resolves it to a
  session and an owner, and every tool touching personal state reads the
  owner from there. `owner_id` is gone as a parameter and sending one is
  a parameter error — rejected rather than ignored, so a client written
  against the older contract fails loudly instead of quietly acting as
  somebody it did not name.

  Before this, MCP authenticated with a single shared token and believed
  whatever `owner_id` an argument carried. Holding that token was enough
  to read, list and delete another developer's personal memory by naming
  them, which the state model rules out. The documentation had
  recorded it as a known limitation; it is now closed and covered by a
  test that replays the exact calls.

  The shared `ATLAS_MCP_TOKEN` still works and resolves to a reserved
  owner, `local`. A standalone install has one developer and no
  accounts, so it needs nothing else; a team server issues real user ids,
  which `local` is never one of, so the shared token reaches nobody's
  data there. No mode switch, one rule.

### Personal notes

- **`atlas-notes`**, a developer's own notes (personal state).
  Personal *by construction* rather than by classification: every note
  has an owner and there is no shared variant, so a feature asking to
  "share my scratch notes with the team" has nothing to call.
- Addressed by a name the developer chooses, so a write is idempotent
  and a terminal can reach a note without copying an id around. **A
  scratchpad is the note called `scratchpad`** — one concept, not two
  tables for the same idea.
- Four MCP tools, none of which takes an owner or a scope. The only
  thing a caller could name is somebody else, which is precisely what
  must not be nameable.
- Carries `list_since` and `upsert_for_sync` from the start, so
  extending replication to it is wiring rather than new design.
- **Notes replicate**, in all three sync modes: `sync.notes_now`
  for one pass, and the `auto` and `live` supervisors carry them
  alongside sessions and memory. `POST /api/sync/notes/push` and
  `GET /api/sync/notes/pull` on the server, `notes` added to the
  `live` event stream.
- Both ends force the owner independently. The server stores what
  arrives under the authenticated caller; the client stores what comes
  back under its own owner, so a remote that is wrong, compromised, or
  simply older cannot make a machine file another developer's notes as
  its own.

### CLI

- **A spawned terminal is handed its own credential.**
  `ATLAS_SESSION_TOKEN`, `ATLAS_SESSION_ID` and `ATLAS_URL` are set, so
  an agent working in a session can call Atlas back and every call is
  attributable to that session rather than to a shared secret. The token
  is minted per terminal, because only the hash of the one
  `sessions.create` returned is stored.
- `ATLAS_MCP_TOKEN` is **removed** from that environment rather than
  merely not set: the PTY inherits the server's environment, so the
  shared token was reaching every terminal — verified against a running
  container before and after.
- `ATLAS_SERVER_URL` overrides what a terminal is told to call back on;
  it is derived from the listen address otherwise.


- **`atlas`**, a client of the same MCP surface agents use rather than a
  second way in — so anything it can do an agent can do, and a change
  that breaks agents breaks this too. Shipped in the container image.
- `atlas call <tool> key=value` reaches **every** tool, including ones
  added after the binary was built. The rest are shortcuts for what gets
  typed often and add no capability.
- Identity comes from `ATLAS_SESSION_TOKEN`, falling back to
  `ATLAS_MCP_TOKEN`. Deliberately no `--token` flag: a flag invites
  passing somebody else's.
- A table when stdout is a terminal, JSON when it is not. A tool's own
  failure exits non-zero, rather than being reported as the successful
  JSON-RPC response it technically arrives as.

### Workflow runs

- **A node may scope the tools its agent calls** (`allowedTools`, exact
  names or `family.*`). While that node is dispatched to a session, any
  other tool it calls is refused. `afg.submit_task_result` and
  `afg.get_run` are always permitted, because a scope that can deadlock
  the run it scopes would be worse than none. Omitting it scopes
  nothing, so existing workflows are unchanged.
- **It is not containment**, and the refusal says so in its own
  message. The agent holds a terminal and can do anything its user can;
  this limits accidents on the one surface Atlas controls. It is
  documented that way in the reference and in the trust model, so it
  cannot be mistaken for a boundary later.

### Sessions and terminals

- **`resume_command`**: what to run when a session's terminal is
  spawned, so an agent CLI rehydrates its own context. Atlas does not
  persist scrollback — a replayed buffer gives a human something to read
  and the model nothing — so the CLI is asked to restore itself instead.
  Runs only on a real spawn, never on the idempotent repeat, so polling
  `terminal.spawn` cannot restart the agent under the caller.

  It does **not** replicate. Everything else about a session travels to
  a team server and back; a command that runs unattended does not,
  because replicating it would let a row arriving over the network
  arrange for a command to run on a developer's machine.


- Session registry: who is working on what, on which branch, in which
  directory
- Owner-scoped access throughout; a session belonging to someone else
  is indistinguishable from one that does not exist
- Real PTYs through `portable-pty`: spawn, write, read, close
- `terminal.restore`, cloning a session's workspace on a machine that
  has never seen it, and reporting missing git credentials as their own
  distinct error rather than a generic failure

### Workflow runs

- Declarative YAML workflows: node graphs with dependencies,
  dispatched over the messaging bus
- Acceptance gates — a shell command's exit code and output, or a
  JSON-Schema check of the agent's reported payload
- Retries that inject the failure reason back into the agent's context
- A live run view over SSE that replays a run's history and then
  continues on the same connection, closing itself when the run ends

### Team server (Mode 2)

- Local accounts: one-time bootstrap, Argon2id, session tokens stored
  only as hashes
- Invitations with a server-generated temporary password the inviter
  never learns, and which must be replaced before the account can do
  anything else
- Account disablement that revokes every path at once, including
  tokens already issued
- "Continue with GitLab" — links to an existing account, never creates
  one
- Sync in three modes: `focus` (manual, the default), `auto` (polled),
  `live` (SSE, reconnecting on its own)
- Personal state replicating to the team server marked private to its
  owner, with the server forcing ownership rather than trusting the
  client

### Operations

- Health check that runs a real database query rather than answering as
  soon as the process is up
- Graceful shutdown on SIGINT and SIGTERM
- Configuration entirely from the environment; startup refuses a bad
  configuration instead of coming up degraded, and names the variable
  that fixes it
- Multi-stage container image running as a non-root user
- Documentation site with guide, concepts and a complete reference
