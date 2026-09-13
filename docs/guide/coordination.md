# Coordination between agents

Four agents on one project need to tell each other things. Without a
channel, that is you relaying: reading one window, pasting into
another, remembering who is waiting on whom.

Atlas gives them a bus. Broadcasts to everyone on a project, direct
messages to one session.

## Send

```jsonc
// messaging.send_message — broadcast: omit `to`
{
  "project": "your-org/your-project",
  "from": "01M1T52M…",
  "type": "status",
  "payload": { "branch": "feat/auth", "state": "tests green" }
}

// direct: name a session
{
  "project": "your-org/your-project",
  "from": "01M1T52M…",
  "to": "01M1T7PP…",
  "type": "request",
  "payload": { "ask": "migrations are yours — is 0004 landed?" },
  "correlation_id": "auth-work"
}
```

`payload` is arbitrary JSON. `type` defaults to `"message"` and is a
free string — Atlas does not interpret it, so agents can agree on
`status`, `question`, `handoff` or whatever suits, without a schema
change here.

`correlation_id` threads related messages; `reply_to` points at the
message being answered.

## Read

```jsonc
// messaging.read_inbox
{ "project": "your-org/your-project", "for": "01M1T7PP…", "since": "01M1T52M…" }
```

An inbox is **every broadcast on the project, plus every direct message
addressed to you**, oldest first. Pass the last id you saw as `since`
to page forward; `limit` caps a page.

Polling, not push. An agent checks its inbox between turns, the way it
would check anything else — which is what an agent's loop can actually
do, whereas being interrupted mid-thought is not.

## Workflow tasks arrive here too

This is not a separate channel. When a workflow run dispatches a node,
it sends a `task` message on this same bus, addressed to the session
that should do the work:

```jsonc
{ "message_type": "task",
  "payload": { "runId": "01M1T7PY…", "nodeId": "test",
               "title": "Cover it", "instructions": "…", "isRetry": false } }
```

An agent that reads its inbox already receives workflow tasks. There is
no second thing to wire up, and nothing is injected into a PTY behind
the agent's back — dispatch is a message, and the agent picks it up the
way it picks up everything else.

## What this is not

It does not deliver messages to a *person*. There are no notifications
and no UI; these are agent-to-agent, read by whatever is polling.

And it does not persist conversations. The dialogue you had with your
agent CLI stays in that CLI. What lands here is coordination —
deliberately, so that "what did we decide" lives in the tracker and in
the repository, not in an archive nobody reads.
