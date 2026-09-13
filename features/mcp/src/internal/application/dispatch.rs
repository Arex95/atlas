//! JSON-RPC method dispatch for the MCP endpoint.
//!
//! Two kinds of methods:
//! - Protocol methods (`initialize`, `tools/list`, `tools/call`).
//!   `initialize` and `tools/list` return static-ish shapes;
//!   `tools/call` dispatches by name into `tracker_tools`.
//! - Anything else → `-32601` method not found.

use atlas_afg::api::AfgRuntime;
use atlas_graph::api::{GraphStore, GraphWatcher};
use atlas_memory::api::MemoryStore;
use atlas_messaging::api::MessageStore;
use atlas_notes::api::NoteStore;
use atlas_sessions::api::{Caller, SessionStore};
use atlas_sync::api::SyncSupervisor;
use atlas_terminal::api::PtyPool;
use atlas_tracker::api::TrackerRuntime;
use serde_json::{Value, json};

use crate::internal::domain::{
    PROTOCOL_VERSION, SERVER_NAME, SERVER_VERSION, TOOLS, ToolName, tools::tool_schema,
};
use crate::internal::infrastructure::jsonrpc::{JsonRpcError, afg_error_to_jsonrpc, code};

use super::{
    afg_tools, graph_tools, memory_tools, messaging_tools, notes_tools, session_tools, sync_tools,
    terminal_tools, tracker_tools,
};

#[allow(clippy::too_many_arguments)]
pub async fn handle_request(
    runtime: &TrackerRuntime,
    messages: &MessageStore,
    sessions: &SessionStore,
    terminals: &PtyPool,
    afg: &AfgRuntime,
    sync: &SyncSupervisor,
    memory: &MemoryStore,
    notes: &NoteStore,
    graph: &GraphStore,
    graph_watcher: &GraphWatcher,
    caller: &Caller,
    method: &str,
    params: Value,
) -> Result<Value, JsonRpcError> {
    match method {
        "initialize" => Ok(initialize_response()),
        "tools/list" => Ok(tools_list_response()),
        "tools/call" => {
            tools_call(
                runtime,
                messages,
                sessions,
                terminals,
                afg,
                sync,
                memory,
                notes,
                graph,
                graph_watcher,
                caller,
                params,
            )
            .await
        }
        other => Err(JsonRpcError::new(
            code::METHOD_NOT_FOUND,
            format!("unknown method {other:?}"),
        )),
    }
}

fn initialize_response() -> Value {
    json!({
        "protocolVersion": PROTOCOL_VERSION,
        "capabilities": { "tools": {} },
        "serverInfo": { "name": SERVER_NAME, "version": SERVER_VERSION }
    })
}

fn tools_list_response() -> Value {
    let tools: Vec<Value> = TOOLS.iter().copied().map(tool_schema).collect();
    json!({ "tools": tools })
}

/// Refuses a tool the node this session is executing did not ask for.
///
/// **Not a security boundary, and it must not be described as one.**
/// The session's agent holds a terminal and can do anything its user
/// can; a list of tool names does not change that. What it does is
/// keep an accident on the one surface Atlas controls — a node that
/// says "run the tests" has no business closing an issue, whether it
/// decided to through a misread instruction or another agent's
/// message.
///
/// Only sessions are scoped. A caller holding the shared token is not
/// executing a node, so there is nothing to scope it against.
async fn enforce_tool_scope(
    afg: &AfgRuntime,
    caller: &Caller,
    tool: &str,
) -> Result<(), JsonRpcError> {
    let Some(session_id) = caller.session_id() else {
        return Ok(());
    };
    let scope = afg
        .tool_scope_for_session(session_id)
        .await
        .map_err(|e| afg_error_to_jsonrpc(&e))?;
    let Some(scope) = scope else { return Ok(()) };

    if scope.permits(tool) {
        return Ok(());
    }
    Err(JsonRpcError::new(
        code::INVALID_REQUEST,
        format!(
            "{tool} is outside what this workflow node asked for ({}). \
             This limits accidents, not capability — if the node needs it, \
             add it to the node's allowedTools.",
            scope.patterns().join(", ")
        ),
    ))
}

