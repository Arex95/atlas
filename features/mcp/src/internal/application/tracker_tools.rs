//! Concrete implementations of the three tracker tools.
//!
//! Each function accepts the tool's declared params as a
//! `serde_json::Value`, validates them into domain types, calls
//! the [`TrackerRuntime`], and returns a `Result<Value, JsonRpcError>`
//! shaped for `tools/call`.

use std::str::FromStr;

use atlas_tracker::api::{
    IssueFilter, IssueId, IssueStatus, Label, NewIssue, ProjectRef, TrackerRuntime,
};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::Value;

use crate::internal::infrastructure::jsonrpc::{JsonRpcError, code, tracker_error_to_jsonrpc};

#[derive(Deserialize)]
struct ListIssuesParams {
    project: String,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    labels: Vec<String>,
    #[serde(default)]
    updated_after: Option<String>,
    #[serde(default)]
    milestone: Option<String>,
}

#[derive(Deserialize)]
struct GetIssueParams {
    project: String,
    id: String,
}

#[derive(Deserialize)]
struct ListRelationsParams {
    project: String,
    id: String,
}

#[derive(Deserialize)]
struct CreateIssueParams {
    project: String,
    title: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    labels: Vec<String>,
}

#[derive(Deserialize)]
struct UpdateStatusParams {
    project: String,
    id: String,
    status: String,
}

#[derive(Deserialize)]
struct CloseIssueParams {
    project: String,
    id: String,
}

fn invalid_params(msg: impl Into<String>) -> JsonRpcError {
    JsonRpcError::new(code::INVALID_PARAMS, msg)
}

fn parse_project(raw: &str) -> Result<ProjectRef, JsonRpcError> {
    ProjectRef::from_str(raw).map_err(|e| invalid_params(format!("bad project: {e}")))
}

fn parse_status(raw: &str) -> Result<IssueStatus, JsonRpcError> {
    match raw {
        "open" => Ok(IssueStatus::Open),
        "closed" => Ok(IssueStatus::Closed),
        other => Err(invalid_params(format!(
            "status must be \"open\" or \"closed\", got {other:?}"
        ))),
    }
}

fn parse_ts(raw: &str) -> Result<DateTime<Utc>, JsonRpcError> {
    DateTime::parse_from_rfc3339(raw)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|e| invalid_params(format!("updated_after must be RFC3339: {e}")))
}

/// Shared by every tool whose params are the `tracker.list_issues`
/// filter shape (`list_issues` and `plan_progress`).
fn parse_filter(params: ListIssuesParams) -> Result<IssueFilter, JsonRpcError> {
    let status = params.status.as_deref().map(parse_status).transpose()?;
    let updated_after = params.updated_after.as_deref().map(parse_ts).transpose()?;
    Ok(IssueFilter {
        status,
        labels: params.labels.into_iter().map(Label).collect(),
        updated_after,
        // Blank is the same as absent: a caller that passed an empty
        // string meant "every milestone", not "the one with no name".
        milestone: params.milestone.filter(|m| !m.trim().is_empty()),
    })
}

pub async fn list_issues(runtime: &TrackerRuntime, params: Value) -> Result<Value, JsonRpcError> {
    let params: ListIssuesParams =
        serde_json::from_value(params).map_err(|e| invalid_params(format!("bad params: {e}")))?;
    let project = parse_project(&params.project)?;
    let filter = parse_filter(params)?;
    let issues = runtime
        .list_issues(&project, &filter)
        .await
        .map_err(|e| tracker_error_to_jsonrpc(&e))?;
    Ok(serde_json::to_value(issues).unwrap_or(Value::Null))
}

pub async fn get_issue(runtime: &TrackerRuntime, params: Value) -> Result<Value, JsonRpcError> {
    let params: GetIssueParams =
        serde_json::from_value(params).map_err(|e| invalid_params(format!("bad params: {e}")))?;
    let project = parse_project(&params.project)?;
    let issue = runtime
        .get_issue(&project, &IssueId(params.id))
        .await
        .map_err(|e| tracker_error_to_jsonrpc(&e))?;
    Ok(serde_json::to_value(issue).unwrap_or(Value::Null))
}

pub async fn list_relations(
    runtime: &TrackerRuntime,
    params: Value,
) -> Result<Value, JsonRpcError> {
    let params: ListRelationsParams =
        serde_json::from_value(params).map_err(|e| invalid_params(format!("bad params: {e}")))?;
    let project = parse_project(&params.project)?;
    let relations = runtime
        .list_relations(&project, &IssueId(params.id))
        .await
        .map_err(|e| tracker_error_to_jsonrpc(&e))?;
    Ok(serde_json::to_value(relations).unwrap_or(Value::Null))
}

pub async fn create_issue(runtime: &TrackerRuntime, params: Value) -> Result<Value, JsonRpcError> {
    let params: CreateIssueParams =
        serde_json::from_value(params).map_err(|e| invalid_params(format!("bad params: {e}")))?;
    let project = parse_project(&params.project)?;
    let input = NewIssue {
        title: params.title,
        description: params.description,
        labels: params.labels.into_iter().map(Label).collect(),
    };
    let issue = runtime
        .create_issue(&project, input)
        .await
        .map_err(|e| tracker_error_to_jsonrpc(&e))?;
    Ok(serde_json::to_value(issue).unwrap_or(Value::Null))
}

pub async fn update_status(runtime: &TrackerRuntime, params: Value) -> Result<Value, JsonRpcError> {
    let params: UpdateStatusParams =
        serde_json::from_value(params).map_err(|e| invalid_params(format!("bad params: {e}")))?;
    let project = parse_project(&params.project)?;
    let status = parse_status(&params.status)?;
    let issue = runtime
        .update_status(&project, &IssueId(params.id), status)
        .await
        .map_err(|e| tracker_error_to_jsonrpc(&e))?;
    Ok(serde_json::to_value(issue).unwrap_or(Value::Null))
}

pub async fn close_issue(runtime: &TrackerRuntime, params: Value) -> Result<Value, JsonRpcError> {
    let params: CloseIssueParams =
        serde_json::from_value(params).map_err(|e| invalid_params(format!("bad params: {e}")))?;
    let project = parse_project(&params.project)?;
    let issue = runtime
        .close_issue(&project, &IssueId(params.id))
        .await
        .map_err(|e| tracker_error_to_jsonrpc(&e))?;
    Ok(serde_json::to_value(issue).unwrap_or(Value::Null))
}

pub async fn plan_progress(runtime: &TrackerRuntime, params: Value) -> Result<Value, JsonRpcError> {
    let params: ListIssuesParams =
        serde_json::from_value(params).map_err(|e| invalid_params(format!("bad params: {e}")))?;
    let project = parse_project(&params.project)?;
    let filter = parse_filter(params)?;
    let progress = runtime
        .plan_progress(&project, &filter)
        .await
        .map_err(|e| tracker_error_to_jsonrpc(&e))?;
    Ok(serde_json::to_value(progress).unwrap_or(Value::Null))
}
