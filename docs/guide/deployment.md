# Deployment

Two different jobs, two different shapes:

| | How it runs | Why |
|---|---|---|
| **Your machine** | The binary, natively | It drives PTYs against your working tree. A container plus bind mounts is friction that harms the exact ergonomics it exists for |
| **A team server** | The container image | The hosts people actually use — a VPS, a NAS, a box in the office — are heterogeneous. Pull an image, update a compose file, restart |

The same image serves both if you want it to; it just is not the
recommended path locally.

## Locally

```bash
cp .env.example .env    # set ATLAS_MCP_TOKEN
just run
```

Prefer a loopback address on a workstation:

```bash
ATLAS_LISTEN_ADDR=127.0.0.1:4000
```

Atlas holds your sessions and drives shells against your code. There is
no reason for that to answer on every interface. The container default
is `0.0.0.0` because a container's port is published deliberately.

## Compose

```bash
cp .env.example .env
just up          # build and run
just down        # tear down, drop the volume
```

Compose reads that same `.env`, keeps the database on a named volume so
it survives a rebuild, and publishes the port. Override the published
port with `ATLAS_HOST_PORT` rather than `ATLAS_LISTEN_ADDR` — inside
the container Atlas must bind `0.0.0.0` or the published port reaches
nothing.

## A real server

Whatever runs the image — compose, a panel, a scheduler — the same
things hold.

**Configuration comes from the environment.** The image is the same one
you built locally; nothing is baked in.

**The health check exercises the database.** `GET /health` runs a real
query rather than answering as soon as the process is up, and returns
`503` when storage is unreachable. Point your orchestrator at it.

**Startup is deterministic.** Migrations run on boot from a single
process. Atlas refuses to start on a bad configuration rather than
coming up degraded — a missing `ATLAS_MCP_TOKEN`, a busy port, a
partial OAuth configuration are all startup failures with a non-zero
exit, so a restart policy sees the failure instead of a process
listening where nobody is looking.

**Shutdown is graceful.** SIGINT and SIGTERM stop accepting new
connections and let in-flight requests finish, so a deploy does not cut
a sync mid-pass.

**Persist the data directory.** `ATLAS_DB_PATH` should point inside a
volume. Everything is in that one SQLite file.

**Put it behind a reverse proxy with TLS.** Session tokens and bearers
travel on every request. If you terminate TLS upstream, make sure the
proxy does not buffer responses — the `live` sync and workflow streams
are SSE, and a buffering proxy turns "live" into "eventually, in a
batch".

**Back it up by copying the file, and prove it by restoring.** A backup
nobody has restored is a hypothesis.

## Checklist before it faces anyone

- `ATLAS_MCP_TOKEN` set to something random, and never
  `ATLAS_MCP_TOKEN_ALLOW_UNSET` — that flag leaves every tool open to
  anything that can reach the port
- Only the proxy publishes a port; the container is not exposed directly
- TLS terminated, and the proxy not buffering SSE
- The data directory on a volume, with a backup that has been restored
  once
- The first account created immediately after the server is reachable —
  `register` works once, and you want it to be you
- Tracker token in a file the container can read, referenced by
  `ATLAS_TRACKER_TOKEN_FILE`, never passed as a value and never as a
  build argument (build arguments end up in the image history, readable
  by anyone who can pull it)

## Upgrades

Migrations are embedded in the binary and run at startup, so an upgrade
is: pull the new image, restart, watch the log line that says it is
listening. Nothing separate to run.

Roll back by pinning the previous image tag. Tag images with the commit
as well as a moving tag, so "which build is this" has an answer.
