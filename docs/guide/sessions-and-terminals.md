# Sessions and terminals

A session records that someone is working on a project, on a branch, in
a directory. A terminal is a real shell bound to one.

They are separate on purpose: a session is durable and portable, a
terminal is neither. Registering a session touches no filesystem and
starts no process — the directory does not even have to exist yet.

## Register a session

```jsonc
// sessions.create
{
  "project": "your-org/your-project",
  "owner_id": "01M1TD4YJR7X1JTK29NTBB1AD3",
  "remote_url": "git@github.com:you/your-project.git",
  "branch": "main",
  "relative_path": "your-org/your-project",
  "title": "auth refactor",
  "agent_kind": "claude"
}
```

Two fields deserve attention.

**`relative_path` is relative.** It resolves against this machine's
`ATLAS_WORKSPACE_ROOT`, so the same session means
`~/dev/your-org/your-project` on one machine and `/srv/code/your-org/your-project`
on another. That is what makes it portable — see
[Portable sessions](/concepts/portable-sessions).

**`owner_id` scopes everything.** `sessions.list`, `get` and
`update_status` all take it, and a session belonging to someone else
returns the *same* not-found error as an id that does not exist.
Distinguishing them would confirm to a caller that a session they may
not see is there.

In Mode 1, with no accounts, pick any stable string and use it
consistently — it becomes a real user id when you activate
[team mode](/guide/team-mode).

## Open a shell in it

```jsonc
// terminal.spawn
{ "session_id": "01M1T52MDRS3CAH3B4G51Q2HCM" }
```

A real PTY, with your shell, in the session's directory. Not a
transcript and not a request/response API pretending to be one:
`terminal.write` sends raw stdin, so interactive programs, prompts and
ANSI all behave as they would in a terminal, because they are in one.

`spawn` is idempotent — spawning a session that already has a process
returns the existing one rather than erroring or starting a second.

```jsonc
// terminal.write
{ "session_id": "01M1T52M…", "input": "cargo test\n" }

// terminal.read_output — pass back the next_offset it returns
{ "session_id": "01M1T52M…", "since_offset": 0 }
```

Reading is poll-based: each call returns output plus a `next_offset` to
send on the following call. There is no push over MCP.

`terminal.close` kills the process and drops it from the pool.

## When the directory is not there

On a second machine, a fresh container, or a rebuilt disk, `spawn`
fails — the resolved path does not exist. That is the expected path,
not an error state:

```
terminal.spawn    → path_not_found
terminal.restore  → clones from the session's own remote_url and branch
terminal.spawn    → a real shell, in the right place
```

`restore` is a no-op when the path already exists, so it is safe to
call before every spawn.

It **will not** resolve missing git credentials. A clone that fails for
lack of access returns a distinct `credentials_missing` error rather
than a generic failure, and stops. That is what lets you tell "this
repo needs your key" from "the URL is wrong" — and Atlas acquiring
credentials on your behalf would be a worse problem than a clone that
failed loudly.

## What survives, and what does not

| | Survives a restart | Travels to another machine |
|---|---|---|
| The session | Yes | Yes |
| The PTY process | No | No |
| Its scrollback | No | No |

Terminal output is ephemeral by design — never written to disk. Closing
your laptop costs you a shell, not a session: the arrangement comes
back wherever you open it next, and rebuilding a shell from it takes a
second and is always correct.

That is also why sessions carry `agent_kind` and a `title`. They are
what a person needs to tell four sessions apart at a glance, which is
the actual problem when several agents are working at once.

## Archiving

```jsonc
// sessions.update_status
{ "id": "01M1T52M…", "owner_id": "01M1TD4…", "status": "archived" }
```

Sessions are archived rather than deleted, and `sessions.list` takes a
`status` filter. Work that is finished stops cluttering the list without
losing what it was.
