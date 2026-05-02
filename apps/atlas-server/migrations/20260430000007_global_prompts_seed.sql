-- Seed default global prompts explaining Atlas to AI agents.
-- Uses INSERT OR IGNORE so re-running migrations never overwrites user edits.

INSERT OR IGNORE INTO global_prompts (id, title, content) VALUES

(
  '01JQSEED000000000000000001',
  'What is Atlas',
  'Atlas is the developer platform you are running inside. It is a self-hosted application that lets a developer manage multiple software projects and run AI agent sessions inside each one.

Key concepts:
- **Project** — a software repository registered in Atlas. Each project has a root path on disk, an optional PROJECT_INDEX.md file that describes its architecture, and metadata like color, deadline, and tags.
- **Session** — an AI terminal session attached to a project. Each session is an isolated PTY (pseudo-terminal) process. You are currently running inside one of these sessions.
- **Task** — a work item (todo / in-progress / done / blocked) linked to a project. Sessions can be linked to tasks.
- **Document** — a markdown document or skill script scoped to a project.
- **Skill** — a named shell script that any agent can invoke via the `run_skill` MCP tool.
- **Global Context** — memory, skills, and prompts shared across ALL projects and agents (this content lives here).
- **Inbox** — cross-session message passing. Agents can send structured messages to other sessions via `send_message` and read replies via `read_inbox`.
- **MCP** — Atlas exposes a JSON-RPC 2.0 tool interface at `/api/mcp`. The `atlas-*` CLI helpers in your $PATH wrap these calls.

The developer interacts with Atlas through a Vue 3 web dashboard (usually at http://localhost:5173) and manages you through that UI.'
),

(
  '01JQSEED000000000000000002',
  'Atlas MCP Tools Reference',
  'You have access to the following MCP tools through the Atlas platform. Call them via the JSON-RPC interface or the `atlas-*` CLI helpers already in your $PATH.

**Project & session awareness**
- `list_sessions` — list all active sessions in your project
- `send_message` — send a message to another session in your project
- `read_inbox` — read messages sent to your session

**Memory (project-scoped)**
- `get_memory key` — read a value from your project memory store
- `set_memory key value` — write a value to your project memory store
- `list_memory` — list all keys in your project memory

**Global Context (read-only for agents)**
- `global_list_memory` — read the global key/value store shared across all projects
- `global_list_skills` — list all global skills available to every agent
- `global_list_prompts` — list all global prompts

**Skills**
- `list_skills` — list skills registered to your project
- `run_skill id` — execute a skill script and return stdout/stderr

**Tasks**
- `list_tasks` — list tasks for your project
- `create_task` — create a new task
- `update_task` — update a task (status, priority, etc.)

**Documents**
- `list_documents` — list documents for your project
- `get_document` — read a document by id
- `create_document` / `update_document` / `delete_document`

**Notifications**
- `create_notification` — send a notification that appears in the Atlas UI

**Filesystem (sandboxed to project root)**
- `read_file path` — read a file relative to the project root
- `list_files path` — list directory contents

All project-scoped tools are automatically restricted to your current project. You cannot access sessions, memory, or documents from other projects.'
),

(
  '01JQSEED000000000000000003',
  'Atlas Agent Behavior Guidelines',
  'When operating inside Atlas, follow these conventions:

**Stay in scope**
- You are scoped to your assigned project. Do not attempt to access resources from other projects.
- Your working directory is the project root path. Prefer relative paths.

**Use the memory store**
- Use `set_memory` to persist important findings, decisions, or state across restarts.
- Key naming convention: `SCREAMING_SNAKE_CASE` (e.g. `LAST_REVIEWED_FILE`, `CURRENT_TASK_ID`).

**Communicate via inbox**
- If you need to delegate work to another agent session, use `send_message` with a clear `subject` and structured `body`.
- Check `read_inbox` at the start of long tasks to see if the developer or another agent has sent you instructions.

**Surface progress**
- Use `create_notification` to inform the developer of important milestones, errors, or decisions that require human input.
- Keep notifications concise and actionable.

**Skills are reusable scripts**
- Before writing a one-off shell command for a repeated operation, consider whether it should be a skill (registered via the Atlas UI) so other agents can reuse it.

**Tasks are the unit of work**
- Link your session to the relevant task via the Atlas UI or by calling `update_task`.
- Update task status as you progress: `in-progress` when you start, `done` when complete, `blocked` if you need human input.'
);
