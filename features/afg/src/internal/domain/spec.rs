//! The YAML/JSON shape a workflow author writes. Parsed once at
//! registration time and stored as normalized JSON — the runtime
//! never re-parses YAML per run.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowSpec {
    pub name: String,
    #[serde(default = "default_version")]
    pub version: i64,
    #[serde(default)]
    pub description: Option<String>,
    pub nodes: Vec<WorkflowNodeSpec>,
}

fn default_version() -> i64 {
    1
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowNodeSpec {
    /// Stable identifier, unique within the spec.
    pub id: String,
    pub title: String,
    /// Free-form instructions handed to the executing agent.
    pub instructions: String,
    /// Ids of nodes that must be completed before this one can run.
    #[serde(default)]
    pub depends_on: Vec<String>,
    /// Optional session-agent tag (e.g. "claude", "codex"). Not used
    /// for routing in this issue — the runtime always dispatches to
    /// the run's initiator (or an explicit target session) — but
    /// captured so a future routing slice needs no spec migration.
    #[serde(default)]
    pub agent_kind: Option<String>,
    /// Every criterion must pass before the run advances past this
    /// node; on any failure the same node is redispatched with
    /// feedback in the payload, up to `max_retries` times.
    #[serde(default)]
    pub acceptance_criteria: Vec<AcceptanceCriterion>,
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,
    /// Tool names, or `family.*`, the executing agent is asked to stay
    /// within. Absent means unrestricted, so a workflow written before
    /// this existed behaves exactly as it did.
    ///
    /// **Not containment** — the agent holds a terminal and can do
    /// anything its user can. It limits accidents: a node that says
    /// "run the tests" has no business closing an issue.
    #[serde(default)]
    pub allowed_tools: Vec<String>,
}

fn default_max_retries() -> u32 {
    3
}

/// A single verifiable check the runtime runs after the agent
/// reports its `task_result`. Type-tagged so YAML stays readable;
/// unknown types are rejected at parse time.
///
/// Only `Shell` and `SchemaValidate` land in this issue.
/// `project-map-metric` needs Project Map (0% in this rebuild) and
/// `llm-judge` needs a live model call — both are deliberate
/// follow-ups, not oversights.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum AcceptanceCriterion {
    /// Run a shell command in the project root (or `cwd` if set) and
    /// check its exit code plus an optional stdout/stderr regex.
    Shell {
        command: String,
        #[serde(default)]
        cwd: Option<String>,
        #[serde(default = "default_expect_exit")]
        expect_exit: i32,
        #[serde(default)]
        stdout_matches: Option<String>,
        #[serde(default = "default_shell_timeout")]
        timeout_secs: u64,
    },
    /// JSON-Schema validate the `task_result` payload the agent
    /// replied with.
    SchemaValidate { schema: serde_json::Value },
}

fn default_expect_exit() -> i32 {
    0
}

fn default_shell_timeout() -> u64 {
    60
}
