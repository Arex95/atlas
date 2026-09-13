# Connect a tracker

With a tracker configured, agents read and write **real issues** in
your GitLab or GitHub. Atlas keeps no tasks table of its own — your
tracker is the source of truth, and adding a second one would mean a
sync bug per field.

## Configure it

```bash
ATLAS_TRACKER_KIND=gitlab
ATLAS_TRACKER_URL=https://gitlab.com
ATLAS_TRACKER_TOKEN_FILE=~/.config/atlas/gitlab-token
```

`ATLAS_TRACKER_TOKEN_FILE` is a **path to a file**, not the token.
Atlas reads it at startup from wherever you already keep it, the way
SSH reads a key, and never stores a copy. That also keeps the value out
of `docker inspect`, process listings and shell history — which passing
it directly as a variable would not.

Give the token the narrowest scope that works. Reading issues needs
read access; `tracker.create_issue` and `tracker.update_status` need
write. Nothing here needs admin.

Leave `ATLAS_TRACKER_KIND` unset and the `tracker.*` tools return a
clear "tracker disabled" error instead of failing obscurely. Everything
else is unaffected.

## Use it

```jsonc
// tracker.list_issues
{ "project": "your-org/your-project", "status": "open", "labels": ["backend"] }

// tracker.create_issue
{ "project": "your-org/your-project", "title": "Add /api/widgets",
  "description": "## Acceptance criteria\n\n- [ ] returns a JSON array\n" }

// tracker.close_issue
{ "project": "your-org/your-project", "id": "142" }
```

`project` accepts nested subgroups — `acme/platform/tools/widgets`
resolves correctly, not just `group/project`.

## The mirror

Set `ATLAS_TRACKER_MIRROR_PROJECTS` and Atlas keeps a local read copy,
refreshed on an interval:

```bash
ATLAS_TRACKER_MIRROR_PROJECTS=your-org/your-project,your-org/other
ATLAS_TRACKER_MIRROR_INTERVAL_SECS=300
```

Reads then come from SQLite instead of a round trip, which matters when
an agent lists issues repeatedly in a loop.

**Writes always go to the real tracker**, and refresh the mirror
immediately afterwards. A write is never visible locally before it is
real — the failure mode that avoids is an agent believing it filed an
issue that never left your machine.

Without the variable there is no mirror and every read goes upstream.
That is a fine way to run; the mirror is an optimisation, not a
requirement.

## Plan progress

```jsonc
// tracker.plan_progress
{ "project": "your-org/your-project", "status": "open" }
```

This counts **acceptance-criteria checkboxes**, not issues. It scans
matching issues for `- [ ]` and `- [x]` lines under an
`## Acceptance criteria` heading and returns ticked over total.

The difference is the point. An issue with sixteen criteria and one
ticked is 1/16 here — not "0 of 1 issues done", which is the same
number it would have shown before any work started at all. Criteria are
the unit that actually moves.

It follows that the number is only as good as your issue bodies. If
nothing writes acceptance criteria, there is nothing to count.
