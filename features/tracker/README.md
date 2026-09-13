# atlas-tracker

External issue tracker as a hexagonal port. Consumers
depend on the port; adapters implement it. This crate ships the port
plus one read-only adapter for GitLab; adapters for GitHub and other
trackers land under their own issues.

## Public surface

Re-exported from [`api`](./src/api.rs) — do not reach into `internal`
from outside the crate.

- Types: `Issue`, `IssueId`, `IssueStatus`, `Label`, `IssueRelation`,
  `IssueFilter`, `ProjectRef`, `ProjectRefError`, `TrackerError`.
- Port: `trait IssueTracker` (async, `Send + Sync`).
- Composition: `TrackerConfig`, `TrackerKind`, `TrackerConfigError`,
  `build_tracker(&TrackerConfig) -> TrackerRuntime`.

## Configuration

Read from the environment at process start; the crate itself never
touches env — the binary does, so composition stays at the edge.

| Variable | Required when | Purpose |
|---|---|---|
| `ATLAS_DB_PATH` | Always (has a default) | Path to the SQLite file. Default `./atlas-data/atlas.db`. Parent directory is created on startup. |
| `ATLAS_TRACKER_KIND` | Always | `none` (default) or `gitlab`. Any other value fails startup with the accepted list. |
| `ATLAS_TRACKER_URL` | `kind == gitlab` | Base URL of the tracker (e.g. `https://gitlab.com` or a self-hosted instance). |
| `ATLAS_TRACKER_TOKEN_FILE` | `kind == gitlab` | Path to a file whose contents are the PAT. The file is read once at startup; its value is never persisted by Atlas. |
| `ATLAS_TRACKER_MIRROR_PROJECTS` | Optional | Comma-separated `owner/repo` list. Empty (unset) → the mirror syncer does not start. Invalid entry → startup fails. |
| `ATLAS_TRACKER_MIRROR_INTERVAL_SECS` | Optional | Positive integer. Default `300` (five minutes). |

Rotation is by process restart — PATs were chosen precisely so that
runtime rotation complexity did not become the crate's problem.

### Token file shape

A single line, the PAT and nothing else. Leading and trailing
whitespace is stripped. Empty content fails startup.

```
glpat-xxxxxxxxxxxxxxxxxxxx
```

Protect the file the same way you protect `~/.ssh/keys` or
`~/.config/gh/hosts.yml`.

## Mirror

When `ATLAS_TRACKER_KIND=gitlab` and at least one project is listed
in `ATLAS_TRACKER_MIRROR_PROJECTS`, the composition wraps the
upstream adapter in a local `MirroredTracker` and constructs a
`MirrorSyncer` for it. Reads served from `MirroredTracker` hit
SQLite only — a tracker outage never fails a read, the last-known
state is returned.

The syncer runs an initial pass at startup and then re-pulls every
`ATLAS_TRACKER_MIRROR_INTERVAL_SECS` with a delta filter
(`updated_after` = last successful sync). One bad pull is logged
and the loop continues; a 429 with `retry_after` is honoured.

Migrations live at `features/tracker/migrations/` and are embedded
via `sqlx::migrate!()` — rebuild after adding a new one, or the
binary fails with `VersionMissing` (see repo CLAUDE.md).

A webhook receiver shortens the gap between a change upstream and the
mirror reflecting it — `POST /api/webhooks/tracker/gitlab`, mounted
only when `ATLAS_TRACKER_WEBHOOK_SECRET` is set. It does not replace
the polling loop, and stores nothing from the payload: the event names
an issue, and the issue is re-fetched.

Explicitly **not** in this crate today: cross-project auto-discovery.

## What is inside the crate

```
features/tracker/
├── migrations/                 SQL migrations owned by this feature
├── src/
│   ├── api.rs                  public re-exports
│   ├── contract.rs             shared contract every adapter must pass
│   ├── lib.rs
│   └── internal/
│       ├── domain/             tracker-agnostic types + port trait
│       ├── application/
│       │   ├── runtime.rs       consumer-facing handle
│       │   ├── factory.rs      config → composition
│       │   └── syncer.rs       background pull loop for the mirror
│       └── infrastructure/
│           ├── disabled.rs     no-op adapter for `kind == none`
│           ├── fake.rs         in-memory adapter for tests
│           ├── gitlab/         GitLab REST v4 adapter (read-only)
│           └── mirror/         SQLite mirror + MirroredTracker
├── tests/
│   ├── contract_fake.rs        fake satisfies the contract
│   ├── contract_mirror.rs      mirror satisfies the contract
│   ├── gitlab_adapter.rs       GitLab adapter satisfies the contract
│   │                            (wiremock, no live calls)
│   └── syncer.rs               initial pass, outage, cursor, relations
└── README.md
```

## Testing

```bash
cargo test -p atlas-tracker --all-features
```

CI runs offline. The GitLab adapter is tested against `wiremock`
mocks matching the shape of the endpoints it consumes. The
`test-support` feature exposes `FakeTracker` and `GitLabTracker`
for downstream crates that want to reuse them in their own tests.

## Scope of this crate today

Only read: `list_issues`, `get_issue`, `list_relations`. Writes,
the local mirror, MCP tool exposure and additional
adapters are separate issues; they extend the port, not this file.
