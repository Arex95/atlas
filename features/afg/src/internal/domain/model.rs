use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::spec::WorkflowSpec;

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct WorkflowId(pub String);

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RunId(pub String);

#[derive(Clone, Debug, Serialize)]
pub struct Workflow {
    pub id: WorkflowId,
    pub project: String,
    pub name: String,
    /// Where this workflow's acceptance gates run. Recorded once, at
    /// registration, so a later caller cannot choose the directory a
    /// gate's shell executes in.
    pub project_root: Option<String>,
    pub source_path: Option<String>,
    pub spec: WorkflowSpec,
    pub version: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    Pending,
    Running,
    Failed,
    Completed,
}

impl RunStatus {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Running => "running",
            Self::Failed => "failed",
            Self::Completed => "completed",
        }
    }

    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "pending" => Some(Self::Pending),
            "running" => Some(Self::Running),
            "failed" => Some(Self::Failed),
            "completed" => Some(Self::Completed),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct WorkflowRun {
    pub id: RunId,
    pub workflow_id: WorkflowId,
    pub status: RunStatus,
    pub current_node_id: Option<String>,
    pub correlation_id: String,
    pub initiator_session_id: String,
    /// The session the current node was dispatched to, and the only
    /// one whose task result is accepted. `None` until the first
    /// dispatch — never treated as "anyone".
    pub target_session_id: Option<String>,
    pub started_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeEventKind {
    /// The runtime picked this node as current and dispatched a task.
    Enter,
    /// The agent replied with a `task_result` for this node.
    Exec,
    /// A single acceptance criterion returned pass.
    GatePass,
    /// A single acceptance criterion returned fail. Payload carries
    /// the reason so the retry can inject it back into the agent's
    /// context.
    GateFail,
    /// Runtime is redispatching the same node after a gate failure.
    Retry,
    /// Runtime moved on to the next node.
    Advance,
    /// The run finished successfully at this node.
    Complete,
    /// Something went wrong (missing session, `max_retries`
    /// exhausted, etc.).
    Error,
}

impl NodeEventKind {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Enter => "enter",
            Self::Exec => "exec",
            Self::GatePass => "gate_pass",
            Self::GateFail => "gate_fail",
            Self::Retry => "retry",
            Self::Advance => "advance",
            Self::Complete => "complete",
            Self::Error => "error",
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct WorkflowNodeEvent {
    pub id: String,
    pub run_id: RunId,
    pub node_id: String,
    pub kind: NodeEventKind,
    pub payload: Option<serde_json::Value>,
    pub message_id: Option<String>,
    pub at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize)]
pub struct RunDetail {
    pub run: WorkflowRun,
    pub events: Vec<WorkflowNodeEvent>,
}
