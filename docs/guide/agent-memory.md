# Agent memory

An agent that forgets everything between sessions makes you the
memory — re-explaining the build command, the naming convention, the
one service that needs a flag, every time.

Atlas gives it two places to write things down, and they have opposite
sharing rules:

| Scope | About | Reaches |
|---|---|---|
| `project` | the codebase | everyone working on that project |
| `personal` | you | only you, on your own machines |

The split is not a suggestion. It is a column on the row and a
constraint in the database, and it decides where the value goes when it
replicates.

## Remember something

```jsonc
// About the project — shared
{ "scope": "project", "project": "your-org/your-project",
  "key": "release-build", "value": "cargo build --release --locked" }

// About the person — private
{ "scope": "personal", "owner_id": "01M1TD4YJR7X1JTK29NTBB1AD3",
  "key": "review-style", "value": { "wants": "the failing case first" } }
```

`value` is arbitrary JSON — a string, an object, a list. Writing a key
that already exists **overwrites** it; there is no append, and no
history.

`scope` decides which other field you must supply, and supplying the
wrong one is an error rather than a default. `"personal"` with a
`project` and no `owner_id` does not fall back to anything — it fails
to parse.

## Read it back

```jsonc
// memory.recall — one key
{ "scope": "project", "project": "your-org/your-project", "key": "release-build" }

// memory.list — a whole bucket
{ "scope": "personal", "owner_id": "01M1TD4…" }

// memory.forget — errors if there was nothing there
{ "scope": "project", "project": "your-org/your-project", "key": "release-build" }
```

**There is no call that returns both buckets.** That is deliberate:
reading personal data goes through a path that takes an owner and
filters by it, and a `list_all` would hand that filter back to whoever
is calling.

## Choosing a scope

The test is not "is this sensitive". It is **what is this a fact
about**.

> *"The release build needs `--locked`"* — a fact about the repository.
> True for whoever works on it next. **Project.**

> *"This developer wants the failing case explained before the fix"* —
> a fact about a person. Follows them to another project, and is nobody
> else's. **Personal.**

The awkward middle case is a preference that only applies to one
codebase — "I like reviewing this repo's migrations first". It is about
you, so it is personal. Personal memory is keyed by owner **alone**,
not by owner and project, so it will surface when you work elsewhere.
That is the tradeoff of the simpler rule, and it is the right way round:
better a preference that shows up somewhere harmless than one that
leaks to the team.

The same key can exist in both buckets without colliding. They are
separate spaces.

## What happens when it replicates

In Mode 1 nothing leaves your machine and the distinction is
bookkeeping.

Point it at a [team server](/guide/team-mode) and the two buckets
diverge:

- **Project memory reaches everyone.** That is what makes it worth
  writing — the next person's agent already knows the build command.
- **Personal memory reaches the server and stops there**, stored marked
  private to you. It goes so your second laptop has it and a dead disk
  does not lose it, not so anyone can read it.

```jsonc
// sync.memory_now — one pass, both buckets
{ "remote_url": "https://atlas.example.com/api/sync/",
  "bearer_token": "b0d663ad…", "owner_id": "01M1TD4…" }
```

A client pushing a personal row that claims a *different* owner does
not get an error — the server files it under the authenticated caller
instead. There is no code path that writes into another developer's
memory at all, which is a stronger guarantee than a check that could be
forgotten at one call site.

Conflicts between machines resolve by last write, keyed on
`(project, key)` or `(owner_id, key)` rather than on the row id — two
machines can learn the same fact independently and hold different ids
for it.

## A caveat worth knowing

`owner_id` is a parameter an agent passes, and the MCP surface does not
identify the calling session — so an agent on your machine can name any
owner it likes. Every agent there shares one token and Atlas cannot
tell them apart.

That is a real limit of the current design, not a claim about it. The
person-facing HTTP routes do not have it: they take identity from a
session token, which is why the *server* forces personal rows under the
authenticated owner regardless of what was sent. Within one machine,
treat scope as bookkeeping rather than as a boundary.
