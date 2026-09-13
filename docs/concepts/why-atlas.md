# Why Atlas

Working with one AI agent is a chat window. Working with four is a
logistics problem, and the tools have not caught up.

You open a terminal per agent and lose track of which is which. You
paste the same context into each because none of them can see what the
others learned. You ask one to wait for another and then poll it
yourself, because "wait" means you watching. You close the laptop and
the arrangement evaporates — the branches survive, the *setup* does
not. And when an agent reports a task done, the only thing standing
between "done" and merged is you reading the diff.

None of that is a model problem. It is a coordination problem, and it
does not get better as the models improve — four capable agents with no
shared state is a worse mess than four mediocre ones, because they get
further before you notice they diverged.

Atlas is the layer underneath that arrangement.

## Two claims, and they only work together

**One: a report is not evidence.** An agent that says the task is done
is producing text, and text is what it produces when the task failed
too. A model is a probability distribution over tokens; it is not a
system that returns an error when it is wrong. You cannot prompt that
property into existence, and you cannot train it in either — the labs
that build these models shape *tendencies*, and a tendency is not a
guarantee. The only thing that holds is a check run by the system
receiving the work.

So in Atlas the acceptance criteria are executed by the Rust runtime,
never self-reported. A node advances when a command exits zero or a
payload matches a schema. When it does not, the reason is injected back
into the agent's context and the node retries. Nobody is trusted on
their own report, including you.

**Two: the rules belong to the project, not to whoever is at the
keyboard.** Every developer on a team has quietly invented their own
way of working with their agent — what it is allowed to touch, when it
may move on, what counts as finished. Four people, four methodologies,
none of them written down, all of them enforced by attention. That is
the state of practice, and it does not survive a second person.

So a workflow in Atlas is a graph declared in the repository. A node
names its acceptance criteria, scopes the tools its agent may call, and
is dispatched on its own — the agent is told its step, not the ones
after it. The file is re-read from disk every time a run starts, so the
rules come from the repository rather than from a copy somebody
registered once. That is what makes it a team's way of working rather
than one developer's discipline: your teammate pulls, and their next
run is held to what you committed.

Both halves exist on the market, and separately they are mature:
`AGENTS.md` is in sixty thousand repositories and tells agents how a
project works, with no way to check that they listened; CI and policy
engines check relentlessly and tell nobody what to do. What is missing
is the binding — see
[Instructions and checks](/concepts/instructions-and-checks) for who
does what, with sources.

Neither claim carries the product alone. A gate with no shared
declaration is a personal habit with extra ceremony. A declaration with
no runtime check is a document everyone agreed to and nobody executes —
which is every methodology that ever failed. Atlas is the two at once:
**a way of working the team declares, and a runtime that enforces it
instead of asking.**

## What it decides

- **A session is a first-class thing.** Who is working on what, on
  which branch, in which directory — recorded, not implied by which
  terminal tab you left open. It survives the process and the machine.
- **Terminals are real.** A PTY, with a shell in it, that you and an
  agent can both drive. Not a transcript of one, not a request/response
  API pretending to be a shell.
- **Agents talk to each other.** A message bus with broadcasts and
  direct messages, so "tell the other one it can start" is a call
  rather than something you relay.
- **A finished task is one that passed a check.** See above; it is the
  reason the rest exists.
- **The caller never asserts what the server can derive.** Who you are
  comes from the credential the request arrived with, never from a
  field in it. An agent cannot name a session other than its own, an
  owner other than itself, or a run it did not start. This is a rule
  rather than a series of fixes: every place the shape was violated was
  a way to reach somebody else's work, and treating it as one class is
  what closes the ones nobody has found yet.
- **Issues stay where they already are.** Atlas has no tasks table. It
  reads and writes your GitLab or GitHub, with a local mirror so reads
  are fast and writes are real.
- **What is yours stays yours.** Personal state replicates to a server
  *you* host, marked private to you — and it stays private there, with
  no admin view and no reporting query that quietly includes it.

## Where the boundary actually is

It is worth being exact, because the rest of this page is worth less if
this part is oversold.

**A tool scope is not containment.** A node that declares three tools
is telling its agent what this step is about, and stopping it from
wandering into a fourth by accident. It is not stopping an agent that
has a real terminal and can do anything you can do by typing. Accidents
between agents are the common failure; a scope addresses accidents, and
says so.

**The operating system is the boundary.** Atlas runs as you, on your
machine, with your permissions. Anything stronger — a sandbox, a
container, a separate user — is something you put around it, and Atlas
will not pretend to be a substitute for it.

**A webhook's token proves a sender, not a payload.** So Atlas re-reads
the issue from the tracker rather than believing what arrived.

**Dispatching one node is not context isolation.** The task an agent
receives describes its own step and no other, which keeps it from
working ahead of the graph. It is not a wall: the workflow file is in
the repository the agent can read. What it buys is that the agent is
not *handed* the next three steps and invited to take them.

Every one of these is written the same way in the code that implements
it. A boundary you describe accurately is one people can build on; one
you describe generously is one they find out about later.

## What it leaves alone

**No interface.** The frontend is a separate repository. This is the
daemon, its HTTP API and its MCP surface.

**No task database.** Adding one means two sources of truth for the
same issue and a sync bug for every field. Your tracker already is the
source of truth.

**No credential store.** Atlas reads your tracker token from the file
you already keep it in, the way SSH reads a key. It never persists one,
which means there is no vault to breach and no export to leak.

**No inbound command channel.** Atlas is not driven from a messaging
app. Anyone who takes over such an account inherits every irreversible
action the agent can perform, on a machine that in practice holds the
developer's real work and real credentials. That is not a risk to
mitigate with confirmations; it is a door that stays shut.

**No cloud.** Not "self-hosted first, hosted later" — single-tenant and
self-hosted is the design. A hosted tier would have to answer whose
personal state it holds, and the honest answer would stop being "only
yours".

**No model.** Atlas orchestrates the agent CLIs you already use. It has
no opinion about which, and it does not talk to model providers.

## What it is not

It is not an agent framework. It does not decide how an agent thinks,
prompt it, or wrap its reasoning. Agents arrive as CLIs that speak MCP
and leave as CLIs that speak MCP.

It is not a CI system. Acceptance criteria are a gate on a node, run on
your machine against your working tree, not a pipeline with a build
matrix.

And it is not a chat archive. Conversation stays in whichever CLI you
ran; Atlas keeps sessions, not transcripts. That is deliberate — the
thing worth persisting is the arrangement, not the dialogue.
