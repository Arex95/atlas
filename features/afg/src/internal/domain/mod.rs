//! The AFG (Agent Flow Graph) vocabulary and pure logic: the spec
//! shape, parsing/validation, and the domain types the runtime
//! persists.

mod error;
mod model;
mod parse;
mod spec;
mod tool_scope;

pub use error::AfgError;
pub use model::{
    NodeEventKind, RunDetail, RunId, RunStatus, Workflow, WorkflowId, WorkflowNodeEvent,
    WorkflowRun,
};
pub use parse::{parse_spec, pick_next_node};
pub use spec::{AcceptanceCriterion, WorkflowNodeSpec, WorkflowSpec};
pub use tool_scope::ToolScope;
