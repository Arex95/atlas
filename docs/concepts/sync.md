# Sync

How eagerly your machine talks to the team server is your choice, per
machine, changeable at runtime. Three modes:

| | When it syncs | For |
|---|---|---|
| **`focus`** | Only when you ask | Deep work. The default |
| **`auto`** | Every *N* seconds | A normal collaborative day |
| **`live`** | The moment the server says something changed | Pair-working |

Two people on the same team can hold different modes at the same time
without conflict, and neither can tell what the other picked.

## Why the quiet one is the default

Baking a single cadence into the product wins one group and alienates
the other two. Someone deep in a problem does not want background
network activity and state moving underneath them; someone pairing
wants the opposite, immediately.

`focus` is the default because quiet should not be a preference you
have to discover. Nothing syncs until you ask, and you ask with
`sync.sessions_now` or `sync.memory_now`.

## The modes differ in timing, not in meaning

This is the property the whole design protects: **all three reach the
same state.** `focus` is `auto` with a longer interval; `auto` is
`live` with a worse clock. The sync pass they run is identical.

That is why `live` events carry a **notification and no data**. The
event says "something of yours changed" and the client answers by
running the same pass it would have run on a timer. One consistency
path, not two.

The alternative — putting the changed rows in the event — looks like a
saved round trip and is really a second replication mechanism with its
own ordering, deduplication and missed-event problems, sitting next to
one that already solved them.

It also makes a **missed event harmless by construction**. Disconnect
during a change and nobody replays it for you; reconnecting runs a pass
that collects everything you missed anyway. There is no replay buffer
to size and no cursor in the stream, because nothing depends on
receiving every event.

## Conflicts resolve by last write

Two machines editing the same thing resolve by `updated_at`: newest
wins, deterministically, on both sides.

For sessions the identity is the row id. For agent memory it is the
**logical key** — `(project, key)` or `(owner_id, key)` — and that
difference matters. Two machines can each remember the same fact
without having seen the other's row, so they hold different ids for
one thing. Matching on id would treat them as unrelated and collide on
the unique index instead of merging.

## `live` in practice

The client holds an SSE stream open to the server. It is server-to-client
only, because that is the only direction that needs a stream: your
writes already go over the ordinary REST push and work.

Three behaviours worth knowing:

- **It converges on connect**, before waiting for any event. Anything
  that changed while you were disconnected produced a notification
  nobody heard — harmless, precisely because the pull that follows does
  not care what triggered it.
- **It reconnects on its own**, with a backoff that starts at a second
  and caps at thirty. Arming `live` against a server that is *down* is
  not an error; that is what the reconnect loop is for, and refusing
  would make the mode unavailable exactly when the server is having a
  bad day.
- **`sync.status` reports whether the stream is actually connected.**
  A long-lived connection that dies silently would otherwise be
  indistinguishable from a quiet team — the worst failure mode a live
  feature has, because it looks like success.

## Configuration is never written down

`auto` and `live` need a remote URL, a bearer token and an owner id.
Those live **in process memory only**. A restart drops back to `focus`
and you re-arm.

Deliberate: the alternative is a file on disk holding a credential for
another machine. See
[State and privacy](/concepts/state-and-privacy#credentials-are-not-atlas-s-to-hold).

## What syncs today

Sessions and agent memory. Extending it to more personal state is
ongoing work — the engine is not per-type, so each addition is a
push/pull pair rather than a new mechanism.
