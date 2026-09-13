//! The tools this MCP server exposes.
//!
//! The name → schema table lives here so dispatch and `tools/list`
//! read from a single source. Adding a tool is one row plus a
//! branch in `tracker_tools::dispatch`.

use serde_json::Value;

mod afg;
mod graph;
mod memory;
mod messaging;
mod notes;
mod sessions;
mod sync;
mod terminal;
mod tracker;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// The `Tracker` prefix keeps parity with the wire tool names
// (`tracker.list_issues` etc.). Dropping it would put readability
// at the variant vs the wire out of sync; new non-tracker tools
// will carry their own natural prefix.
#[allow(clippy::enum_variant_names)]
pub enum ToolName {
    TrackerListIssues,
    TrackerGetIssue,
    TrackerListRelations,
    TrackerCreateIssue,
    TrackerUpdateStatus,
    TrackerCloseIssue,
    TrackerPlanProgress,
    MessagingSendMessage,
    MessagingReadInbox,
    SessionsCreate,
    SessionsList,
    SessionsGet,
    SessionsUpdateStatus,
    SessionsSetResumeCommand,
    TerminalSpawn,
    TerminalWrite,
    TerminalReadOutput,
    TerminalClose,
    TerminalRestore,
    AfgRegisterWorkflow,
    AfgStartRun,
    AfgSubmitTaskResult,
    AfgGetRun,
    AfgListRuns,
    SyncSessionsNow,
    SyncMemoryNow,
    SyncNotesNow,
    SyncSetMode,
    SyncStatus,
    GraphReindex,
    GraphOverview,
    GraphFind,
    GraphNode,
    GraphRelated,
    GraphOutline,
    GraphFindings,
    GraphChanges,
    GraphWatch,
    GraphUnwatch,
    GraphWatchStatus,
    MemoryRemember,
    MemoryRecall,
    MemoryList,
    MemoryForget,
    NotesWrite,
    NotesRead,
    NotesList,
    NotesDelete,
}

