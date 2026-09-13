# State and privacy

Every row Atlas persists is one of three types, and the type is a
column on the row rather than a convention in the code that writes it.

| Type | What it is | Where it goes |
|---|---|---|
| **1 — project** | Belongs to a project, meant for everyone on it | Replicates freely |
| **2 — personal** | Belongs to one developer, across every project | Replicates **marked private to its owner** |
| **3 — ephemeral** | Live PTY output, transient notifications | Never persisted at all |

The classification exists because "workflows the team agreed on", "my
scratch notes" and "what this shell printed two seconds ago" have
nothing in common except that code was tempted to store them the same
way. Retrofitting per-row visibility afterwards is expensive and
error-prone, so it is a design axis from the schema up.

## Type 2 replicates, and stays private

The obvious reading of "personal" is "never leaves your machine". That
reading loses a real thing: move to a new laptop, or lose a disk, and
"it never left" means "it is gone".

So personal state does go to the team server — **marked private to its
owner**, and the server enforces that rather than promising it:

- Every read of Type 2 data goes through a method that takes an owner
  and filters by it. There is no variant without the filter.
- There is no admin bypass, and no reporting query that includes
  private rows.
- Pushing a row that claims a different owner does not fail with an
  error — it is **stored under the authenticated caller instead**. A
  client that lies writes into its own bucket, so there is no code path
  that touches another developer's data at all.

The last one is the difference between "we check" and "it cannot
happen". A check has a call site that might be forgotten; a design with
no such path does not.

## Agent memory is the clearest example

An agent accumulates two different kinds of knowledge, and they have
opposite sharing rules:

```jsonc
// About the project — Type 1, shared with everyone on it
{ "scope": "project", "project": "your-org/your-project",
  "key": "build", "value": "cargo build --release" }

// About the person — Type 2, private, and not tied to any project
{ "scope": "personal", "owner_id": "01M1TD4…",
  "key": "prefers", "value": { "explanations": "short" } }
```

"The release build takes a `--release` flag" is a fact about the
repository and belongs to whoever works on it. "This developer wants
short explanations" is a fact about a person and is nobody else's,
including on a server they share.

Note that personal memory is keyed by owner **alone**, not by owner and
project. What an agent learns about how you work is not a property of
the repository you happened to be in when it learned it.

The database refuses a row that names both a project and an owner, or
neither, with a CHECK constraint — so a malformed row cannot be written
by any path, including one added later by someone who has not read
this page.

## Type 3 is a separate idea from Type 2

"Ephemeral" could look like a strict subset of private, and collapsing
them is the mistake this classification exists to prevent. Live PTY
output is not "private data we happen not to write down" — it is data
that must **not** be written down. Naming it separately means an
accidental persist is visible as a wrong type rather than as an
unremarkable insert.

## Credentials are not Atlas's to hold

Atlas stores no tracker token. `ATLAS_TRACKER_TOKEN_FILE` is a *path*
to a file you already keep, read at startup, the way SSH reads a key.

There is no vault to breach, no export that leaks one, and no "where
does Atlas keep my token" question with an uncomfortable answer. It
also keeps the value out of `docker inspect`, process listings and
shell history, which passing it as a variable would not.

Sync configuration follows the same instinct: when you put a machine in
`auto` or `live` mode, the remote URL and bearer live **in process
memory only** and are never written to disk. A restart drops back to
manual and you re-arm it. That is a small inconvenience bought
deliberately — the alternative is a file on disk holding a credential
for someone else's server.

The one exception is honest about itself: signing in with "Continue with GitLab" does
hand the server a provider access token, because the exchange happens
there and the result arrives in this process. It is used once, to ask
the provider who you are, and then dropped. **It is not stored.**

It used to be. The column existed for a consumer that was never built —
a tracker adapter that would use the token later — so what actually sat
in the database was a live third-party credential, in plaintext, read
by nothing. If that adapter arrives, the token comes back with a design
for where it lives and how it is rotated. "Stored just in case" was not
one.
