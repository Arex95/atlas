# Trust model

> **Scope:** what Atlas defends against, what it does not, and which of
> its guarantees are boundaries rather than warnings.
> **Status:** current · **Updated:** 2026-09-09

Atlas runs agents that hold real terminals on a real machine. That is
the product, not an oversight, and it decides everything below. This
page says plainly what follows from it, because a system that is vague
about its limits gets trusted for things it never claimed.

## The one boundary

**The only boundary against an agent that behaves badly is the
operating system.** Nothing inside the Atlas process contains an agent
that already has a terminal.

An agent working in a session can run any command its user could. It
can read the files that user can read and write the files that user can
write. Atlas gives it a PTY on purpose — that is how it does the work
you asked for.

So if you need an agent's blast radius bounded, bound it where bounds
are real: run the server in a container with only the mounts it needs,
as a user with only the access it needs, on a network that reaches only
what it must. Atlas's own Docker image is that posture. Everything
Atlas does above the operating system is organisation, not containment.

## What Atlas does enforce

These are **real** guarantees, structural rather than advisory, and
each one is checked somewhere the caller cannot reach around.

**Identity comes from the credential, never from an argument.** Every
MCP call resolves to an owner from the token it presented. No tool
accepts an `owner_id`, and sending one is an error rather than being
ignored. See [Identity](/reference/mcp-tools#identity).

**You act only on sessions you own.** Spawning, writing to, reading
from or closing a terminal, reading an inbox addressed by session id,
starting a workflow run, submitting its result — each checks ownership,
and somebody else's session is reported as *not found* rather than as
forbidden, so ids cannot be probed for. Scoped by owner rather than by
session, deliberately: Atlas exists to let one developer's agents drive
one another.

**Personal state is private to its owner — from other users of the
machine, and from anything the server can be asked.** Type 2 rows
replicate to a team server marked private, and the queries that read
them take an owner and filter by it. There is no administrative view
that includes them, and no reporting query that crosses them. The
database file is created `0600` and its directory `0700`, so a second
account on the same machine cannot read it either.

**It is not private from whoever operates the server.** The file is not
encrypted at rest: anyone who can read it can read everything in it.
That is a deliberate limit rather than a gap to close. Encrypting it
would put the key on the same machine as the process that must decrypt
it, which moves the problem rather than solving it — and it would leave
your source code, sitting in plaintext beside it, no safer. Full-disk
encryption is the layer that answers a stolen machine, and it belongs
to the operating system.

So the honest statement is: **the server operator is someone you
trust.** What the database no longer holds is anything that works if
stolen — passwords and tokens are stored hashed, and the one
third-party credential it used to keep is gone.

**In transit it is always encrypted.** Atlas refuses to sync to a
remote host over plain HTTP: every request carries a bearer token and a
body of notes, memory and messages, and accepting `http://` in silence
made the unsafe choice the quiet default. Loopback is exempt, because
that traffic never reaches a network.

**A workflow advances only through its gates.** An acceptance criterion
is not advice to the agent — the run does not move until the criterion
passes, and the agent cannot mark its own work done. Gates run in the
directory the workflow was registered against; no caller names it.

**A malformed request is refused, not repaired.** Request shapes reject
unknown fields, so a call that names something Atlas does not accept
fails loudly instead of being silently reinterpreted. The same rule
holds at the database: constraints make invalid rows impossible rather
than merely unlikely.

## What Atlas does not enforce

Naming these is the point of the page. Each is useful; none is a
boundary, and none should be reported as a security failure.

**Layer compliance is a measurement, not a gate.** `graph.findings`
reports imports that cross a boundary your `atlas.layers.toml`
declares closed. It reports them *after* the fact; nothing prevents the
import, and the check sees only imports the graph resolved. Its report
publishes its own coverage for that reason.

**Project Map findings are heuristics.** Unreferenced files, hubs, long
files, undocumented directories — each is a prompt to look, not a
verdict. The `mentions` predicate in particular connects files that
merely share a word.

**Change impact is bounded by what the graph resolved.** An import the
resolver could not follow is invisible to it, so an empty blast radius
means "nothing found", never "nothing exists".

**A workflow node's tool scope limits accidents, not capability.** A
node may name the tools its agent should use, and others are refused —
but that agent holds a terminal, so the scope bounds what it reaches
*through Atlas*, not what it can do. It is worth having because
accidents are common and this is the one surface Atlas controls; it is
not worth reporting as a vulnerability when it is bypassed by typing.

**Atlas does not screen what an agent writes.** There is no filter over
model output, no scanner over commands before they run. Such a filter
would be a heuristic over an attacker-influenced string, and treating
it as protection is exactly the mistake this page exists to prevent.

**Atlas does not custody your credentials.** It does not hold your git
or tracker credentials on your behalf; it uses what is already on the
machine, and reports when they are missing rather than resolving it.

## Multi-user, and what it does and does not mean

A team server holds several developers' data, and this is where Atlas
differs from a personal agent — a personal agent has one trust envelope
and can treat everything inside it as equivalent. Atlas cannot.

**What holds between users on one server:** the ownership rules above.
One developer's agents cannot drive another's terminals, read their
session inboxes, advance their workflow runs, or read their personal
memory.

**What does not hold:** they share a machine. Two developers' sessions
are processes under the same operating system, with whatever access
that user account has. Atlas separates *its own* surfaces by owner; it
does not sandbox one developer's shell from another's files. If that
separation matters, give them separate machines or separate
containers — that is an operating-system boundary, and only the
operating system provides it.

**Project state is shared on purpose.** Anything Type 1 — project
memory, the graph, workflows, broadcasts — is visible to everyone on
that project. That is what makes it a team server.

## Reporting something

A report is most useful when it says which of the guarantees above it
crosses. Something that defeats an item under *What Atlas does not
enforce* is welcome as an ordinary issue — those are known limits, and
sharpening them is worth doing — but it is not a vulnerability, because
nothing claimed otherwise.
