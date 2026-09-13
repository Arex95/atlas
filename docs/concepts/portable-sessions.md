# Portable sessions

A session is portable when opening it on a machine that has never seen
the project *works* — the checkout appears and the shell starts in it.
That is a stronger claim than "the row synced", and it is what this
page is about.

## The problem with an absolute path

The obvious way to record where work is happening is the directory:

```
/home/you/dev/your-org/your-project
```

Sync that to a second machine and it is wrong in three ways at once —
different user, possibly different layout, possibly a directory that
does not exist at all. The row arrives intact and means nothing.

## Relative paths under a per-machine root

A session stores its path **relative** to that machine's own
`ATLAS_WORKSPACE_ROOT`:

```jsonc
{ "relative_path": "your-org/your-project",
  "remote_url": "git@github.com:you/your-project.git",
  "branch": "main" }
```

| Machine | `ATLAS_WORKSPACE_ROOT` | Resolves to |
|---|---|---|
| laptop | `~/dev` | `/home/you/dev/your-org/your-project` |
| desktop | `/srv/code` | `/srv/code/your-org/your-project` |

The same row means the right directory in both places, and neither has
to know how the other organises its disk.

## The session carries how to rebuild itself

A relative path only helps if something is there. So the session also
records its `remote_url` and `branch` — enough to *reconstruct* the
checkout, not just locate it.

That is what `terminal.restore` does. If the resolved path is missing,
it clones from the session's own remote at its own branch. If the path
already exists, it does nothing.

```
sessions.create      → registered, no process, no directory needed
terminal.spawn       → fails: path not found on this machine
terminal.restore     → clones
terminal.spawn       → a real shell, in the right place
terminal.restore     → no-op, the path is there now
```

A second laptop, a fresh container, a rebuilt machine: same sequence,
and nothing about it is special-cased for those situations.

## What restore will not do

**It does not resolve missing git credentials.** If the clone fails
for lack of access, that is reported as its own distinct error —
`credentials_missing`, separate from a generic clone failure — and
stops there.

Detect and report, do not resolve. A server that quietly acquires
credentials on your behalf is a worse problem than a clone that failed
and said why, and the distinct error is what lets a caller tell "this
repo needs your key" from "the URL is wrong".

## Why the terminal is not what travels

Nothing tries to move a running shell between machines. The PTY and its
output are Type 3 — ephemeral, never persisted, gone when the process
ends. See [State and privacy](/concepts/state-and-privacy).

What travels is the *arrangement*: which project, which branch, which
directory, whose it is. Rebuilding a shell from that takes a second and
is always correct. Serialising a live process would be neither.

That also means closing your laptop costs you a shell, not a session.
The scrollback is gone; the arrangement is not, and it comes back
wherever you open it next.
