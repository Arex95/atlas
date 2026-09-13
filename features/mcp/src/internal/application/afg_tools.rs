//! Concrete implementations of the five AFG tools.

use atlas_afg::api::{AfgRuntime, RunId, RunStatus, WorkflowId};
use atlas_sessions::api::Caller;
use serde::Deserialize;
use serde_json::Value;

use crate::internal::infrastructure::jsonrpc::{JsonRpcError, afg_error_to_jsonrpc, code};

#[derive(Deserialize)]
struct RegisterWorkflowParams {
    project: String,
    project_root: String,
    #[serde(default)]
    source_path: Option<String>,
    #[serde(default)]
    yaml: Option<String>,
}

#[derive(Deserialize)]
// Every field names the session/workflow it identifies — the
// `_id` postfix is what makes each one legible from the wire
// schema (`workflow_id`, `session_id`, `target_session_id`), not
// noise to strip.
#[allow(clippy::struct_field_names)]
struct StartRunParams {
    workflow_id: String,
    session_id: String,
    #[serde(default)]
    target_session_id: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SubmitTaskResultParams {
    run_id: String,
    node_id: String,
    #[serde(default)]
    payload: Option<Value>,
}

#[derive(Deserialize)]
struct GetRunParams {
    run_id: String,
}

#[derive(Deserialize)]
struct ListRunsParams {
    workflow_id: String,
    #[serde(default)]
    status: Option<String>,
}

fn invalid_params(msg: impl Into<String>) -> JsonRpcError {
    JsonRpcError::new(code::INVALID_PARAMS, msg)
}

pub async fn register_workflow(afg: &AfgRuntime, params: Value) -> Result<Value, JsonRpcError> {
    let params: RegisterWorkflowParams =
        serde_json::from_value(params).map_err(|e| invalid_params(format!("bad params: {e}")))?;
    let workflow = afg
        .register_workflow(
            &params.project,
            &params.project_root,
            params.source_path.as_deref(),
            params.yaml.as_deref(),
        )
        .await
        .map_err(|e| afg_error_to_jsonrpc(&e))?;
    Ok(serde_json::to_value(workflow).unwrap_or(Value::Null))
}

pub async fn start_run(
    afg: &AfgRuntime,
    caller: &Caller,
    params: Value,
) -> Result<Value, JsonRpcError> {
    let params: StartRunParams =
        serde_json::from_value(params).map_err(|e| invalid_params(format!("bad params: {e}")))?;
    let run = afg
        .start_run(
            &WorkflowId(params.workflow_id),
            &params.session_id,
            params.target_session_id.as_deref(),
            caller,
        )
        .await
        .map_err(|e| afg_error_to_jsonrpc(&e))?;
    Ok(serde_json::to_value(run).unwrap_or(Value::Null))
}

pub async fn submit_task_result(
    afg: &AfgRuntime,
    caller: &Caller,
    params: Value,
) -> Result<Value, JsonRpcError> {
    let params: SubmitTaskResultParams =
        serde_json::from_value(params).map_err(|e| invalid_params(format!("bad params: {e}")))?;
    let run = afg
        .submit_task_result(
            &RunId(params.run_id),
            &params.node_id,
            caller,
            params.payload.as_ref(),
        )
        .await
        .map_err(|e| afg_error_to_jsonrpc(&e))?;
    Ok(serde_json::to_value(run).unwrap_or(Value::Null))
}

pub async fn get_run(afg: &AfgRuntime, params: Value) -> Result<Value, JsonRpcError> {
    let params: GetRunParams =
        serde_json::from_value(params).map_err(|e| invalid_params(format!("bad params: {e}")))?;
    let detail = afg
        .get_run(&RunId(params.run_id))
        .await
        .map_err(|e| afg_error_to_jsonrpc(&e))?;
    Ok(serde_json::to_value(detail).unwrap_or(Value::Null))
}

pub async fn list_runs(afg: &AfgRuntime, params: Value) -> Result<Value, JsonRpcError> {
    let params: ListRunsParams =
        serde_json::from_value(params).map_err(|e| invalid_params(format!("bad params: {e}")))?;
    let status = params
        .status
        .as_deref()
        .map(|s| RunStatus::parse(s).ok_or_else(|| invalid_params(format!("unknown status {s:?}"))))
        .transpose()?;
    let runs = afg
        .list_runs(&WorkflowId(params.workflow_id), status)
        .await
        .map_err(|e| afg_error_to_jsonrpc(&e))?;
    Ok(serde_json::to_value(runs).unwrap_or(Value::Null))
}