impl ToolName {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::TrackerListIssues => "tracker.list_issues",
            Self::TrackerGetIssue => "tracker.get_issue",
            Self::TrackerListRelations => "tracker.list_relations",
            Self::TrackerCreateIssue => "tracker.create_issue",
            Self::TrackerUpdateStatus => "tracker.update_status",
            Self::TrackerCloseIssue => "tracker.close_issue",
            Self::TrackerPlanProgress => "tracker.plan_progress",
            Self::MessagingSendMessage => "messaging.send_message",
            Self::MessagingReadInbox => "messaging.read_inbox",
            Self::SessionsCreate => "sessions.create",
            Self::SessionsList => "sessions.list",
            Self::SessionsGet => "sessions.get",
            Self::SessionsUpdateStatus => "sessions.update_status",
            Self::SessionsSetResumeCommand => "sessions.set_resume_command",
            Self::TerminalSpawn => "terminal.spawn",
            Self::TerminalWrite => "terminal.write",
            Self::TerminalReadOutput => "terminal.read_output",
            Self::TerminalClose => "terminal.close",
            Self::TerminalRestore => "terminal.restore",
            Self::AfgRegisterWorkflow => "afg.register_workflow",
            Self::AfgStartRun => "afg.start_run",
            Self::AfgSubmitTaskResult => "afg.submit_task_result",
            Self::AfgGetRun => "afg.get_run",
            Self::AfgListRuns => "afg.list_runs",
            Self::SyncSessionsNow => "sync.sessions_now",
            Self::SyncMemoryNow => "sync.memory_now",
            Self::SyncNotesNow => "sync.notes_now",
            Self::SyncSetMode => "sync.set_mode",
            Self::SyncStatus => "sync.status",
            Self::GraphReindex => "graph.reindex",
            Self::GraphOverview => "graph.overview",
            Self::GraphFind => "graph.find",
            Self::GraphNode => "graph.node",
            Self::GraphRelated => "graph.related",
            Self::GraphOutline => "graph.outline",
            Self::GraphFindings => "graph.findings",
            Self::GraphChanges => "graph.changes",
            Self::GraphWatch => "graph.watch",
            Self::GraphUnwatch => "graph.unwatch",
            Self::GraphWatchStatus => "graph.watch_status",
            Self::MemoryRemember => "memory.remember",
            Self::MemoryRecall => "memory.recall",
            Self::MemoryList => "memory.list",
            Self::MemoryForget => "memory.forget",
            Self::NotesWrite => "notes.write",
            Self::NotesRead => "notes.read",
            Self::NotesList => "notes.list",
            Self::NotesDelete => "notes.delete",
        }
    }

    #[must_use]
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "tracker.list_issues" => Some(Self::TrackerListIssues),
            "tracker.get_issue" => Some(Self::TrackerGetIssue),
            "tracker.list_relations" => Some(Self::TrackerListRelations),
            "tracker.create_issue" => Some(Self::TrackerCreateIssue),
            "tracker.update_status" => Some(Self::TrackerUpdateStatus),
            "tracker.close_issue" => Some(Self::TrackerCloseIssue),
            "tracker.plan_progress" => Some(Self::TrackerPlanProgress),
            "messaging.send_message" => Some(Self::MessagingSendMessage),
            "messaging.read_inbox" => Some(Self::MessagingReadInbox),
            "sessions.create" => Some(Self::SessionsCreate),
            "sessions.list" => Some(Self::SessionsList),
            "sessions.get" => Some(Self::SessionsGet),
            "sessions.update_status" => Some(Self::SessionsUpdateStatus),
            "sessions.set_resume_command" => Some(Self::SessionsSetResumeCommand),
            "terminal.spawn" => Some(Self::TerminalSpawn),
            "terminal.write" => Some(Self::TerminalWrite),
            "terminal.read_output" => Some(Self::TerminalReadOutput),
            "terminal.close" => Some(Self::TerminalClose),
            "terminal.restore" => Some(Self::TerminalRestore),
            "afg.register_workflow" => Some(Self::AfgRegisterWorkflow),
            "afg.start_run" => Some(Self::AfgStartRun),
            "afg.submit_task_result" => Some(Self::AfgSubmitTaskResult),
            "afg.get_run" => Some(Self::AfgGetRun),
            "afg.list_runs" => Some(Self::AfgListRuns),
            "sync.sessions_now" => Some(Self::SyncSessionsNow),
            "sync.memory_now" => Some(Self::SyncMemoryNow),
            "sync.notes_now" => Some(Self::SyncNotesNow),
            "sync.set_mode" => Some(Self::SyncSetMode),
            "sync.status" => Some(Self::SyncStatus),
            "graph.reindex" => Some(Self::GraphReindex),
            "graph.overview" => Some(Self::GraphOverview),
            "graph.find" => Some(Self::GraphFind),
            "graph.node" => Some(Self::GraphNode),
            "graph.related" => Some(Self::GraphRelated),
            "graph.outline" => Some(Self::GraphOutline),
            "graph.findings" => Some(Self::GraphFindings),
            "graph.changes" => Some(Self::GraphChanges),
            "graph.watch" => Some(Self::GraphWatch),
            "graph.unwatch" => Some(Self::GraphUnwatch),
            "graph.watch_status" => Some(Self::GraphWatchStatus),
            "memory.remember" => Some(Self::MemoryRemember),
            "memory.recall" => Some(Self::MemoryRecall),
            "memory.list" => Some(Self::MemoryList),
            "memory.forget" => Some(Self::MemoryForget),
            "notes.write" => Some(Self::NotesWrite),
            "notes.read" => Some(Self::NotesRead),
            "notes.list" => Some(Self::NotesList),
            "notes.delete" => Some(Self::NotesDelete),
            _ => None,
        }
    }
}

