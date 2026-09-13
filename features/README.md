# features/

> **Scope:** the shape every feature crate is held to, and how each rule
> is checked.
> **Status:** current · **Updated:** 2026-09-12

Every business area of Atlas is its own crate here — one folder per
area, a modular monolith split by feature rather than by layer.

| Crate | |
|---|---|
| `afg/` | Declarative workflow runs: node graphs, acceptance gates, retries |
| `auth/` | Accounts, session tokens, invitations, GitLab and GitHub OAuth |
| `graph/` | Project Map: indexing, search, findings, layer compliance |
| `mcp/` | The JSON-RPC surface agents call, and its tool schemas |
| `memory/` | Agent memory, split into project and personal buckets |
| `messaging/` | The inter-session coordination bus |
| `notes/` | A developer's own notes, personal by construction |
| `sessions/` | The session registry and its ownership rules |
| `sync/` | Replication in focus / auto / live modes |
| `terminal/` | The PTY pool, and restoring a missing workspace |
| `tracker/` | The external-tracker port, its adapters and local mirror |

## Shape

```
features/<area>/
├── Cargo.toml
├── migrations/                    the tables this feature owns
└── src/
    ├── api.rs                     public surface — everything other
    │                              crates may import, and nothing else
    ├── contract.rs                optional; see "Ports"
    └── internal/
        ├── domain/                types and rules. No I/O, no drivers
        ├── application/           use cases and HTTP routers
        └── infrastructure/        persistence, transport, external clients
```

Nothing else sits at `src/` level. A file outside `api.rs`, `lib.rs`,
`contract.rs` and `internal/` is a deviation and needs a reason in this
page before it exists.

*Checked by:* a person, at review. A new file at `src/` level is
visible in any diff.

---

## The layers, and the one direction they run

Inside a crate:

```
application  →  infrastructure  →  domain
 (use cases)      (adapters)       (types)
```

Each may depend on what is to its right. **Nothing depends leftward.**

- **`domain` depends on nothing.** Not on `sqlx`, not on `axum`, not on
  `reqwest`. The moment it knows a driver's name, that driver can no
  longer be replaced without touching the rules.
- **`infrastructure` depends only on `domain`.** An adapter implements
  the domain's types; it does not reach up into use cases.
- **`application` may use its crate's `infrastructure` directly.** No
  port is inserted to avoid this — see "Ports" below.

*Checked by:* `atlas.layers.toml` declares these paths and directions;
a violation is an ERROR-severity finding in `atlas findings --project
<p> --severity error`.

### `api.rs` and the binaries are outside this, deliberately

The layer check ranks file paths, and two different relations exist
that one rank cannot hold at once: **inside** a crate `api.rs`
re-exports from `internal/`, and **between** crates one crate's
`application` imports another crate's `api.rs`. Declaring `api.rs` as a
layer above `application` satisfies the first and turns the second into
violations — 61 of them, measured, every one a crate legally importing
another's public surface.

They need no layer rule because they have a stronger one: `internal/`
is private, so importing it from another crate is a **compile error**,
not a finding. `layer_check_incomplete` reporting those files as
uncovered is accurate and expected — the layer check is about the
inside of a crate. Read it that way rather than as a hole.

### Errors belong to the domain; conversions belong to the adapter

A crate declares one error enum in `internal/domain/error.rs`. The
conversion **into** it from a driver's error type lives in
`internal/infrastructure/error.rs`, next to the adapter that produces
it.

```rust
// domain/error.rs           — what can go wrong, in this crate's words
pub enum NotesError { NotFound, EmptyName, Storage(String) }

// infrastructure/error.rs   — and where SQL failures land in it
impl From<sqlx::Error> for NotesError { … }
```

Putting the `impl` in the domain is the tempting shortcut, because it
makes `?` work inside the store. It also makes the domain import
`sqlx`, which is exactly the dependency the first rule forbids.

*Checked by:* `grep -rl sqlx features/*/src/internal/domain/` must be
empty.

---

## Where things go

### An HTTP router lives in `application/`

A router is a driving adapter: it translates a request into a use case.
It sits beside the use cases it exposes, named `router.rs` — or
`<thing>_router.rs` when a crate has more than one.

*Checked by:* a router under `infrastructure/` produces an
`infrastructure → application` import, which the layer direction
rejects.

### A crate whose only behaviour is storage has no `application/`

`messaging`, `notes`, `sessions` and `terminal` have no
`application/`, and that is deliberate rather than unfinished. Their
store *is* the use case: write, read, list, delete, each a single call
with the owner scoping already in the SQL. A module that forwarded to
it would carry no information.

Add `application/` the moment there is a decision to make that the
store cannot make alone — a transaction spanning two tables, a rule
about ordering, a use case with a name of its own.

*Checked by:* a person, at review. The question to ask is "does this
module do anything a caller could not do by calling the store twice?"
If the answer is no, it should not exist.

### File names, and what a `mod.rs` may hold

A `mod.rs` **declares submodules and re-exports them**. Nothing else —
no types, no functions. A reader opening it is asking "what is in
here", and a 130-line index answers a different question.

The names that recur, and what each holds:

| File | Layer | |
|---|---|---|
| `error.rs` | domain | the crate's one error enum |
| `error.rs` | infrastructure | the `From` conversions into it |
| `model.rs` | domain | the crate's types, when there are few enough to share a file |
| `validation.rs` | domain | rules about what an input has to look like |
| `store.rs` | infrastructure | every SQL statement the crate runs |
| `router.rs` | application | the HTTP router |
| `runtime.rs` | application | the use-case object |

