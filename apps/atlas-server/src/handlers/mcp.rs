use crate::constants::{defaults, env, errors, response, terminal as term_consts};
use crate::dtos::session::CreateSessionRequest;
use crate::mcp::{McpRequest, jsonrpc_error, jsonrpc_success};
use crate::repositories::{document as doc_repo, global as global_repo, project as project_repo, reminder as reminder_repo, session as session_repo, skill as skill_repo, task as task_repo};
use crate::services::notification as notif_svc;
use crate::services::session as session_svc;
use crate::socket_events;
use crate::terminal::TerminalManager;
use axum::http as ax_http;
use axum::{
    Json,
    extract::{Extension, State},
    response::{IntoResponse, sse::{Event, KeepAlive, Sse}},
};
use futures::stream;
use std::convert::Infallible;
use std::time::Duration;
use serde_json::json;
use socketioxide::SocketIo;
use sqlx::SqlitePool;
use std::sync::Arc;

pub async fn handle_mcp_request(
    State(pool): State<SqlitePool>,
    Extension(tm): Extension<Arc<TerminalManager>>,
    Extension(io): Extension<SocketIo>,
    headers: ax_http::HeaderMap,
    Json(request): Json<McpRequest>,
) -> impl IntoResponse {
    let session_id_from_header = headers
        .get("x-atlas-session-id")
        .and_then(|h| h.to_str().ok())
        .unwrap_or(term_consts::MCP_AGENT_ID);

    tracing::info!(
        "[MCP] Received request from {}: {} (ID: {:?})",
        session_id_from_header,
        request.method,
        request.id
    );

    let is_notification = request.id.is_null();

    // Resolve the caller's project_id for MCP project isolation.
    let caller_project_id: Option<String> =
        if session_id_from_header != term_consts::MCP_AGENT_ID {
            session_repo::find_project_id(&pool, session_id_from_header)
                .await
                .ok()
                .flatten()
        } else {
            None
        };

    let result = match request.method.as_str() {
        "initialize" => jsonrpc_success(
            request.id,
            json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {
                    "tools": { "listChanged": false },
                    "resources": { "listChanged": false, "subscribe": false },
                    "prompts": { "listChanged": false }
                },
                "serverInfo": {
                    "name": "Atlas Orchestrator",
                    "version": "0.1.0",
                    "instructions": crate::prompts::SERVER_INSTRUCTIONS
                }
            }),
        ),
        "notifications/initialized" => {
            return (ax_http::StatusCode::ACCEPTED, "").into_response();
        }
        "prompts/list" => {
            let prompts = vec![json!({
                "name": "orchestration_manual",
                "description": "Atlas Operational Guidelines for Autonomous Agents",
                "arguments": []
            })];
            jsonrpc_success(request.id, json!({ "prompts": prompts }))
        }
        "prompts/get" => {
            let params = request.params.unwrap_or(json!({}));
            let name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
            match name {
                "orchestration_manual" => jsonrpc_success(
                    request.id,
                    json!({
                        "description": "Atlas Operational Guidelines",
                        "messages": [{
                            "role": "user",
                            "content": {
                                "type": "text",
                                "text": crate::prompts::ORCHESTRATION_PROMPT_FOR_AGENTS
                            }
                        }]
                    }),
                ),
                _ => jsonrpc_error(request.id, -32602, "Unknown prompt name"),
            }
        }
        "resources/list" => {
            let resources = vec![
                json!({
                    "uri": "atlas://sessions",
                    "name": "Active AI Sessions",
                    "description": "List of active AI sessions in this project. SEE atlas://manual FOR GUIDELINES.",
                    "mimeType": "application/json"
                }),
                json!({
                    "uri": "atlas://manual",
                    "name": "Atlas Orchestration Manual",
                    "description": "Instructions on how to react to inter-agent messages and use Atlas tools",
                    "mimeType": "text/markdown"
                }),
                json!({
                    "uri": "atlas://projects",
                    "name": "Atlas Projects",
                    "description": "List of projects managed by Atlas",
                    "mimeType": "application/json"
                }),
                json!({
                    "uri": "atlas://global",
                    "name": "Global Context",
                    "description": "Read-only global memory, skills, and prompts shared across all projects and agents",
                    "mimeType": "application/json"
                }),
            ];
            jsonrpc_success(request.id, json!({ "resources": resources }))
        }
        "resources/read" => {
            let params = request.params.unwrap_or(json!({}));
            let uri = params.get("uri").and_then(|v| v.as_str()).unwrap_or("");

            match uri {
                "atlas://manual" | "manual" => jsonrpc_success(
                    request.id,
                    json!({
                        "contents": [{
                            "uri": uri,
                            "mimeType": "text/markdown",
                            "text": crate::prompts::ORCHESTRATION_MANUAL
                        }]
                    }),
                ),
                "atlas://sessions" | "sessions" => {
                    let sessions = session_repo::find_active(&pool, caller_project_id.as_deref()).await;
                    match sessions {
                        Ok(s) => jsonrpc_success(
                            request.id,
                            json!({
                                "contents": [{
                                    "uri": uri,
                                    "mimeType": "application/json",
                                    "text": serde_json::to_string(&s).unwrap_or_else(|_| "[]".to_string())
                                }]
                            }),
                        ),
                        Err(e) => jsonrpc_error(request.id, -32000, &format!("Database error: {}", e)),
                    }
                }
                "atlas://projects" | "projects" => {
                    let projects = crate::repositories::project::find_all_mcp(&pool).await;
                    match projects {
                        Ok(p) => jsonrpc_success(
                            request.id,
                            json!({
                                "contents": [{
                                    "uri": uri,
                                    "mimeType": "application/json",
                                    "text": serde_json::to_string(&p).unwrap_or_else(|_| "[]".to_string())
                                }]
                            }),
                        ),
                        Err(e) => jsonrpc_error(request.id, -32603, &e.to_string()),
                    }
                }
                "atlas://global" | "global" => {
                    let memory = global_repo::list_memory_mcp(&pool).await.unwrap_or_default();
                    let skills = global_repo::list_skills_full_mcp(&pool).await.unwrap_or_default();
                    let prompts = global_repo::list_prompts_mcp(&pool).await.unwrap_or_default();

                    let payload = serde_json::json!({
                        "memory": memory,
                        "skills": skills,
                        "prompts": prompts
                    });
                    jsonrpc_success(
                        request.id,
                        json!({
                            "contents": [{
                                "uri": uri,
                                "mimeType": "application/json",
                                "text": serde_json::to_string(&payload).unwrap_or_else(|_| "{}".to_string())
                            }]
                        }),
                    )
                }
                _ => jsonrpc_error(request.id, -32602, "Unknown resource URI"),
            }
        }
        "tools/list" => {
            let tools = vec![
                json!({
                    "name": "list_projects",
                    "description": "List all projects in Atlas",
                    "inputSchema": { "type": "object", "properties": {} }
                }),
                json!({
                    "name": "read_file",
                    "description": "Read a file from a registered Atlas project. Path must be absolute and inside a project root (e.g. /home/user/projects/myapp/src/main.rs). Use list_projects first to get root paths.",
                    "inputSchema": {
                        "type": "object",
                        "properties": { "path": { "type": "string", "description": "Absolute path to the file" } },
                        "required": ["path"]
                    }
                }),
                json!({
                    "name": "list_sessions",
                    "description": "List all currently active AI agent sessions in this project. Use this to discover other agents you can coordinate with via send_message.",
                    "inputSchema": { "type": "object", "properties": {} }
                }),
                json!({
                    "name": "send_message",
                    "description": "Send a message to another session in this project",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "toSessionId": { "type": "string", "description": "Target session ID (must be in the same project)" },
                            "message": { "type": "string", "description": "Message content" },
                            "fromId": { "type": "string", "description": "ID of the sender (defaults to current agent)" }
                        },
                        "required": ["toSessionId", "message"]
                    }
                }),
                json!({
                    "name": "read_inbox",
                    "description": "Read incoming messages for a session in this project",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "sessionId": { "type": "string", "description": "Session ID (must be in the same project)" },
                            "limit": { "type": "number", "default": 20 }
                        },
                        "required": ["sessionId"]
                    }
                }),
                json!({
                    "name": "get_history",
                    "description": "Read the saved conversation history of a session. Useful to recover context after a restart or to review what was said.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "sessionId": { "type": "string", "description": "Session ID (defaults to current session)" },
                            "limit": { "type": "number", "description": "Max messages to return (default 50)" }
                        }
                    }
                }),
                json!({
                    "name": "save_message",
                    "description": "Persist a message to the session conversation history. Call this to record what the user said (role=user) and what you responded (role=assistant) so the history panel shows a full log.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "role": { "type": "string", "enum": ["user", "assistant", "system"], "description": "Who produced this message" },
                            "content": { "type": "string", "description": "Full message content" },
                            "sessionId": { "type": "string", "description": "Target session ID (defaults to current session)" }
                        },
                        "required": ["role", "content"]
                    }
                }),
                json!({
                    "name": "list_documents",
                    "description": "List documents. Defaults to session scope (your private docs). Use scope='project' + projectId for shared project documents.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "scope": { "type": "string", "description": "session (default) | project", "default": "session" },
                            "sessionId": { "type": "string", "description": "Your session ID ($ATLAS_SESSION_ID). Required for session scope when called via mcp-remote." },
                            "projectId": { "type": "string", "description": "Required when scope=project" },
                            "type": { "type": "string", "description": "Filter by type: document, skill, index" }
                        }
                    }
                }),
                json!({
                    "name": "read_document",
                    "description": "Read the full content of a document by its id. Works for both session and project scope documents. Get the id from list_documents.",
                    "inputSchema": {
                        "type": "object",
                        "properties": { "id": { "type": "string" } },
                        "required": ["id"]
                    }
                }),
                json!({
                    "name": "create_document",
                    "description": "Create a document private to this session (default) or shared at project scope. IMPORTANT: always pass sessionId=$ATLAS_SESSION_ID — mcp-remote does not forward HTTP headers.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "title": { "type": "string" },
                            "content": { "type": "string" },
                            "type": { "type": "string", "description": "document | skill | index", "default": "document" },
                            "scope": { "type": "string", "description": "session (default, private to this session) | project (shared across all sessions)", "default": "session" },
                            "sessionId": { "type": "string", "description": "REQUIRED: your $ATLAS_SESSION_ID — run 'echo $ATLAS_SESSION_ID' to get it" },
                            "projectId": { "type": "string", "description": "Required only when scope=project" }
                        },
                        "required": ["title", "sessionId"]
                    }
                }),
                json!({
                    "name": "write_document",
                    "description": "Overwrite the content of an existing document (session or project scope). Requires the document id from list_documents. Optionally update the title too.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "id": { "type": "string" },
                            "content": { "type": "string" },
                            "title": { "type": "string" }
                        },
                        "required": ["id", "content"]
                    }
                }),
                json!({
                    "name": "delete_document",
                    "description": "Permanently delete a document by its id. Works for both session and project scope. Get the id from list_documents.",
                    "inputSchema": {
                        "type": "object",
                        "properties": { "id": { "type": "string" } },
                        "required": ["id"]
                    }
                }),
                json!({
                    "name": "list_skills",
                    "description": "List saved skill scripts for a project. Skills are reusable shell scripts the developer stored in Atlas. Pass projectId as slug (e.g. 'tubalita') or ULID.",
                    "inputSchema": {
                        "type": "object",
                        "properties": { "projectId": { "type": "string" } },
                        "required": ["projectId"]
                    }
                }),
                json!({
                    "name": "run_skill",
                    "description": "Execute a saved skill script in a terminal session. Skills are reusable shell scripts stored in Atlas (e.g. run tests, deploy, lint). Use list_skills to see available scripts first.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "skillId": { "type": "string", "description": "Skill ID to execute" },
                            "sessionId": { "type": "string", "description": "Target session ID (defaults to caller's session)" }
                        },
                        "required": ["skillId"]
                    }
                }),
                json!({
                    "name": "create_notification",
                    "description": "Create a persistent notification visible in the Atlas dashboard",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "message": { "type": "string" },
                            "title": { "type": "string" },
                            "type": { "type": "string", "description": "info | warning | error | success", "default": "info" },
                            "projectId": { "type": "string" }
                        },
                        "required": ["message"]
                    }
                }),
                json!({
                    "name": "list_reminders",
                    "description": "List reminders. Defaults to session scope. Use scope='project' + projectId for shared project reminders.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "scope": { "type": "string", "description": "session (default) | project", "default": "session" },
                            "sessionId": { "type": "string", "description": "Your session ID ($ATLAS_SESSION_ID). Required for session scope when called via mcp-remote." },
                            "projectId": { "type": "string", "description": "Required when scope=project" },
                            "status": { "type": "string", "description": "pending | done" }
                        }
                    }
                }),
                json!({
                    "name": "create_reminder",
                    "description": "Create a reminder private to this session (default) or shared at project scope. IMPORTANT: always pass sessionId=$ATLAS_SESSION_ID — mcp-remote does not forward HTTP headers.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "title": { "type": "string" },
                            "description": { "type": "string" },
                            "dueAt": { "type": "string", "description": "ISO date string, e.g. 2026-06-01T09:00:00" },
                            "type": { "type": "string", "description": "reminder | deadline", "default": "reminder" },
                            "scope": { "type": "string", "description": "session (default, private to this session) | project (shared across all sessions)", "default": "session" },
                            "sessionId": { "type": "string", "description": "REQUIRED: your $ATLAS_SESSION_ID — run 'echo $ATLAS_SESSION_ID' to get it" },
                            "projectId": { "type": "string", "description": "Required only when scope=project" }
                        },
                        "required": ["title", "dueAt", "sessionId"]
                    }
                }),
                json!({
                    "name": "update_reminder",
                    "description": "Update an existing reminder. Mark it done (status='done'), reschedule (dueAt), or rename it. Requires the reminder id from list_reminders.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "id": { "type": "string" },
                            "title": { "type": "string" },
                            "dueAt": { "type": "string" },
                            "status": { "type": "string", "description": "pending | done" }
                        },
                        "required": ["id"]
                    }
                }),
                json!({
                    "name": "list_tasks",
                    "description": "List tasks. Defaults to session scope. Use scope='project' + projectId for shared project tasks.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "scope": { "type": "string", "description": "session (default) | project", "default": "session" },
                            "sessionId": { "type": "string", "description": "Your session ID ($ATLAS_SESSION_ID). Required for session scope when called via mcp-remote." },
                            "projectId": { "type": "string", "description": "Required when scope=project" },
                            "status": { "type": "string", "description": "todo | in-progress | done | blocked" }
                        }
                    }
                }),
                json!({
                    "name": "create_task",
                    "description": "Create a task private to this session (default) or shared at project scope. IMPORTANT: mcp-remote does not forward HTTP headers — you MUST always pass sessionId=$ATLAS_SESSION_ID explicitly or the call will fail with 'No session ID'.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "title": { "type": "string" },
                            "description": { "type": "string" },
                            "status": { "type": "string", "description": "todo | in-progress | done | blocked", "default": "todo" },
                            "priority": { "type": "string", "description": "low | medium | high | critical", "default": "medium" },
                            "dueDate": { "type": "string", "description": "YYYY-MM-DD" },
                            "assignedTo": { "type": "string" },
                            "scope": { "type": "string", "description": "session (default, private to this session) | project (shared across all sessions)", "default": "session" },
                            "sessionId": { "type": "string", "description": "REQUIRED: your $ATLAS_SESSION_ID — run 'echo $ATLAS_SESSION_ID' to get it" },
                            "projectId": { "type": "string", "description": "Required only when scope=project" }
                        },
                        "required": ["title", "sessionId"]
                    }
                }),
                json!({
                    "name": "update_task",
                    "description": "Update an existing task. Use this to mark tasks in-progress or done, change priority, set a due date, or reassign. Requires the task id (get it from list_tasks).",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "id": { "type": "string" },
                            "title": { "type": "string" },
                            "description": { "type": "string" },
                            "status": { "type": "string", "description": "todo | in-progress | done | blocked" },
                            "priority": { "type": "string", "description": "low | medium | high | critical" },
                            "dueDate": { "type": "string", "description": "YYYY-MM-DD" },
                            "assignedTo": { "type": "string" }
                        },
                        "required": ["id"]
                    }
                }),
                json!({
                    "name": "delete_task",
                    "description": "Permanently delete a task by its id. Get the id from list_tasks.",
                    "inputSchema": {
                        "type": "object",
                        "properties": { "id": { "type": "string" } },
                        "required": ["id"]
                    }
                }),
                json!({
                    "name": "update_project",
                    "description": "Update Atlas project metadata visible in the dashboard (name, description, color, version, author). Use list_projects to get the slug. Does NOT touch the project's files.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "slug": { "type": "string", "description": "Project slug identifier" },
                            "name": { "type": "string" },
                            "description": { "type": "string" },
                            "color": { "type": "string", "description": "Hex color e.g. #3b82f6" },
                            "version": { "type": "string" },
                            "author": { "type": "string" }
                        },
                        "required": ["slug"]
                    }
                }),
                json!({
                    "name": "get_memory",
                    "description": "Read a memory key. Defaults to session scope (your private memory). Use scope='project' + projectId for shared project memory.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "key": { "type": "string" },
                            "scope": { "type": "string", "description": "session (default) | project", "default": "session" },
                            "sessionId": { "type": "string", "description": "Your session ID ($ATLAS_SESSION_ID). Required for session scope when called via mcp-remote." },
                            "projectId": { "type": "string", "description": "Required when scope=project" }
                        },
                        "required": ["key"]
                    }
                }),
                json!({
                    "name": "set_memory",
                    "description": "Write a memory key/value private to this session (default) or shared at project scope. IMPORTANT: always pass sessionId=$ATLAS_SESSION_ID — mcp-remote does not forward HTTP headers.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "key": { "type": "string" },
                            "value": { "type": "string" },
                            "scope": { "type": "string", "description": "session (default, private to this session) | project (shared across all sessions)", "default": "session" },
                            "sessionId": { "type": "string", "description": "REQUIRED: your $ATLAS_SESSION_ID — run 'echo $ATLAS_SESSION_ID' to get it" },
                            "projectId": { "type": "string", "description": "Required only when scope=project" }
                        },
                        "required": ["key", "value", "sessionId"]
                    }
                }),
                json!({
                    "name": "list_memory",
                    "description": "List all memory keys. Defaults to session scope. Use scope='project' + projectId to list all shared project memory keys.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "scope": { "type": "string", "description": "session (default) | project", "default": "session" },
                            "sessionId": { "type": "string", "description": "Your session ID ($ATLAS_SESSION_ID). Required for session scope when called via mcp-remote." },
                            "projectId": { "type": "string", "description": "Required when scope=project" }
                        }
                    }
                }),
                json!({
                    "name": "global_list_memory",
                    "description": "Read the developer-curated global memory: key-value context shared across ALL projects and agents (e.g. coding standards, personal preferences, recurring patterns). Read-only for agents.",
                    "inputSchema": { "type": "object", "properties": {} }
                }),
                json!({
                    "name": "global_list_skills",
                    "description": "List global skill scripts available to all agents across all projects (not project-specific). These are general-purpose automation scripts the developer saved.",
                    "inputSchema": { "type": "object", "properties": {} }
                }),
                json!({
                    "name": "global_list_prompts",
                    "description": "List global prompt templates saved by the developer, available to all agents. Useful for discovering reusable instructions (e.g. code review template, commit message format).",
                    "inputSchema": { "type": "object", "properties": {} }
                }),
                json!({
                    "name": "spawn_session",
                    "description": "Create a new AI session in the current project and return its ID. Use this to implement a sliding-window worker pool: call list_sessions first to count active workers, spawn only if the count is below your limit (e.g. 5), assign a task via create_task + send_message to the new session ID.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "title": { "type": "string", "description": "Human-readable label for the session (e.g. 'worker-auth-module')" },
                            "provider": { "type": "string", "description": "AI provider (default: claude)" },
                            "model": { "type": "string", "description": "Model ID (default: claude-sonnet-4-6)" },
                            "workingDirectory": { "type": "string", "description": "Absolute working directory for the session (defaults to project root)" }
                        }
                    }
                }),
                json!({
                    "name": "close_session",
                    "description": "Kill a session's PTY and remove it from the database. Call this when a worker finishes its task so the pool slot is freed and the next pending task can be assigned. Only sessions belonging to the same project can be closed.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "sessionId": { "type": "string", "description": "ID of the session to close" }
                        },
                        "required": ["sessionId"]
                    }
                }),
                json!({
                    "name": "get_document_links",
                    "description": "Fetch a document and all documents it directly links to (1-hop graph traversal). Use this to navigate an Obsidian-style knowledge graph: read the project index document first, follow only the links relevant to your task, avoid loading the entire documentation set. Works for both project-scope and session-scope documents.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "id": { "type": "string", "description": "Document ID to start from (get IDs from list_documents)" }
                        },
                        "required": ["id"]
                    }
                }),
            ];
            jsonrpc_success(request.id, json!({ "tools": tools }))
        }
        "tools/call" => {
            let params = request.params.unwrap_or(json!({}));
            let name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let default_args = json!({});
            let arguments = params.get("arguments").unwrap_or(&default_args);

            // sessionId param overrides header — lets agents pass their session ID
            // explicitly when mcp-remote doesn't forward the x-atlas-session-id header.
            let session_id_param = arguments.get("sessionId").and_then(|v| v.as_str()).unwrap_or("");
            let session_id_from_header = if !session_id_param.is_empty() { session_id_param } else { session_id_from_header };

            match name {
                "list_projects" => {
                    match crate::repositories::project::find_all_mcp(&pool).await {
                        Ok(p) => jsonrpc_success(
                            request.id,
                            json!({ "content": [{ "type": "text", "text": serde_json::to_string(&p).unwrap_or_else(|_| "[]".to_string()) }] }),
                        ),
                        Err(e) => jsonrpc_error(request.id, -32603, &e.to_string()),
                    }
                }
                "read_file" => {
                    let path = arguments.get("path").and_then(|v| v.as_str()).unwrap_or("");
                    if path.is_empty() {
                        jsonrpc_error(request.id, -32602, "Missing 'path' argument")
                    } else {
                        match crate::handlers::path_guard::validate_path_in_projects(&pool, path).await {
                            Ok(safe_path) => {
                                const MAX_BYTES: u64 = 5 * 1024 * 1024;
                                let oversized = tokio::fs::metadata(&safe_path)
                                    .await
                                    .map(|m| m.len() > MAX_BYTES)
                                    .unwrap_or(false);
                                if oversized {
                                    jsonrpc_error(request.id, -32603, errors::FILE_TOO_LARGE)
                                } else {
                                    match tokio::fs::read_to_string(&safe_path).await {
                                        Ok(content) => jsonrpc_success(
                                            request.id,
                                            json!({ "content": [{ "type": "text", "text": content }] }),
                                        ),
                                        Err(e) => jsonrpc_error(request.id, -32603, &e.to_string()),
                                    }
                                }
                            }
                            Err(e) => jsonrpc_error(request.id, -32603, &e.to_string()),
                        }
                    }
                }
                "list_sessions" => {
                    match session_repo::find_active(&pool, caller_project_id.as_deref()).await {
                        Ok(s) => jsonrpc_success(
                            request.id,
                            json!({ "content": [{ "type": "text", "text": serde_json::to_string(&s).unwrap_or_else(|_| "[]".to_string()) }] }),
                        ),
                        Err(e) => jsonrpc_error(request.id, -32603, &e.to_string()),
                    }
                }
                "send_message" => {
                    let to_id = arguments.get("toSessionId").and_then(|v| v.as_str()).unwrap_or("");
                    let content = arguments
                        .get("message")
                        .or_else(|| arguments.get("content"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    let from_id = arguments
                        .get("fromId")
                        .and_then(|v| v.as_str())
                        .unwrap_or(session_id_from_header);

                    if to_id.is_empty() || content.is_empty() {
                        return Json(jsonrpc_error(request.id, -32602, errors::MISSING_PARAMS))
                            .into_response();
                    }

                    // Project isolation: block cross-project messaging.
                    if let Some(ref my_pid) = caller_project_id {
                        let target_pid = session_repo::find_project_id(&pool, to_id).await.ok().flatten();
                        if target_pid.as_deref() != Some(my_pid.as_str()) {
                            return Json(jsonrpc_error(
                                request.id,
                                -32603,
                                "Cross-project messaging is not allowed",
                            ))
                            .into_response();
                        }
                    }

                    match tm.inject_message(to_id, from_id, content).await {
                        Ok(_) => {
                            let _ = io
                                .within("/")
                                .to(to_id.to_string())
                                .emit(
                                    socket_events::SESSION_RECEIVE_MESSAGE,
                                    &serde_json::json!({
                                        "id": ulid::Ulid::new().to_string(),
                                        "fromId": from_id,
                                        "content": content,
                                        "timestamp": chrono::Utc::now().to_rfc3339(),
                                        "isAgent": true
                                    }),
                                )
                                .await;
                            jsonrpc_success(
                                request.id,
                                json!({ "content": [{ "type": "text", "text": response::MESSAGE_SENT }] }),
                            )
                        }
                        Err(e) => jsonrpc_error(request.id, -32603, &e),
                    }
                }
                "read_inbox" => {
                    let session_id = arguments.get("sessionId").and_then(|v| v.as_str()).unwrap_or("");
                    let limit = arguments.get("limit").and_then(|v| v.as_f64()).unwrap_or(20.0) as i64;

                    // Project isolation: only allow reading inbox of same-project sessions.
                    if let Some(ref my_pid) = caller_project_id {
                        let target_pid = session_repo::find_project_id(&pool, session_id).await.ok().flatten();
                        if target_pid.as_deref() != Some(my_pid.as_str()) {
                            return Json(jsonrpc_error(
                                request.id,
                                -32603,
                                "Access denied: session belongs to a different project",
                            ))
                            .into_response();
                        }
                    }

                    match session_repo::find_inbox(&pool, session_id, limit).await {
                        Ok(m) => jsonrpc_success(
                            request.id,
                            json!({ "content": [{ "type": "text", "text": serde_json::to_string(&m).unwrap_or_else(|_| "[]".to_string()) }] }),
                        ),
                        Err(e) => jsonrpc_error(request.id, -32603, &e.to_string()),
                    }
                }
                "get_history" => {
                    let target_session = arguments
                        .get("sessionId")
                        .and_then(|v| v.as_str())
                        .unwrap_or(session_id_from_header);
                    let limit = arguments.get("limit").and_then(|v| v.as_f64()).unwrap_or(50.0) as i64;

                    match session_repo::find_history(&pool, target_session, limit).await {
                        Ok(msgs) => jsonrpc_success(
                            request.id,
                            json!({ "content": [{ "type": "text", "text": serde_json::to_string(&msgs).unwrap_or_else(|_| "[]".to_string()) }] }),
                        ),
                        Err(e) => jsonrpc_error(request.id, -32603, &e.to_string()),
                    }
                }
                "save_message" => {
                    let role = arguments.get("role").and_then(|v| v.as_str()).unwrap_or("assistant");
                    let content = arguments.get("content").and_then(|v| v.as_str()).unwrap_or("");
                    let requested_session = arguments
                        .get("sessionId")
                        .and_then(|v| v.as_str())
                        .unwrap_or(session_id_from_header);

                    if content.is_empty() {
                        return Json(jsonrpc_error(request.id, -32602, errors::MISSING_PARAMS))
                            .into_response();
                    }

                    if !["user", "assistant", "system"].contains(&role) {
                        return Json(jsonrpc_error(request.id, -32602, "role must be user, assistant or system"))
                            .into_response();
                    }

                    // Resolve the actual session id — agent may pass a name or MCP_AGENT fallback
                    let resolved_session = session_repo::resolve_session_id(&pool, requested_session, session_id_from_header).await;
                    let target_session = match resolved_session.as_deref() {
                        Some(id) => id.to_string(),
                        None => return Json(jsonrpc_error(request.id, -32602, "No valid session found — pass a sessionId from list_sessions")).into_response(),
                    };

                    let id = ulid::Ulid::new().to_string();
                    match session_repo::create_message(&pool, &id, &target_session, role, content).await {
                        Ok(_) => jsonrpc_success(
                            request.id,
                            json!({ "content": [{ "type": "text", "text": "Message saved" }] }),
                        ),
                        Err(e) => jsonrpc_error(request.id, -32603, &e.to_string()),
                    }
                }
                "list_documents" => {
                    use crate::repositories::{session_document as sd_repo};
                    let scope = arguments.get("scope").and_then(|v| v.as_str()).unwrap_or("session");
                    let kind = arguments.get("type").and_then(|v| v.as_str());
                    if scope == "project" {
                        let project_id_raw = arguments.get("projectId").and_then(|v| v.as_str()).unwrap_or("");
                        let project_id = match crate::repositories::project::resolve_id(&pool, project_id_raw).await {
                            Ok(id) => id,
                            Err(_) => return Json(jsonrpc_error(request.id, -32602, "Project not found")).into_response(),
                        };
                        match doc_repo::find_for_mcp(&pool, &project_id, kind).await {
                            Ok(docs) => jsonrpc_success(request.id, json!({ "content": [{ "type": "text", "text": serde_json::to_string(&docs).unwrap_or_else(|_| "[]".to_string()) }] })),
                            Err(e) => jsonrpc_error(request.id, -32603, &e.to_string()),
                        }
                    } else {
                        let sid = match session_repo::resolve_session_id(&pool, session_id_from_header, session_id_from_header).await {
                            Some(s) => s,
                            None => return Json(jsonrpc_error(request.id, -32602, "No session ID — pass sessionId=$ATLAS_SESSION_ID in this tool call (mcp-remote does not forward HTTP headers)")).into_response(),
                        };
                        match sd_repo::find_all(&pool, &sid, kind).await {
                            Ok(docs) => jsonrpc_success(request.id, json!({ "content": [{ "type": "text", "text": serde_json::to_string(&docs).unwrap_or_else(|_| "[]".to_string()) }] })),
                            Err(e) => jsonrpc_error(request.id, -32603, &e.to_string()),
                        }
                    }
                }
                "read_document" => {
                    let id = arguments.get("id").and_then(|v| v.as_str()).unwrap_or("");
                    // Try project docs first, then session docs via direct query.
                    if let Ok(Some(d)) = doc_repo::find_full_by_id(&pool, id).await {
                        return Json(jsonrpc_success(request.id, json!({ "content": [{ "type": "text", "text": format!("# {}\n\n{}", d.title, d.content) }] }))).into_response();
                    }
                    match sqlx::query_as::<_, (String, String)>(
                        "SELECT title, content FROM session_documents WHERE id = ?",
                    )
                    .bind(id)
                    .fetch_optional(&pool)
                    .await
                    {
                        Ok(Some((title, content))) => jsonrpc_success(request.id, json!({ "content": [{ "type": "text", "text": format!("# {}\n\n{}", title, content) }] })),
                        Ok(None) => jsonrpc_error(request.id, -32602, "Document not found"),
                        Err(e) => jsonrpc_error(request.id, -32603, &e.to_string()),
                    }
                }
                "create_document" => {
                    use crate::repositories::session_document as sd_repo;
                    let scope = arguments.get("scope").and_then(|v| v.as_str()).unwrap_or("session");
                    let title = arguments.get("title").and_then(|v| v.as_str()).unwrap_or("");
                    let content = arguments.get("content").and_then(|v| v.as_str()).unwrap_or("");
                    let kind = arguments.get("type").and_then(|v| v.as_str()).unwrap_or("document");
                    let links = arguments.get("links").and_then(|v| v.as_str()).unwrap_or("[]");
                    if title.is_empty() {
                        return Json(jsonrpc_error(request.id, -32602, errors::MISSING_PARAMS)).into_response();
                    }
                    let doc_id = ulid::Ulid::new().to_string();
                    if scope == "project" {
                        let project_id = arguments.get("projectId").and_then(|v| v.as_str()).unwrap_or("");
                        let resolved_pid = match crate::repositories::project::resolve_id(&pool, project_id).await {
                            Ok(id) => id,
                            Err(_) => return Json(jsonrpc_error(request.id, -32602, "Project not found")).into_response(),
                        };
                        match doc_repo::create_simple(&pool, &doc_id, &resolved_pid, title, content, kind, links).await {
                            Ok(_) => jsonrpc_success(request.id, json!({ "content": [{ "type": "text", "text": format!("Created project document '{}' (id: {})", title, doc_id) }] })),
                            Err(e) => jsonrpc_error(request.id, -32603, &e.to_string()),
                        }
                    } else {
                        let sid = match session_repo::resolve_session_id(&pool, session_id_from_header, session_id_from_header).await {
                            Some(s) => s,
                            None => return Json(jsonrpc_error(request.id, -32602, "No session ID — pass sessionId=$ATLAS_SESSION_ID in this tool call (mcp-remote does not forward HTTP headers)")).into_response(),
                        };
                        match sd_repo::create(&pool, &doc_id, &sid, title, content, kind, links).await {
                            Ok(_) => jsonrpc_success(request.id, json!({ "content": [{ "type": "text", "text": format!("Created session document '{}' (id: {})", title, doc_id) }] })),
                            Err(e) => jsonrpc_error(request.id, -32603, &e.to_string()),
                        }
                    }
                }
                "write_document" => {
                    use crate::repositories::session_document as sd_repo;
                    let id = arguments.get("id").and_then(|v| v.as_str()).unwrap_or("");
                    let content = arguments.get("content").and_then(|v| v.as_str()).unwrap_or("");
                    let title = arguments.get("title").and_then(|v| v.as_str());
                    let links = arguments.get("links").and_then(|v| v.as_str());
                    if id.is_empty() {
                        return Json(jsonrpc_error(request.id, -32602, errors::MISSING_PARAMS)).into_response();
                    }
                    // Try session doc first, then project doc
                    if let Ok(true) = sd_repo::write_content(&pool, id, content, title, links).await {
                        return Json(jsonrpc_success(request.id, json!({ "content": [{ "type": "text", "text": "Session document updated" }] }))).into_response();
                    }
                    match doc_repo::write_content(&pool, id, content, title, links).await {
                        Ok(true) => jsonrpc_success(request.id, json!({ "content": [{ "type": "text", "text": "Project document updated" }] })),
                        Ok(false) => jsonrpc_error(request.id, -32602, "Document not found"),
                        Err(e) => jsonrpc_error(request.id, -32603, &e.to_string()),
                    }
                }
                "delete_document" => {
                    use crate::repositories::session_document as sd_repo;
                    let id = arguments.get("id").and_then(|v| v.as_str()).unwrap_or("");
                    // Try session doc first, then project doc
                    if let Ok(true) = sd_repo::delete(&pool, id).await {
                        return Json(jsonrpc_success(request.id, json!({ "content": [{ "type": "text", "text": "Session document deleted" }] }))).into_response();
                    }
                    match doc_repo::delete(&pool, id).await {
                        Ok(true) => jsonrpc_success(request.id, json!({ "content": [{ "type": "text", "text": "Project document deleted" }] })),
                        Ok(false) => jsonrpc_error(request.id, -32602, "Document not found"),
                        Err(e) => jsonrpc_error(request.id, -32603, &e.to_string()),
                    }
                }
                "list_skills" => {
                    let project_id_raw = arguments.get("projectId").and_then(|v| v.as_str()).unwrap_or("");
                    let project_id = if project_id_raw.is_empty() {
                        String::new()
                    } else {
                        match crate::repositories::project::resolve_id(&pool, project_id_raw).await {
                            Ok(id) => id,
                            Err(_) => return Json(jsonrpc_error(request.id, -32602, "Project not found")).into_response(),
                        }
                    };
                    match skill_repo::find_for_mcp(&pool, &project_id).await {
                        Ok(skills) => jsonrpc_success(request.id, json!({ "content": [{ "type": "text", "text": serde_json::to_string(&skills).unwrap_or_else(|_| "[]".to_string()) }] })),
                        Err(e) => jsonrpc_error(request.id, -32603, &e.to_string()),
                    }
                }
                "run_skill" => {
                    let skill_id = arguments.get("skillId").and_then(|v| v.as_str()).unwrap_or("");
                    let target_session = arguments
                        .get("sessionId")
                        .and_then(|v| v.as_str())
                        .unwrap_or(session_id_from_header);

                    if skill_id.is_empty() {
                        return Json(jsonrpc_error(request.id, -32602, errors::MISSING_PARAMS)).into_response();
                    }

                    match crate::services::skill::run(&pool, &tm, skill_id, target_session, caller_project_id.as_deref()).await {
                        Ok(msg) => jsonrpc_success(request.id, json!({ "content": [{ "type": "text", "text": msg }] })),
                        Err(e) => jsonrpc_error(request.id, -32603, &e),
                    }
                }
                "create_notification" => {
                    let message = arguments.get("message").and_then(|v| v.as_str()).unwrap_or("");
                    let title = arguments.get("title").and_then(|v| v.as_str());
                    let kind = arguments.get("type").and_then(|v| v.as_str()).unwrap_or("info");
                    let project_id = arguments.get("projectId").and_then(|v| v.as_str());
                    if message.is_empty() {
                        return Json(jsonrpc_error(request.id, -32602, errors::MISSING_PARAMS)).into_response();
                    }
                    let payload = crate::dtos::notification::CreateNotificationRequest {
                        project_id: project_id.map(|s| s.to_string()),
                        session_id: None,
                        title: title.map(|s| s.to_string()),
                        message: message.to_string(),
                        kind: kind.to_string(),
                    };
                    match notif_svc::create(&pool, &payload).await {
                        Ok(n) => {
                            let _ = io.emit(socket_events::NOTIFICATION_NEW, &serde_json::json!({
                                "id": &n.id, "title": &n.title, "message": &n.message,
                                "type": &n.kind, "projectId": &n.project_id
                            })).await;
                            jsonrpc_success(request.id, json!({ "content": [{ "type": "text", "text": format!("Notification created: {}", n.id) }] }))
                        }
                        Err(e) => jsonrpc_error(request.id, -32603, &e.to_string()),
                    }
                }
                "list_reminders" => {
                    let scope = arguments.get("scope").and_then(|v| v.as_str()).unwrap_or("session");
                    let status = arguments.get("status").and_then(|v| v.as_str());
                    if scope == "project" {
                        let project_id = if let Some(raw) = arguments.get("projectId").and_then(|v| v.as_str()) {
                            match crate::repositories::project::resolve_id(&pool, raw).await {
                                Ok(id) => Some(id),
                                Err(_) => return Json(jsonrpc_error(request.id, -32602, "Project not found")).into_response(),
                            }
                        } else { None };
                        match reminder_repo::find_for_mcp(&pool, project_id.as_deref(), status).await {
                            Ok(reminders) => jsonrpc_success(request.id, json!({ "content": [{ "type": "text", "text": serde_json::to_string(&reminders).unwrap_or_else(|_| "[]".to_string()) }] })),
                            Err(e) => jsonrpc_error(request.id, -32603, &e.to_string()),
                        }
                    } else {
                        let sid = match session_repo::resolve_session_id(&pool, session_id_from_header, session_id_from_header).await {
                            Some(s) => s,
                            None => return Json(jsonrpc_error(request.id, -32602, "No session ID — pass sessionId=$ATLAS_SESSION_ID in this tool call (mcp-remote does not forward HTTP headers)")).into_response(),
                        };
                        match reminder_repo::find_for_session(&pool, &sid, status).await {
                            Ok(reminders) => jsonrpc_success(request.id, json!({ "content": [{ "type": "text", "text": serde_json::to_string(&reminders).unwrap_or_else(|_| "[]".to_string()) }] })),
                            Err(e) => jsonrpc_error(request.id, -32603, &e.to_string()),
                        }
                    }
                }
                "create_reminder" => {
                    let scope = arguments.get("scope").and_then(|v| v.as_str()).unwrap_or("session");
                    let title = arguments.get("title").and_then(|v| v.as_str()).unwrap_or("");
                    let description = arguments.get("description").and_then(|v| v.as_str()).unwrap_or("");
                    let due_at = arguments.get("dueAt").and_then(|v| v.as_str()).unwrap_or("");
                    let kind = arguments.get("type").and_then(|v| v.as_str()).unwrap_or("reminder");
                    if title.is_empty() || due_at.is_empty() {
                        return Json(jsonrpc_error(request.id, -32602, errors::MISSING_PARAMS)).into_response();
                    }
                    let id = ulid::Ulid::new().to_string();
                    if scope == "project" {
                        let project_id_raw = arguments.get("projectId").and_then(|v| v.as_str());
                        let resolved_pid_opt = if let Some(pid) = project_id_raw {
                            match crate::repositories::project::resolve_id(&pool, pid).await {
                                Ok(pid) => Some(pid),
                                Err(_) => return Json(jsonrpc_error(request.id, -32602, "Project not found")).into_response(),
                            }
                        } else { None };
                        match reminder_repo::create(&pool, &id, resolved_pid_opt.as_deref(), None, title, description, due_at, kind).await {
                            Ok(_) => jsonrpc_success(request.id, json!({ "content": [{ "type": "text", "text": format!("Project reminder created: {}", id) }] })),
                            Err(e) => jsonrpc_error(request.id, -32603, &e.to_string()),
                        }
                    } else {
                        let sid = match session_repo::resolve_session_id(&pool, session_id_from_header, session_id_from_header).await {
                            Some(s) => s,
                            None => return Json(jsonrpc_error(request.id, -32602, "No session ID — pass sessionId=$ATLAS_SESSION_ID in this tool call (mcp-remote does not forward HTTP headers)")).into_response(),
                        };
                        match reminder_repo::create(&pool, &id, None, Some(&sid), title, description, due_at, kind).await {
                            Ok(_) => jsonrpc_success(request.id, json!({ "content": [{ "type": "text", "text": format!("Session reminder created: {}", id) }] })),
                            Err(e) => jsonrpc_error(request.id, -32603, &e.to_string()),
                        }
                    }
                }
                "update_reminder" => {
                    let id = arguments.get("id").and_then(|v| v.as_str()).unwrap_or("");
                    let title = arguments.get("title").and_then(|v| v.as_str());
                    let due_at = arguments.get("dueAt").and_then(|v| v.as_str());
                    let status = arguments.get("status").and_then(|v| v.as_str());
                    if id.is_empty() {
                        return Json(jsonrpc_error(request.id, -32602, errors::MISSING_PARAMS)).into_response();
                    }
                    match reminder_repo::update_simple(&pool, id, title, due_at, status).await {
                        Ok(true) => jsonrpc_success(request.id, json!({ "content": [{ "type": "text", "text": "Reminder updated" }] })),
                        Ok(false) => jsonrpc_error(request.id, -32602, "Reminder not found"),
                        Err(e) => jsonrpc_error(request.id, -32603, &e.to_string()),
                    }
                }
                "list_tasks" => {
                    let scope = arguments.get("scope").and_then(|v| v.as_str()).unwrap_or("session");
                    let status = arguments.get("status").and_then(|v| v.as_str());
                    if scope == "project" {
                        let project_id = if let Some(raw) = arguments.get("projectId").and_then(|v| v.as_str()) {
                            match crate::repositories::project::resolve_id(&pool, raw).await {
                                Ok(id) => Some(id),
                                Err(_) => return Json(jsonrpc_error(request.id, -32602, "Project not found")).into_response(),
                            }
                        } else { None };
                        match task_repo::find_for_mcp(&pool, project_id.as_deref(), status).await {
                            Ok(tasks) => jsonrpc_success(request.id, json!({ "content": [{ "type": "text", "text": serde_json::to_string(&tasks).unwrap_or_else(|_| "[]".to_string()) }] })),
                            Err(e) => jsonrpc_error(request.id, -32603, &e.to_string()),
                        }
                    } else {
                        let sid = match session_repo::resolve_session_id(&pool, session_id_from_header, session_id_from_header).await {
                            Some(s) => s,
                            None => return Json(jsonrpc_error(request.id, -32602, "No session ID — pass sessionId=$ATLAS_SESSION_ID in this tool call (mcp-remote does not forward HTTP headers)")).into_response(),
                        };
                        match task_repo::find_for_session(&pool, &sid, status).await {
                            Ok(tasks) => jsonrpc_success(request.id, json!({ "content": [{ "type": "text", "text": serde_json::to_string(&tasks).unwrap_or_else(|_| "[]".to_string()) }] })),
                            Err(e) => jsonrpc_error(request.id, -32603, &e.to_string()),
                        }
                    }
                }
                "create_task" => {
                    let scope = arguments.get("scope").and_then(|v| v.as_str()).unwrap_or("session");
                    let title = arguments.get("title").and_then(|v| v.as_str()).unwrap_or("");
                    let description = arguments.get("description").and_then(|v| v.as_str()).unwrap_or("");
                    let status = arguments.get("status").and_then(|v| v.as_str()).unwrap_or("todo");
                    let priority = arguments.get("priority").and_then(|v| v.as_str()).unwrap_or("medium");
                    let due_date = arguments.get("dueDate").and_then(|v| v.as_str());
                    let assigned_to = arguments.get("assignedTo").and_then(|v| v.as_str());
                    if title.is_empty() {
                        return Json(jsonrpc_error(request.id, -32602, errors::MISSING_PARAMS)).into_response();
                    }
                    let id = ulid::Ulid::new().to_string();
                    if scope == "project" {
                        let project_id = arguments.get("projectId").and_then(|v| v.as_str()).unwrap_or("");
                        let resolved_pid = match crate::repositories::project::resolve_id(&pool, project_id).await {
                            Ok(id) => id,
                            Err(_) => return Json(jsonrpc_error(request.id, -32602, "Project not found")).into_response(),
                        };
                        match task_repo::create_simple(&pool, &id, &resolved_pid, title, description, status, priority, due_date, assigned_to).await {
                            Ok(_) => jsonrpc_success(request.id, json!({ "content": [{ "type": "text", "text": format!("Project task created: {} (id: {})", title, id) }] })),
                            Err(e) => jsonrpc_error(request.id, -32603, &e.to_string()),
                        }
                    } else {
                        let sid = match session_repo::resolve_session_id(&pool, session_id_from_header, session_id_from_header).await {
                            Some(s) => s,
                            None => return Json(jsonrpc_error(request.id, -32602, "No session ID — pass sessionId=$ATLAS_SESSION_ID in this tool call (mcp-remote does not forward HTTP headers)")).into_response(),
                        };
                        match task_repo::create_for_session(&pool, &id, &sid, title, description, status, priority, due_date, assigned_to).await {
                            Ok(_) => jsonrpc_success(request.id, json!({ "content": [{ "type": "text", "text": format!("Session task created: {} (id: {})", title, id) }] })),
                            Err(e) => jsonrpc_error(request.id, -32603, &e.to_string()),
                        }
                    }
                }
                "update_task" => {
                    let id = arguments.get("id").and_then(|v| v.as_str()).unwrap_or("");
                    let title = arguments.get("title").and_then(|v| v.as_str());
                    let description = arguments.get("description").and_then(|v| v.as_str());
                    let status = arguments.get("status").and_then(|v| v.as_str());
                    let priority = arguments.get("priority").and_then(|v| v.as_str());
                    let due_date = arguments.get("dueDate").and_then(|v| v.as_str());
                    let assigned_to = arguments.get("assignedTo").and_then(|v| v.as_str());
                    if id.is_empty() {
                        return Json(jsonrpc_error(request.id, -32602, errors::MISSING_PARAMS)).into_response();
                    }
                    match task_repo::update_simple(&pool, id, title, description, status, priority, due_date, assigned_to).await {
                        Ok(true) => jsonrpc_success(request.id, json!({ "content": [{ "type": "text", "text": "Task updated" }] })),
                        Ok(false) => jsonrpc_error(request.id, -32602, "Task not found"),
                        Err(e) => jsonrpc_error(request.id, -32603, &e.to_string()),
                    }
                }
                "delete_task" => {
                    let id = arguments.get("id").and_then(|v| v.as_str()).unwrap_or("");
                    if id.is_empty() {
                        return Json(jsonrpc_error(request.id, -32602, errors::MISSING_PARAMS)).into_response();
                    }
                    match task_repo::delete(&pool, id).await {
                        Ok(true) => jsonrpc_success(request.id, json!({ "content": [{ "type": "text", "text": "Task deleted" }] })),
                        Ok(false) => jsonrpc_error(request.id, -32602, "Task not found"),
                        Err(e) => jsonrpc_error(request.id, -32603, &e.to_string()),
                    }
                }
                "update_project" => {
                    let slug = arguments.get("slug").and_then(|v| v.as_str()).unwrap_or("");
                    let name = arguments.get("name").and_then(|v| v.as_str());
                    let description = arguments.get("description").and_then(|v| v.as_str());
                    let color = arguments.get("color").and_then(|v| v.as_str());
                    let version = arguments.get("version").and_then(|v| v.as_str());
                    let author = arguments.get("author").and_then(|v| v.as_str());
                    if slug.is_empty() {
                        return Json(jsonrpc_error(request.id, -32602, errors::MISSING_PARAMS)).into_response();
                    }
                    match crate::repositories::project::update_simple(&pool, slug, name, description, color, version, author).await {
                        Ok(true) => jsonrpc_success(request.id, json!({ "content": [{ "type": "text", "text": format!("Project '{}' updated", slug) }] })),
                        Ok(false) => jsonrpc_error(request.id, -32602, "Project not found"),
                        Err(e) => jsonrpc_error(request.id, -32603, &e.to_string()),
                    }
                }
                "list_memory" => {
                    use crate::repositories::session_memory as sm_repo;
                    let scope = arguments.get("scope").and_then(|v| v.as_str()).unwrap_or("session");
                    if scope == "project" {
                        let project_id_raw = arguments.get("projectId").and_then(|v| v.as_str()).unwrap_or("");
                        let resolved = match crate::repositories::project::resolve_id(&pool, project_id_raw).await {
                            Ok(id) => id,
                            Err(_) => return Json(jsonrpc_error(request.id, -32602, "Project not found")).into_response(),
                        };
                        match crate::repositories::memory::find_all(&pool, &resolved).await {
                            Ok(rows) => jsonrpc_success(request.id, json!({ "content": [{ "type": "text", "text": serde_json::to_string(&rows).unwrap_or_else(|_| "[]".to_string()) }] })),
                            Err(e) => jsonrpc_error(request.id, -32603, &e.to_string()),
                        }
                    } else {
                        let sid = match session_repo::resolve_session_id(&pool, session_id_from_header, session_id_from_header).await {
                            Some(s) => s,
                            None => return Json(jsonrpc_error(request.id, -32602, "No session ID — pass sessionId=$ATLAS_SESSION_ID in this tool call (mcp-remote does not forward HTTP headers)")).into_response(),
                        };
                        match sm_repo::find_all(&pool, &sid).await {
                            Ok(rows) => jsonrpc_success(request.id, json!({ "content": [{ "type": "text", "text": serde_json::to_string(&rows).unwrap_or_else(|_| "[]".to_string()) }] })),
                            Err(e) => jsonrpc_error(request.id, -32603, &e.to_string()),
                        }
                    }
                }
                "get_memory" => {
                    use crate::repositories::session_memory as sm_repo;
                    let scope = arguments.get("scope").and_then(|v| v.as_str()).unwrap_or("session");
                    let key = arguments.get("key").and_then(|v| v.as_str()).unwrap_or("");
                    if key.is_empty() {
                        return Json(jsonrpc_error(request.id, -32602, errors::MISSING_PARAMS)).into_response();
                    }
                    if scope == "project" {
                        let project_id_raw = arguments.get("projectId").and_then(|v| v.as_str()).unwrap_or("");
                        let resolved = match crate::repositories::project::resolve_id(&pool, project_id_raw).await {
                            Ok(id) => id,
                            Err(_) => return Json(jsonrpc_error(request.id, -32602, "Project not found")).into_response(),
                        };
                        match crate::repositories::memory::find_value(&pool, &resolved, key).await {
                            Ok(Some(v)) => jsonrpc_success(request.id, json!({ "content": [{ "type": "text", "text": v }] })),
                            Ok(None) => jsonrpc_error(request.id, -32602, errors::KEY_NOT_FOUND),
                            Err(e) => jsonrpc_error(request.id, -32603, &e.to_string()),
                        }
                    } else {
                        let sid = match session_repo::resolve_session_id(&pool, session_id_from_header, session_id_from_header).await {
                            Some(s) => s,
                            None => return Json(jsonrpc_error(request.id, -32602, "No session ID — pass sessionId=$ATLAS_SESSION_ID in this tool call (mcp-remote does not forward HTTP headers)")).into_response(),
                        };
                        match sm_repo::find_value(&pool, &sid, key).await {
                            Ok(Some(v)) => jsonrpc_success(request.id, json!({ "content": [{ "type": "text", "text": v }] })),
                            Ok(None) => jsonrpc_error(request.id, -32602, errors::KEY_NOT_FOUND),
                            Err(e) => jsonrpc_error(request.id, -32603, &e.to_string()),
                        }
                    }
                }
                "set_memory" => {
                    use crate::repositories::session_memory as sm_repo;
                    let scope = arguments.get("scope").and_then(|v| v.as_str()).unwrap_or("session");
                    let key = arguments.get("key").and_then(|v| v.as_str()).unwrap_or("");
                    let value = arguments.get("value").and_then(|v| v.as_str()).unwrap_or("");
                    if key.is_empty() {
                        return Json(jsonrpc_error(request.id, -32602, errors::MISSING_PARAMS)).into_response();
                    }
                    if scope == "project" {
                        let project_id_raw = arguments.get("projectId").and_then(|v| v.as_str()).unwrap_or("");
                        let resolved = match crate::repositories::project::resolve_id(&pool, project_id_raw).await {
                            Ok(id) => id,
                            Err(_) => return Json(jsonrpc_error(request.id, -32602, "Project not found")).into_response(),
                        };
                        match crate::services::memory::set(&pool, &resolved, key, value).await {
                            Ok(()) => jsonrpc_success(request.id, json!({ "content": [{ "type": "text", "text": format!("Project memory key '{}' saved", key) }] })),
                            Err(e) => jsonrpc_error(request.id, -32603, &e.to_string()),
                        }
                    } else {
                        let sid = match session_repo::resolve_session_id(&pool, session_id_from_header, session_id_from_header).await {
                            Some(s) => s,
                            None => return Json(jsonrpc_error(request.id, -32602, "No session ID — pass sessionId=$ATLAS_SESSION_ID in this tool call (mcp-remote does not forward HTTP headers)")).into_response(),
                        };
                        let id = ulid::Ulid::new().to_string();
                        match sm_repo::upsert(&pool, &id, &sid, key, value).await {
                            Ok(()) => jsonrpc_success(request.id, json!({ "content": [{ "type": "text", "text": format!("Session memory key '{}' saved", key) }] })),
                            Err(e) => jsonrpc_error(request.id, -32603, &e.to_string()),
                        }
                    }
                }
                "global_list_memory" => {
                    match global_repo::list_memory_mcp(&pool).await {
                        Ok(r) => jsonrpc_success(request.id, json!({ "content": [{ "type": "text", "text": serde_json::to_string(&r).unwrap_or_else(|_| "[]".to_string()) }] })),
                        Err(e) => jsonrpc_error(request.id, -32603, &e.to_string()),
                    }
                }
                "global_list_skills" => {
                    match global_repo::list_skills_mcp(&pool).await {
                        Ok(r) => jsonrpc_success(request.id, json!({ "content": [{ "type": "text", "text": serde_json::to_string(&r).unwrap_or_else(|_| "[]".to_string()) }] })),
                        Err(e) => jsonrpc_error(request.id, -32603, &e.to_string()),
                    }
                }
                "global_list_prompts" => {
                    match global_repo::list_prompts_mcp(&pool).await {
                        Ok(r) => jsonrpc_success(request.id, json!({ "content": [{ "type": "text", "text": serde_json::to_string(&r).unwrap_or_else(|_| "[]".to_string()) }] })),
                        Err(e) => jsonrpc_error(request.id, -32603, &e.to_string()),
                    }
                }
                "spawn_session" => {
                    let project_id = match caller_project_id.as_deref() {
                        Some(id) => id.to_string(),
                        None => return Json(jsonrpc_error(request.id, -32603, "Cannot spawn session: caller project could not be resolved")).into_response(),
                    };
                    let slug = match project_repo::find_slug_by_id(&pool, &project_id).await {
                        Ok(Some(s)) => s,
                        _ => return Json(jsonrpc_error(request.id, -32603, "Project not found")).into_response(),
                    };

                    let default_author = std::env::var(env::ATLAS_DEFAULT_AUTHOR)
                        .unwrap_or_else(|_| defaults::AUTHOR.to_string());

                    let title = arguments.get("title").and_then(|v| v.as_str())
                        .unwrap_or("worker").to_string();
                    let provider = arguments.get("provider").and_then(|v| v.as_str())
                        .unwrap_or("claude").to_string();
                    let model = arguments.get("model").and_then(|v| v.as_str())
                        .unwrap_or("claude-sonnet-4-6").to_string();
                    let working_directory = arguments.get("workingDirectory").and_then(|v| v.as_str())
                        .unwrap_or("").to_string();

                    let req = CreateSessionRequest { provider, model, mode: "agent".to_string(), working_directory, title };
                    match session_svc::create(&pool, &slug, &req, &default_author).await {
                        Ok(Some(session)) => jsonrpc_success(
                            request.id,
                            json!({ "content": [{ "type": "text", "text": format!("Session spawned. id={} title={}", session.id, session.title.unwrap_or_default()) }] }),
                        ),
                        Ok(None) => jsonrpc_error(request.id, -32603, "Project not found"),
                        Err(e) => jsonrpc_error(request.id, -32603, &e.to_string()),
                    }
                }
                "close_session" => {
                    let target_id = arguments.get("sessionId").and_then(|v| v.as_str()).unwrap_or("");
                    if target_id.is_empty() {
                        return Json(jsonrpc_error(request.id, -32602, errors::MISSING_PARAMS)).into_response();
                    }
                    // Project isolation: only allow closing sessions in the same project.
                    if let Some(ref my_pid) = caller_project_id {
                        let target_pid = session_repo::find_project_id(&pool, target_id).await.ok().flatten();
                        if target_pid.as_deref() != Some(my_pid.as_str()) {
                            return Json(jsonrpc_error(request.id, -32603, "Cross-project session close is not allowed")).into_response();
                        }
                    }
                    tm.kill_session(target_id).await;
                    match session_svc::delete(&pool, target_id).await {
                        Ok(()) => jsonrpc_success(
                            request.id,
                            json!({ "content": [{ "type": "text", "text": format!("Session {} closed", target_id) }] }),
                        ),
                        Err(e) => jsonrpc_error(request.id, -32603, &e.to_string()),
                    }
                }
                "get_document_links" => {
                    use crate::repositories::session_document as sd_repo;
                    let id = arguments.get("id").and_then(|v| v.as_str()).unwrap_or("");
                    if id.is_empty() {
                        return Json(jsonrpc_error(request.id, -32602, errors::MISSING_PARAMS)).into_response();
                    }

                    let (root_title, root_content, root_links) =
                        if let Ok(Some(d)) = doc_repo::find_full_by_id(&pool, id).await {
                            (d.title, d.content, d.links)
                        } else {
                            match sqlx::query_as::<_, (String, String, String)>(
                                "SELECT title, content, links FROM session_documents WHERE id = ?",
                            )
                            .bind(id)
                            .fetch_optional(&pool)
                            .await
                            {
                                Ok(Some((t, c, l))) => (t, c, l),
                                _ => return Json(jsonrpc_error(request.id, -32602, "Document not found")).into_response(),
                            }
                        };

                    let linked_proj = doc_repo::find_linked(&pool, &root_links).await.unwrap_or_default();
                    let linked_sess = sd_repo::find_linked(&pool, &root_links).await.unwrap_or_default();

                    let linked: Vec<serde_json::Value> = linked_proj.iter()
                        .map(|d| json!({ "id": d.id, "title": d.title, "kind": d.kind, "content": d.content, "links": d.links }))
                        .chain(linked_sess.iter().map(|d| json!({ "id": d.id, "title": d.title, "kind": d.kind, "content": d.content, "links": d.links })))
                        .collect();

                    let result = json!({
                        "root": { "id": id, "title": root_title, "content": root_content, "links": root_links },
                        "linked": linked
                    });
                    jsonrpc_success(
                        request.id,
                        json!({ "content": [{ "type": "text", "text": serde_json::to_string(&result).unwrap_or_default() }] }),
                    )
                }
                _ => jsonrpc_error(request.id, -32601, "Tool not found"),
            }
        }
        _ => jsonrpc_error(request.id, -32601, "Method not found"),
    };

    if is_notification {
        (ax_http::StatusCode::ACCEPTED, "").into_response()
    } else {
        Json(result).into_response()
    }
}

// SSE endpoint for MCP Streamable HTTP transport (GET /api/mcp).
// Claude Code opens this to receive server-initiated messages; Atlas has none,
// so we just hold the connection open with keepalive pings.
pub async fn handle_mcp_sse() -> Sse<impl futures::Stream<Item = Result<Event, Infallible>>> {
    Sse::new(stream::pending::<Result<Event, Infallible>>())
        .keep_alive(KeepAlive::new().interval(Duration::from_secs(25)).text("ping"))
}