pub const TOOLS: &[ToolName] = &[
    ToolName::TrackerListIssues,
    ToolName::TrackerGetIssue,
    ToolName::TrackerListRelations,
    ToolName::TrackerCreateIssue,
    ToolName::TrackerUpdateStatus,
    ToolName::TrackerCloseIssue,
    ToolName::TrackerPlanProgress,
    ToolName::MessagingSendMessage,
    ToolName::MessagingReadInbox,
    ToolName::SessionsCreate,
    ToolName::SessionsList,
    ToolName::SessionsGet,
    ToolName::SessionsUpdateStatus,
    ToolName::SessionsSetResumeCommand,
    ToolName::TerminalSpawn,
    ToolName::TerminalWrite,
    ToolName::TerminalReadOutput,
    ToolName::TerminalClose,
    ToolName::TerminalRestore,
    ToolName::AfgRegisterWorkflow,
    ToolName::AfgStartRun,
    ToolName::AfgSubmitTaskResult,
    ToolName::AfgGetRun,
    ToolName::AfgListRuns,
    ToolName::SyncSessionsNow,
    ToolName::SyncMemoryNow,
    ToolName::SyncNotesNow,
    ToolName::SyncSetMode,
    ToolName::SyncStatus,
    ToolName::GraphReindex,
    ToolName::GraphOverview,
    ToolName::GraphFind,
    ToolName::GraphNode,
    ToolName::GraphRelated,
    ToolName::GraphOutline,
    ToolName::GraphFindings,
    ToolName::GraphChanges,
    ToolName::GraphWatch,
    ToolName::GraphUnwatch,
    ToolName::GraphWatchStatus,
    ToolName::MemoryRemember,
    ToolName::MemoryRecall,
    ToolName::MemoryList,
    ToolName::MemoryForget,
    ToolName::NotesWrite,
    ToolName::NotesRead,
    ToolName::NotesList,
    ToolName::NotesDelete,
];

#[must_use]
pub fn tool_schema(t: ToolName) -> Value {
    match t {
        ToolName::MessagingSendMessage | ToolName::MessagingReadInbox => {
            messaging::messaging_schema(t)
        }
        ToolName::SessionsCreate
        | ToolName::SessionsList
        | ToolName::SessionsGet
        | ToolName::SessionsUpdateStatus
        | ToolName::SessionsSetResumeCommand => sessions::sessions_schema(t),
        ToolName::TerminalSpawn
        | ToolName::TerminalWrite
        | ToolName::TerminalReadOutput
        | ToolName::TerminalClose
        | ToolName::TerminalRestore => terminal::terminal_schema(t),
        ToolName::AfgRegisterWorkflow
        | ToolName::AfgStartRun
        | ToolName::AfgSubmitTaskResult
        | ToolName::AfgGetRun
        | ToolName::AfgListRuns => afg::afg_schema(t),
        ToolName::SyncSessionsNow
        | ToolName::SyncMemoryNow
        | ToolName::SyncNotesNow
        | ToolName::SyncSetMode
        | ToolName::SyncStatus => sync::sync_schema(t),
        ToolName::MemoryRemember
        | ToolName::MemoryRecall
        | ToolName::MemoryList
        | ToolName::MemoryForget => memory::memory_schema(t),
        ToolName::NotesWrite
        | ToolName::NotesRead
        | ToolName::NotesList
        | ToolName::NotesDelete => notes::notes_schema(t),
        ToolName::GraphReindex
        | ToolName::GraphOverview
        | ToolName::GraphFind
        | ToolName::GraphNode
        | ToolName::GraphRelated
        | ToolName::GraphOutline
        | ToolName::GraphFindings
        | ToolName::GraphChanges
        | ToolName::GraphWatch
        | ToolName::GraphUnwatch
        | ToolName::GraphWatchStatus => graph::graph_schema(t),
        _ => tracker::tracker_schema(t),
    }
}