**A domain may be split by concept instead**, and should be once
`model.rs` stops being one subject. `tracker` is the example:
`issue.rs`, `filter.rs`, `project.rs`, `port.rs`, `plan_progress.rs`.
That is better than a `model.rs` holding five unrelated things, not a
deviation from it.

*Checked by:* a person, at review. `grep -c "^\s*\(pub \)\?fn \|^\s*impl " <mod.rs>`
returning anything above zero is the question to ask — the exception is
a submodule's own `mod.rs` defining that submodule's interface, as
`infrastructure/extract/mod.rs` does for the `Extractor` trait.

### One file, one reason to change

Length is not the test. `graph/infrastructure/store.rs` is 566 lines
and correct, because a schema change touches its writes, its queries
and its row mapping together — that is one reason to change. Splitting
it into reads and writes would force `NodeRow` and `node_from_row` into
a third module imported by both halves, which is worse than the file
was.

The test is whether two parts change for **different** reasons. The
binary failed it: composition changes when a crate is added, the
environment contract changes when an operator-facing variable is, and
the two had been in one file long enough that `run` needed helper
functions carved out of it purely to stay under the length lint. They
are now `main.rs` and `config.rs`.

*Checked by:* a person, at review. The question is not "is this long"
but "name the two things that would each make me edit this file".

### The use-case object is a `Runtime`

When a crate has an object over its store that runs its use cases, it
is `<Area>Runtime` — `AfgRuntime`, `TrackerRuntime`. One word for one
role.

Different words are for different things, and they earn it: `PtyPool`
pools processes, `SyncSupervisor` supervises a loop, `ChangeHub` fans
out events, `<Area>Store` is persistence. "Facade" is not on the list:
it names a pattern rather than what the object does.

*Checked by:* a person, at review, on any new type in this position.

---

## Ports

**A port exists where a second implementation exists.** `IssueTracker`
has four (GitLab, mirrored, fake, disabled). `Extractor` has three.
Everything else calls its store directly.

This is a rule about honesty, not about purity. A trait with one
implementation adds a layer of indirection to a call that had no
choice to make, and it makes the codebase look more substitutable than
it is.

### A port with more than one adapter exposes a contract suite

When adapters must be interchangeable, the thing that makes them so is
a shared test suite every one of them passes — not the trait signature,
which says nothing about behaviour. It lives in `src/contract.rs`,
beside `api.rs`, exported behind a `test-support` feature.

`tracker/src/contract.rs` is the example: `run_contract` and
`run_write_contract` run against GitLab (over `wiremock`), the fake and
the mirror, and each has its own test file that does nothing but call
them.

*Checked by:* a port with two adapters and no contract suite is a
review objection.

---

## Migrations

**A feature owns its tables.** Its migrations live inside it and never
touch another feature's schema.

Every crate migrates the **same physical SQLite file**, each with its
own migrator and `ignore_missing` set, so two crates' migration
versions must never collide — a duplicate version reads as a checksum
mismatch at startup and sends whoever debugs it in the wrong direction.

**The version is a UTC timestamp to the second**, taken when you create
the file:

```bash
date -u +%Y%m%d%H%M%S       # 20260912T… → 20260912143017
```

```
features/notes/migrations/20260912143017_add_pinned_column.sql
```

Two crates would have to create a migration in the same second to
collide, which needs no coordination to avoid. Earlier files use an
ad-hoc per-crate lane digit instead; they are left alone because
renaming an applied migration breaks every database that has it, and
none of them can collide with a timestamp taken from here on.

**Rebuild after adding one.** `sqlx::migrate!()` embeds migrations at
compile time, so `make build-server` before `just run`, or the binary
fails with `VersionMissing`.

*Checked by:* `ls features/*/migrations/*.sql | sed 's|.*/||;s/_.*//' |
sort | uniq -d` must print nothing.

---

## The boundary between crates

- **Only `api.rs` is importable from another crate.** Everything under
  `internal/` is private to the feature. A cross-crate import of an
  internal is a build failure, not a review comment.
- **`api.rs` re-exports and holds no logic.** It is the file another
  team reads to know what this crate offers, and the one whose change
  is a visible decision rather than an accident.
- **The binary composes, it does not decide.** `bin/atlas-server` wires
  these crates together and holds no business logic.

*Checked by:* `grep -rn "atlas_[a-z]*::internal" features/ bin/` must be
empty, and Rust's own visibility rules make it a compile error anyway.

---

## Adding a crate

1. `features/<area>/` with `Cargo.toml`, `src/lib.rs`, `src/api.rs`,
   `src/internal/{domain,infrastructure}/` — and `application/` only if
   the rule above says so.
2. Its migrations, versioned with a UTC timestamp.
3. Add it to the four crate lists in the `Dockerfile`. *Checked by:*
   `scripts/check-dockerfile-crates.sh`, which the gate runs.
4. Add it to the table at the top of this page and, if it is a
   subdivision worth scoping the Project Map to, a `[[module]]` entry
   in `atlas.layers.toml`.
5. Compose it in `bin/atlas-server`.

The gate is `just check`: format, the Dockerfile guard, clippy with
`-D warnings` on stable **and** the pinned MSRV, the test suite with
`--locked`, the image build, and the documentation site. A lint that
fires on only one toolchain fires in CI instead of on your machine,
which is why both run.