#[allow(clippy::too_many_arguments)]
async fn tools_call(
    runtime: &TrackerRuntime,
    messages: &MessageStore,
    sessions: &SessionStore,
    terminals: &PtyPool,
    afg: &AfgRuntime,
    sync: &SyncSupervisor,
    memory: &MemoryStore,
    notes: &NoteStore,
    graph: &GraphStore,
    graph_watcher: &GraphWatcher,
    caller: &Caller,
    params: Value,
) -> Result<Value, JsonRpcError> {
    let name = params
        .get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| JsonRpcError::new(code::INVALID_PARAMS, "missing tool name"))?;
    let tool = ToolName::from_str(name).ok_or_else(|| {
        JsonRpcError::new(code::METHOD_NOT_FOUND, format!("unknown tool {name:?}"))
    })?;
    let arguments = params
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| json!({}));

    enforce_tool_scope(afg, caller, name).await?;

    let raw = match tool {
        ToolName::TrackerListIssues => tracker_tools::list_issues(runtime, arguments).await?,
        ToolName::TrackerGetIssue => tracker_tools::get_issue(runtime, arguments).await?,
        ToolName::TrackerListRelations => tracker_tools::list_relations(runtime, arguments).await?,
        ToolName::TrackerCreateIssue => tracker_tools::create_issue(runtime, arguments).await?,
        ToolName::TrackerUpdateStatus => tracker_tools::update_status(runtime, arguments).await?,
        ToolName::TrackerCloseIssue => tracker_tools::close_issue(runtime, arguments).await?,
        ToolName::TrackerPlanProgress => tracker_tools::plan_progress(runtime, arguments).await?,
        ToolName::MessagingSendMessage => {
            messaging_tools::send_message(messages, arguments).await?
        }
        ToolName::MessagingReadInbox => {
            messaging_tools::read_inbox(messages, sessions, caller, arguments).await?
        }
        ToolName::SessionsCreate => session_tools::create(sessions, caller, arguments).await?,
        ToolName::SessionsList => session_tools::list(sessions, caller, arguments).await?,
        ToolName::SessionsGet => session_tools::get(sessions, caller, arguments).await?,
        ToolName::SessionsUpdateStatus => {
            session_tools::update_status(sessions, caller, arguments).await?
        }
        ToolName::SessionsSetResumeCommand => {
            session_tools::set_resume_command(sessions, caller, arguments).await?
        }
        ToolName::TerminalSpawn => terminal_tools::spawn(terminals, caller, arguments).await?,
        ToolName::TerminalWrite => terminal_tools::write(terminals, caller, arguments)?,
        ToolName::TerminalReadOutput => terminal_tools::read_output(terminals, caller, arguments)?,
        ToolName::TerminalClose => terminal_tools::close(terminals, caller, arguments)?,
        ToolName::TerminalRestore => terminal_tools::restore(terminals, caller, arguments).await?,
        ToolName::AfgRegisterWorkflow => afg_tools::register_workflow(afg, arguments).await?,
        ToolName::AfgStartRun => afg_tools::start_run(afg, caller, arguments).await?,
        ToolName::AfgSubmitTaskResult => {
            afg_tools::submit_task_result(afg, caller, arguments).await?
        }
        ToolName::AfgGetRun => afg_tools::get_run(afg, arguments).await?,
        ToolName::AfgListRuns => afg_tools::list_runs(afg, arguments).await?,
        ToolName::SyncSessionsNow => sync_tools::sessions_now(sessions, caller, arguments).await?,
        ToolName::SyncMemoryNow => sync_tools::memory_now(memory, caller, arguments).await?,
        ToolName::SyncNotesNow => sync_tools::notes_now(notes, caller, arguments).await?,
        ToolName::SyncSetMode => sync_tools::set_mode(sync, caller, arguments).await?,
        ToolName::GraphReindex => graph_tools::reindex(graph, arguments).await?,
        ToolName::GraphOverview => graph_tools::overview(graph, arguments).await?,
        ToolName::GraphFind => graph_tools::find(graph, arguments).await?,
        ToolName::GraphNode => graph_tools::node(graph, arguments).await?,
        ToolName::GraphRelated => graph_tools::related(graph, arguments).await?,
        ToolName::GraphOutline => graph_tools::outline(graph, arguments).await?,
        ToolName::GraphFindings => graph_tools::findings(graph, arguments).await?,
        ToolName::GraphChanges => graph_tools::changes(graph, arguments).await?,
        ToolName::GraphWatch => graph_tools::watch(graph_watcher, arguments).await?,
        ToolName::GraphUnwatch => graph_tools::unwatch(graph_watcher, arguments).await?,
        ToolName::GraphWatchStatus => graph_tools::watch_status(graph_watcher).await,
        ToolName::MemoryRemember => memory_tools::remember(memory, caller, arguments).await?,
        ToolName::MemoryRecall => memory_tools::recall(memory, caller, arguments).await?,
        ToolName::MemoryList => memory_tools::list(memory, caller, arguments).await?,
        ToolName::MemoryForget => memory_tools::forget(memory, caller, arguments).await?,
        ToolName::NotesWrite => notes_tools::write(notes, caller, arguments).await?,
        ToolName::NotesRead => notes_tools::read(notes, caller, arguments).await?,
        ToolName::NotesList => notes_tools::list(notes, caller, arguments).await?,
        ToolName::NotesDelete => notes_tools::delete(notes, caller, arguments).await?,
        ToolName::SyncStatus => sync_tools::status(sync).await,
    };

    // MCP `tools/call` shape: content is a list of blocks. We
    // return one JSON text block; agents parse the text back into
    // structured data. Structured content lands in a later issue.
    Ok(json!({
        "content": [ {
            "type": "text",
            "text": serde_json::to_string(&raw).unwrap_or_else(|_| "null".to_owned())
        } ],
        "isError": false
    }))
}
