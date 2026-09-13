---
layout: home

hero:
  name: Atlas
  text: "Several agents, one way of working, nothing taken on trust"
  tagline: "An agent will tell you the task is done. Atlas is built so that saying it is not enough — the acceptance criteria run in the runtime, not in the agent's report. Declare once how work advances on this project, and every developer's agents are held to the same rules."
  actions:
    - theme: brand
      text: "Get started"
      link: /guide/getting-started
    - theme: alt
      text: "Why Atlas"
      link: /concepts/why-atlas

features:
  - title: Done is a thing that was checked
    details: "A node dispatches, the agent works, and the runtime runs the criteria itself — a command that exits zero, a payload that matches a schema. On failure the reason goes back into the agent's context and the node retries. You cannot instruct a model into reliability; you can only check its output at the system that receives it. That system is Atlas."
  - title: One declared way of working
    details: "The workflow is a graph in your repository, not a habit each developer improvised with their own agent. A node scopes the tools its agent may call, and is dispatched on its own — the agent is told its step, not the ones after it. The file is re-read every time a run starts, so a teammate who pulls your change is held to it on their next run without registering anything."
  - title: Real terminals, portable sessions
    details: "A PTY with a shell in it that you and an agent both drive — not a transcript, not a request/response API pretending to be a shell. A session records how to rehydrate itself, so a machine that has never seen the repository clones it and picks up where the other left off."
  - title: What it refuses is the design
    details: "No inbound command channel from a messaging app. No admin view over personal state. No credential store — your tracker token stays in the file you already keep it in. And Atlas states where its boundary ends rather than overselling it: a tool scope prevents accidents, not a determined agent. The operating system is the real boundary, and the docs say so."
---
