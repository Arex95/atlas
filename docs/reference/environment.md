# Environment variables

Every setting comes from the environment; there is no config file to
find and no `--flag` to remember. Fourteen variables, one of them
effectively mandatory.

Atlas **fails to start** rather than starting degraded when a variable
is present but wrong — a bad listen address, a URL that does not parse,
a partial OAuth configuration. A server that boots with half a feature
silently disabled is harder to debug than one that refuses and says
why.

## Core

| Variable | Default | |
|---|---|---|
| `ATLAS_MCP_TOKEN` | — | Bearer that gates `POST /api/mcp`. See below. |
| `ATLAS_LISTEN_ADDR` | `0.0.0.0:4000` | `host:port`. See below. |
| `ATLAS_DB_PATH` | `./atlas-data/atlas.db` | SQLite file. Created on first run, along with its directory. |
| `ATLAS_WORKSPACE_ROOT` | `~/dev` | Where sessions' `relative_path` resolves. |

### `ATLAS_MCP_TOKEN`

One shared bearer for the whole MCP surface. Anyone holding it can call
every tool — there is no per-tool or per-project scoping, and it
identifies no particular agent.

Set to a non-empty value or the server refuses to start. An empty
string is rejected outright rather than treated as "no token", because
an empty token is almost always an unset variable that expanded to
nothing.

To run without one — local experiments only — set
`ATLAS_MCP_TOKEN_ALLOW_UNSET=true`. That leaves the MCP endpoint open
to anything that can reach the port, so pair it with a listen address
that is not `0.0.0.0`.

### `ATLAS_LISTEN_ADDR`

`4000` is only the default, and every example in these pages uses it for
that reason — not because anything depends on it. If something else on
your machine already holds the port, or you run two instances, move it:

```bash
ATLAS_LISTEN_ADDR=127.0.0.1:4001
```

Atlas refuses to start rather than picking another port for you, and
says so with the variable named:

```
could not bind to 127.0.0.1:4000: Address already in use (os error 98).
Set ATLAS_LISTEN_ADDR to a free address, e.g. 127.0.0.1:4001
```

It exits non-zero, so a supervisor or a restart policy sees a failure
rather than a process that came up on a port nobody is pointing at.
An unparseable value is refused the same way, at startup.

Remember to move the agent CLI's configured URL with it —
`just connect-agent` reads the resolved address, so re-running it
prints the right snippet.

### `ATLAS_WORKSPACE_ROOT`

A leading `~` expands against `HOME`; the server refuses to start if it
has to expand one and `HOME` is unset.

This is what makes a session portable. The session row stores a
*relative* path, and each machine resolves it against its own root — so
the same row means `~/dev/your-org/your-project` on a laptop and
`/srv/code/your-org/your-project` on a desktop, and `terminal.restore` knows
where to clone when the directory is missing.

## External tracker

All optional. With `ATLAS_TRACKER_KIND` unset the tracker tools return
a "tracker disabled" error (`-32003`) rather than failing obscurely.

| Variable | Default | |
|---|---|---|
| `ATLAS_TRACKER_KIND` | `none` | `none`, `gitlab`, `github`. |
| `ATLAS_TRACKER_URL` | — | Base URL. Refuses to start if it does not parse. |
| `ATLAS_TRACKER_TOKEN_FILE` | — | **Path to a file** holding the token, not the token. |
| `ATLAS_TRACKER_MIRROR_PROJECTS` | none | Comma-separated projects to mirror locally. |
| `ATLAS_TRACKER_MIRROR_INTERVAL_SECS` | `300` | How often the mirror pulls. |
| `ATLAS_TRACKER_WEBHOOK_SECRET` | — | Secret GitLab echoes back on a webhook. Unset means the receiver is not mounted at all. |

`ATLAS_TRACKER_TOKEN_FILE` takes a path rather than a value on purpose.
Atlas is not a credential store: the token lives in your own
config, the way an SSH key or `gh auth login` does, and Atlas reads it
where you already keep it. That also keeps it out of `docker inspect`,
process listings and shell history.

Leaving `ATLAS_TRACKER_MIRROR_PROJECTS` unset disables the mirror
entirely; tracker reads then go straight to the real tracker every
time.

`ATLAS_TRACKER_WEBHOOK_SECRET` shortens the delay between a change in
the tracker and the mirror reflecting it; it does not replace the
polling loop, which still runs and still catches anything a delivery
missed. Set it to the same value as the hook's *Secret token* in
GitLab, and treat it as a password: it is the only thing separating a
real delivery from anyone who knows the URL. With it unset the endpoint
returns `404` rather than accepting unauthenticated events.

