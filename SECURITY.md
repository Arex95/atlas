# Security

## Reporting a vulnerability

Report privately, not as a public issue. Use GitHub's **Report a
vulnerability** button under the Security tab, which opens a private
advisory.

Please include what you did, what happened, and what you expected. A
minimal reproduction is worth more than a description. If you are not
sure whether something counts, report it — deciding that is not your
job.

Please do not run automated scanners against a server you do not
operate, and do not test against anyone else's deployment.

## The one boundary

**The only boundary against an agent that behaves badly is the
operating system.** Atlas gives agents real terminals on a real
machine — that is the product — and nothing inside the Atlas process
contains an agent that already has one. Bound the blast radius where
bounds are real: a container, a user account, a network policy.

Everything below is what Atlas enforces *above* that line. The full
statement, including what is deliberately not enforced, is in
[Trust model](https://arex95.github.io/atlas/concepts/trust-model).

## What Atlas is designed to protect

Atlas is **self-hosted**, and a deployment holds either one developer
or a team. Where several developers share a server, the separations
below hold between them; what does not is that they share a machine —
see the trust model.

- **One developer's personal state from another's.** Personal rows are
  read through a path that takes an owner and filters by it; there is
  no variant without the filter, no admin bypass, and no reporting
  query that includes them. A client pushing a row that claims another
  owner has it stored under its own instead — there is no code path
  that writes into someone else's data.
- **Credentials it is not asked to hold.** Tracker tokens are read from
  a file you already keep and are never persisted or copied. Sync
  configuration — a remote URL and bearer for another server — lives in
  process memory only and is gone on restart.
- **Account enumeration.** A wrong password, an unknown email and a
  disabled account return identical responses, byte for byte, and the
  unknown-email path burns comparable time so the difference is not
  observable by clock either. A resource that exists but is not yours
  answers `404`, not `403`.
- **Session revocation.** Disabling an account invalidates every path
  at once — password login, OAuth login, and tokens issued before it
  was disabled. There is no window in which an old bearer still works.
- **Identity on the agent surface.** Every MCP call resolves to an
  owner from the credential it presented. No tool takes an `owner_id`,
  and sending one is an error rather than being ignored. A caller may
  spawn, write to, read from and close terminals, read session
  inboxes, and start or answer workflow runs **only for sessions it
  owns**; somebody else's reads as not-found, so ids cannot be probed
  for.
- **Where a workflow's gates run.** A gate executes shell commands in
  the directory the workflow was registered against. No caller names
  that directory at the moment the commands run.

## What it does not protect, today

These are known and documented, not oversights. Understand them before
exposing a deployment.

**No per-tool scoping.** A credential that identifies a session may
call every tool. Identity decides *whose* data a call reaches, not
which tools it may use. There is no per-tool or per-project allowlist,
and one would not be a boundary anyway while the same agent holds a
terminal.

Bind the MCP endpoint to loopback on a workstation
(`ATLAS_LISTEN_ADDR=127.0.0.1:4000`) and put a team server behind a
reverse proxy you control. It authenticates every request, but it is
not hardened against the open internet.

**Terminals run real commands.** `terminal.write` sends raw stdin to a
shell running as the server's user, in your working tree. That is the
feature. A caller may only drive terminals for sessions it owns, but
within those it can run anything that user can.

**Workflow gates run shell commands** from a workflow spec. A workflow
file is executable content — review one before registering it, with the
same care you would a `Makefile` from a stranger.

**`ATLAS_MCP_TOKEN_ALLOW_UNSET` removes authentication entirely.** It
exists for local experiments. Setting it on anything reachable leaves
every tool open to whatever can reach the port.

**No rate limiting.** Neither the auth endpoints nor the MCP endpoint
limit attempts. Put a reverse proxy in front of anything public.

**SQLite is not encrypted at rest.** The file is created `0600` inside
a `0700` directory, so a second account on the same machine cannot read
it — but whoever administers the host can. Passwords and session tokens
are stored hashed, so what the file holds is not usable if stolen;
sessions, messages and agent memory are in the clear. Disk encryption
is the layer that answers a stolen machine, and it belongs to the
operating system.

## Deploying safely

- Terminate TLS at a reverse proxy; session tokens travel on every
  request
- Publish only the proxy's port — never the container's directly
- Bind loopback on a workstation
- Give the tracker token the narrowest scope that works: reading issues
  needs read, `create_issue` and `update_status` need write, nothing
  needs admin
- Never pass a secret as a Docker build argument — build arguments end
  up in the image history, readable by anyone who can pull it
- Create the first account immediately after the server is reachable.
  `register` works exactly once, and you want it to be you

## Supported versions

Pre-1.0 and moving quickly. Fixes land on `main`; there are no
backports to older tags.
