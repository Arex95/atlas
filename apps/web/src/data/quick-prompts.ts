export interface QuickPrompt {
  id: string;
  label: string;
  description: string;
  category: 'atlas' | 'context' | 'workflow' | 'debug';
  text: string;
}

export const QUICK_PROMPTS: QuickPrompt[] = [
  {
    id: 'atlas-context',
    label: 'Atlas Context',
    description: 'Explain what Atlas is and how to use it',
    category: 'atlas',
    text: `You are working inside the Atlas Orchestrator environment. Atlas is a project management and AI orchestration platform running at http://localhost:4000. It has an MCP server at http://localhost:4000/api/mcp (JSON-RPC 2.0, MCP 2024-11-05). You have access to tools: list_projects, read_file, list_sessions, send_message, read_inbox, list_documents, read_document, create_document, write_document, list_skills, create_notification, list_reminders. Use these tools to understand the project context before starting work.`,
  },
  {
    id: 'atlas-mcp-setup',
    label: 'Configure MCP',
    description: 'Instructions to configure the Atlas MCP server',
    category: 'atlas',
    text: `Configure your MCP server by adding this to your project's .mcp.json:\n{\n  "mcpServers": {\n    "atlas": {\n      "type": "http",\n      "url": "http://localhost:4000/api/mcp"\n    }\n  }\n}\nOr run: mcp add --transport http atlas http://localhost:4000/api/mcp`,
  },
  {
    id: 'read-project-index',
    label: 'Read Project Index',
    description: 'Read the project index before working',
    category: 'context',
    text: `Before you start, use the Atlas MCP tools to understand this project: 1) call list_projects to find the project, 2) call read_document or read_file on PROJECT_INDEX.md to understand the architecture and current state. Only start coding after you understand the context.`,
  },
  {
    id: 'list-agents',
    label: 'List Active Agents',
    description: 'Discover other active sessions',
    category: 'atlas',
    text: `Use the Atlas MCP tool list_sessions to see all active AI sessions in this workspace. Identify which agents are running and what they are working on. Then check read_inbox for any pending messages.`,
  },
  {
    id: 'save-progress',
    label: 'Save Progress',
    description: 'Document progress in Atlas',
    category: 'workflow',
    text: `Please document your current progress: 1) use create_document to save a summary of what you have done and what is pending, 2) use create_notification to notify the dashboard that the task status has changed. Be specific about what was completed and what is next.`,
  },
  {
    id: 'sync-context',
    label: 'Sync Context to Other Agent',
    description: 'Share context with another session',
    category: 'workflow',
    text: `Use send_message to share your current task context with another active session. Include: what you are working on, what files you have changed, and what you need from them. Use list_sessions first to find available agents.`,
  },
  {
    id: 'debug-explain',
    label: 'Debug & Explain',
    description: 'Ask for a systematic debug approach',
    category: 'debug',
    text: `Systematically debug this issue: 1) read the relevant source files first, 2) identify the root cause, 3) explain the problem clearly, 4) propose a fix with trade-offs, 5) implement only after I confirm. Do not guess — read the actual code first.`,
  },
  {
    id: 'rules-check',
    label: 'Check Against Rules',
    description: 'Verify work follows project rules',
    category: 'workflow',
    text: `Before committing any changes, verify your work follows the project rules: no blocking I/O in async handlers (use tokio::fs), no hardcoded colors or strings (use constants and design tokens), no console.log in production code, garde validation on all DTOs, zero clippy warnings, zero vue-tsc warnings.`,
  },
];
