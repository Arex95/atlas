# A shared way of working

This page is for whoever decides how a team works, rather than for
setting up one machine.

## The problem nobody writes down

Every developer working with an AI agent has invented a way of doing
it. What the agent may touch. When it is allowed to move on. What
counts as finished, and who checks. None of it is written anywhere. It
lives in one person's attention, and it is genuinely good — the people
who are effective with agents are effective because of exactly these
habits.

Then a second person joins, with their own set. Now the project has two
methodologies, both invisible, and the only way to notice they disagree
is to read a merge request and find that "done" meant something else.
Add a third and it stops being discoverable at all.

This is not a discipline problem, and telling people to be more careful
does not fix it. A rule that lives in someone's head cannot be reviewed,
cannot be argued with, and cannot be applied by anybody else.

## What Atlas does about it

A workflow is a YAML file **in your repository**. It says what the steps
are, what each one must satisfy before the next begins, and which tools
its agent is expected to use.

Two properties make that more than a document:

1. **The runtime executes the acceptance criteria**, so a step advances
   because a check passed, not because an agent said so.
2. **The file is re-read from disk every time a run starts**, so the
   rules come from the repository rather than from a copy somebody
   registered weeks ago.

Together those mean the way of working travels with the code, through
the tool the team already uses to agree on things: a merge request.
Changing how work advances becomes a diff somebody reviews.

## Where the file goes

```
your-project/
└── .atlas/
    └── workflows/
        ├── ship-a-feature.yaml
        └── fix-a-defect.yaml
```

`.atlas/workflows/` is the convention. Nothing enforces it — any path
inside the project works — but a convention is what lets a newcomer
find the rules without asking, and lets you say "it is in the usual
place" and be understood.

**Commit it.** A workflow that is not in version control is back to
being one person's habit, just in a file.

For what goes *inside* the file — nodes, dependencies, the two kinds of
acceptance criterion, scoping tools — see
[Workflow runs](/guide/workflow-runs).

## Adopting one, as a team

**Write the first one from something you already do.** Not an aspiration
— take a sequence the team performs regularly, with the checks it
actually applies, and write that down. A workflow that describes the
work honestly gets used; one that describes an ideal gets bypassed.

**Review it like code.** It goes in a merge request, someone reads it,
someone objects to a gate that is too strict. That argument is the
point: it is the first time the team has had the conversation with
something concrete in front of it.

**Expect to change it.** A gate that fires constantly for no good reason
is telling you the rule is wrong, not that people are sloppy. Fix the
file, commit, and the next run on every machine uses the new rule.

## What happens for the second developer

They clone the repository. The workflow file comes with it, because it
is a file in the repository.

On their machine they register it once, which points Atlas at the file:

```jsonc
// afg.register_workflow
{ "project": "your-org/your-project",
  "project_root": "/home/them/dev/your-org/your-project",
  "source_path": ".atlas/workflows/ship-a-feature.yaml" }
```

From that point on the file is what counts. They do not re-register when
it changes: **the next run re-reads it.** If you commit a stricter gate
this afternoon, their next run is held to it after a `git pull`, without
anyone being told to do anything.

Registration is a pointer, not a copy.

## Two guarantees worth knowing

**A run keeps the rules it started under.** If somebody commits a change
while an agent is working, that run finishes under the spec it was
dispatched with. An agent is never judged against criteria that arrived
after it started. The *next* run picks up the change.

**A broken file stops the run.** If the YAML does not parse, or the file
is gone, starting a run fails and says why. It does not quietly fall
back to the last version that worked — that would run rules the
repository no longer contains, which is the failure this design exists
to prevent.

## What this does not do

It does not stop a developer from ignoring Atlas and working directly in
a terminal. Nothing here is a restriction on people; the operating
system is the only real boundary, and Atlas says so in the
[trust model](/concepts/trust-model).

What it removes is the *invisibility*. A team that declares how it works
can disagree about it, improve it, and onboard into it. A team that
does not has four different answers and no way to see them.