## Reaching the server from a terminal

| Variable | |
|---|---|
| `ATLAS_SERVER_URL` | What a spawned terminal is told to call Atlas back on. |

Derived from `ATLAS_LISTEN_ADDR` when unset, which is right for the
ordinary case: a terminal runs on the same host as the server, so
whatever it bound is reachable. `0.0.0.0` becomes loopback — it means
"every interface", not an address anything can connect to. Set this
explicitly when the terminal is somewhere else entirely.

## OAuth

Atlas supports **GitLab** and **GitHub**, and either can be enabled
without the other. Both are *link-only*: a successful consent screen
never creates an account, it attaches an identity to one that already
exists. Someone with no account gets a `403`, not a sign-up.

`ATLAS_OAUTH_STATE_SECRET` is shared by both, because it signs this
server's own round trip rather than anything about the provider.

### GitLab

All four, or none. Setting some but not all is a **startup error**, not
a partial enable — a half-configured login is the kind of thing that
looks fine until someone tries to use it.

| Variable | |
|---|---|
| `ATLAS_OAUTH_GITLAB_CLIENT_ID` | From the GitLab OAuth application. |
| `ATLAS_OAUTH_GITLAB_CLIENT_SECRET` | Likewise. |
| `ATLAS_OAUTH_GITLAB_REDIRECT_URI` | Must match the app's registered callback exactly. |
| `ATLAS_OAUTH_STATE_SECRET` | Signs the CSRF `state`. Any long random string. |

### GitHub

Same rule, same shape.

| Variable | |
|---|---|
| `ATLAS_OAUTH_GITHUB_CLIENT_ID` | From the GitHub OAuth app. |
| `ATLAS_OAUTH_GITHUB_CLIENT_SECRET` | Likewise. |
| `ATLAS_OAUTH_GITHUB_REDIRECT_URI` | Must match the app's registered callback exactly. |
| `ATLAS_OAUTH_STATE_SECRET` | The same one GitLab uses. |

Atlas asks for `read:user user:email` and nothing more — no `repo`
scope, because it does not read GitHub repositories.

::: tip Why the email scope is required
An account is found by matching the provider's email against an
existing one. GitHub's profile endpoint returns an address it does not
guarantee to have verified, so Atlas ignores it and reads the account's
email list instead, accepting only the entry that is **both primary and
verified**. Without `user:email` that list is unreachable and login
fails — which is the right failure, because trusting the unverified
address would let anyone claim an account by adding its address to
their own GitHub profile.

A GitHub account with no verified primary email cannot log in. Verify
one on GitHub first.
:::

With a provider's variables unset, its two OAuth routes are **not
mounted at all** — they return `404` rather than a "not configured"
error, so an install that never uses them exposes nothing extra.

`ATLAS_OAUTH_STATE_SECRET` signs a timestamped nonce that is verified
on the callback before any network call. It never leaves the server and
is not derived from anything else, so rotating it is free: it only
invalidates login attempts already in flight, within their five-minute
window.

## A starting point

```bash
# Core
ATLAS_MCP_TOKEN=change-me-to-something-random
ATLAS_LISTEN_ADDR=127.0.0.1:4000
ATLAS_DB_PATH=./atlas-data/atlas.db
ATLAS_WORKSPACE_ROOT=~/dev

# External tracker — optional
ATLAS_TRACKER_KIND=gitlab
ATLAS_TRACKER_URL=https://gitlab.com
ATLAS_TRACKER_TOKEN_FILE=~/.config/atlas/gitlab-token
ATLAS_TRACKER_MIRROR_PROJECTS=your-org/your-project
ATLAS_TRACKER_MIRROR_INTERVAL_SECS=300

# GitLab OAuth — all four or none
# ATLAS_OAUTH_GITLAB_CLIENT_ID=
# ATLAS_OAUTH_GITLAB_CLIENT_SECRET=
# ATLAS_OAUTH_GITLAB_REDIRECT_URI=https://atlas.example.com/api/auth/oauth/gitlab/callback
# ATLAS_OAUTH_GITHUB_CLIENT_ID=
# ATLAS_OAUTH_GITHUB_CLIENT_SECRET=
# ATLAS_OAUTH_GITHUB_REDIRECT_URI=https://atlas.example.com/api/auth/oauth/github/callback
# ATLAS_OAUTH_STATE_SECRET=
```

`127.0.0.1` rather than the `0.0.0.0` default is the right choice for a
workstation: Atlas drives PTYs and holds your sessions, and there is no
reason for that to answer on every interface. The container image
defaults to `0.0.0.0` because a container's port is published
deliberately.
