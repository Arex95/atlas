//! AFG runtime — the orchestration logic that ties parsing,
//! persistence, message dispatch and gates together.
//!
//! Everything flows over `atlas-messaging`'s bus: dispatching a node
//! is sending a `task` message; an agent reports back via
//! `submit_task_result`, which corresponds to a `task_result`
//! message the caller already sent through the normal messaging
//! tools (or, in this issue, delivers directly as call arguments —
//! there is no PTY-injection wiring here, the target session's
//! owner discovers the task by polling their own inbox, same as any
//! other coordination message).

use std::path::{Component, Path, PathBuf};

use atlas_messaging::api::{MessageStore, NewMessage};
use atlas_sessions::api::{Caller, SessionId, SessionStore, SessionsError};
use serde_json::Value;

use crate::internal::domain::{
    AfgError, NodeEventKind, RunDetail, RunId, RunStatus, ToolScope, Workflow, WorkflowId,
    WorkflowNodeSpec, WorkflowRun, WorkflowSpec, parse_spec, pick_next_node,
};
use crate::internal::infrastructure::{AfgStore, GateInput, GateOutcome, run_gate};

const MESSAGE_TYPE_TASK: &str = "task";

/// Everything the runtime needs. Cheap to clone (every field is
/// itself a cheap-clone handle over a shared pool/connection).
#[derive(Clone)]
pub struct AfgRuntime {
    store: AfgStore,
    sessions: SessionStore,
    messages: MessageStore,
}

impl AfgRuntime {
    #[must_use]
    pub fn new(store: AfgStore, sessions: SessionStore, messages: MessageStore) -> Self {
        Self {
            store,
            sessions,
            messages,
        }
    }

    /// Register (or update) a workflow for `project` from either an
    /// inline YAML body or a project-relative source path.
    ///
    /// # Errors
    /// `MissingSource`/`PathOutsideProject` on bad input,
    /// `Parse`/`Validation` on a bad spec, `Io` if `source_path`
    /// can't be read, `Storage` on any SQL failure.
    pub async fn register_workflow(
        &self,
        project: &str,
        project_root: &str,
        source_path: Option<&str>,
        inline_yaml: Option<&str>,
    ) -> Result<Workflow, AfgError> {
        let (yaml, stored_source_path) = match (inline_yaml, source_path) {
            (Some(y), sp) => (y.to_owned(), sp.map(str::to_owned)),
            (None, Some(sp)) => {
                let joined = resolve_project_path(project_root, sp)?;
                let text = tokio::fs::read_to_string(&joined)
                    .await
                    .map_err(|e| AfgError::Io(e.to_string()))?;
                (text, Some(sp.to_owned()))
            }
            (None, None) => return Err(AfgError::MissingSource),
        };

        let spec = parse_spec(&yaml)?;
        self.store
            .upsert_workflow(project, project_root, stored_source_path.as_deref(), &spec)
            .await
    }

    /// Re-read a workflow's source file and adopt it if it changed.
    ///
    /// This is what makes a committed file the source of truth rather
    /// than a copy taken once: the rules a team agreed on live in the
    /// repository, and every machine reads them from there.
    ///
    /// Three cases are deliberately *not* errors, and each returns the
    /// workflow untouched: a workflow registered from inline YAML
    /// (there is no file to read), one that predates recorded project
    /// roots, and a file whose contents parse to the spec already
    /// stored. The last one matters because re-registering would bump
    /// the version on every run and turn the version into a run
    /// counter.
    ///
    /// A file that is *present but broken* is an error. Falling back
    /// to the stored spec would run rules the repository no longer
    /// contains, which is the exact failure this exists to prevent.
    async fn refresh_from_source(&self, workflow: Workflow) -> Result<Workflow, AfgError> {
        let (Some(source_path), Some(project_root)) = (
            workflow.source_path.as_ref(),
            workflow.project_root.as_ref(),
        ) else {
            return Ok(workflow);
        };

        let joined = resolve_project_path(project_root, source_path)?;
        let text = tokio::fs::read_to_string(&joined)
            .await
            .map_err(|e| AfgError::Io(format!("{source_path}: {e}")))?;
        let spec = parse_spec(&text)?;
        if spec == workflow.spec {
            return Ok(workflow);
        }

        self.store
            .upsert_workflow(&workflow.project, project_root, Some(source_path), &spec)
            .await
    }

    /// Start a new run: create the row, pick the first runnable
    /// node, dispatch it as a `task` message, record an `enter`
    /// event.
    ///
    /// # Errors
    /// `NotFound` if the workflow doesn't exist, `Validation` if the
    /// spec has no runnable start node, `Storage` on any SQL
    /// failure.
    pub async fn start_run(
        &self,
        workflow_id: &WorkflowId,
        initiator_session_id: &str,
        target_session_id: Option<&str>,
        caller: &Caller,
    ) -> Result<WorkflowRun, AfgError> {
        self.ensure_session_owned(initiator_session_id, caller.owner_id())
            .await?;
        if let Some(target) = target_session_id {
            self.ensure_session_owned(target, caller.owner_id()).await?;
        }

        let workflow = self.store.get_workflow(workflow_id).await?;
        // The repository is the source of truth, so the file is read
        // now rather than trusted from registration time. A teammate
        // who pulled a changed workflow runs the changed one without
        // having to remember to re-register it.
        let workflow = self.refresh_from_source(workflow).await?;
        let run = self
            .store
            .create_run(workflow_id, initiator_session_id, &workflow.spec)
            .await?;

        let first_node = pick_next_node(&workflow.spec, &[])
            .ok_or_else(|| AfgError::Validation("no runnable start node".to_owned()))?;

        let target = target_session_id.unwrap_or(initiator_session_id);
        self.dispatch_node(&workflow, &run, first_node, target, None)
            .await?;

        self.store.get_run(&run.id).await
    }

    /// Called when an agent reports the result of the run's current
    /// node. Records `exec`, runs the node's gates in order, then
    /// advances/retries/fails accordingly.
    ///
    /// # Errors
    /// `NotFound` if the run/workflow doesn't exist, `Validation` if
    /// `node_id` isn't the run's current node or isn't in the spec,
    /// `Storage` on any SQL failure.
    /// What the session is allowed to call right now.
    ///
    /// `None` when the session is not executing a node, which is the
    /// ordinary case and means no scope applies. A node that declared
    /// nothing also yields an unrestricted scope rather than `None`,
    /// so the two are distinguishable to a caller that cares.
    ///
    /// **Not a security boundary.** The session's agent holds a
    /// terminal and can do anything its user can; this limits
    /// accidents on the one surface Atlas controls.
    ///
    /// # Errors
    /// `Storage` on any SQL failure. A spec that no longer parses
    /// yields `None` rather than an error: refusing every tool because
    /// a workflow is malformed would strand an agent mid-run over a
    /// problem it cannot fix.
    pub async fn tool_scope_for_session(
        &self,
        session_id: &str,
    ) -> Result<Option<ToolScope>, AfgError> {
        let Some((spec_json, node_id)) = self.store.current_node_for_session(session_id).await?
        else {
            return Ok(None);
        };
        // A spec that no longer parses means the code that wrote it
        // and the code reading it disagree. Refusing every tool over
        // that would strand an agent mid-run on a problem it cannot
        // fix, so it goes unscoped — and `graph.findings`-style
        // reporting of a broken workflow is a separate concern.
        let Ok(spec) = serde_json::from_str::<WorkflowSpec>(&spec_json) else {
            return Ok(None);
        };
        Ok(spec
            .nodes
            .iter()
            .find(|n| n.id == node_id)
            .map(|n| ToolScope::new(n.allowed_tools.clone())))
    }

    /// Accepts a node's result from the session that node was
    /// dispatched to, and from no other.
    ///
    /// `project_root` is deliberately **not** a parameter. It used to
    /// be, and it became the working directory of the shell an
    /// acceptance gate runs — so a caller chose where somebody else's
    /// gate executed. It now comes from the workflow, recorded once at
    /// registration.
    ///
    /// # Errors
    /// `NotFound` when the run does not exist *or* the caller is not
    /// the session it was dispatched to — the two are deliberately
    /// indistinguishable, so a caller cannot discover that a run it
    /// may not touch exists. `Validation` if the run is not running or
    /// the node is not in the spec.
    pub async fn submit_task_result(
        &self,
        run_id: &RunId,
        node_id: &str,
        caller: &Caller,
        payload: Option<&Value>,
    ) -> Result<WorkflowRun, AfgError> {
        let run = self.store.get_run(run_id).await?;

        // Authorisation before anything else, including before the
        // status check: telling an unauthorised caller that a run
        // exists but has already finished is still telling them it
        // exists.
        let Some(target) = run.target_session_id.as_deref() else {
            // No node has been dispatched, so no session has been
            // asked for a result. Nobody is authorised to give one.
            return Err(AfgError::NotFound("run"));
        };
        if caller.session_id() != Some(target) {
            return Err(AfgError::NotFound("run"));
        }

        if run.status != RunStatus::Running {
            return Err(AfgError::Validation(format!(
                "run is {}, not running — task_result ignored",
                run.status.as_str()
            )));
        }

        // The node comes from the run, not from the caller.
        //
        // Reading it from the parameter let an agent dispatched one
        // node answer for another: the other node's gates ran, its
        // `advance` was recorded, and the run moved to a step nobody
        // had been asked to do. The run has always known which node it
        // sent out; it simply was not consulting itself.
        //
        // The parameter stays and is checked rather than ignored. An
        // agent that believes it is on a different node has a real
        // problem, and silently accepting the right answer to the
        // wrong question would hide it.
        let Some(current) = run.current_node_id.as_deref() else {
            return Err(AfgError::Validation(
                "this run has no current node — nothing was asked for".to_owned(),
            ));
        };
        if node_id != current {
            return Err(AfgError::Validation(format!(
                "this run is on node '{current}', not '{node_id}' — a result may only \
                 answer the node that was dispatched"
            )));
        }

        let workflow = self.store.workflow_for_run(&run).await?;
        let project_root = workflow.project_root.clone().ok_or_else(|| {
            // Registered before project_root was recorded. Refusing is
            // the only safe answer: guessing a directory to run a
            // shell command in is exactly the defect this replaced.
            AfgError::Validation(
                "this workflow predates recorded project roots; re-register it before running its \
                 acceptance gates"
                    .to_owned(),
            )
        })?;
        let project_root = project_root.as_str();
        let node_spec = workflow
            .spec
            .nodes
            .iter()
            .find(|n| n.id == node_id)
            .ok_or_else(|| AfgError::Validation(format!("node '{node_id}' not in spec")))?
            .clone();

        self.store
            .insert_event(run_id, node_id, NodeEventKind::Exec, payload, None)
            .await?;

        let gate_input = GateInput {
            project_root,
            run_id: &run_id.0,
            node_id,
            task_result_payload: payload,
        };

        let mut failure: Option<(String, Option<String>)> = None;
        for criterion in &node_spec.acceptance_criteria {
            let outcome = run_gate(&gate_input, criterion).await;
            match outcome {
                GateOutcome::Pass { summary } => {
                    self.store
                        .insert_event(
                            run_id,
                            node_id,
                            NodeEventKind::GatePass,
                            Some(&serde_json::json!({ "summary": summary })),
                            None,
                        )
                        .await?;
                }
                GateOutcome::Fail { reason, detail } => {
                    self.store
                        .insert_event(
                            run_id,
                            node_id,
                            NodeEventKind::GateFail,
                            Some(&serde_json::json!({ "reason": reason, "detail": detail })),
                            None,
                        )
                        .await?;
                    failure = Some((reason, detail));
                    break;
                }
            }
        }

        if let Some((reason, detail)) = failure {
            return self
                .handle_gate_failure(&workflow, &run, &node_spec, &reason, detail.as_deref())
                .await;
        }

        self.advance_or_complete(&workflow, &run, &node_spec).await
    }

    /// # Errors
    /// `NotFound` if no run has this id, `Storage` on any SQL
    /// failure.
    pub async fn get_run(&self, run_id: &RunId) -> Result<RunDetail, AfgError> {
        self.store.get_run_detail(run_id).await
    }

    /// # Errors
    /// Any SQL failure.
    pub async fn list_runs(
        &self,
        workflow_id: &WorkflowId,
        status: Option<RunStatus>,
    ) -> Result<Vec<WorkflowRun>, AfgError> {
        self.store.list_runs(workflow_id, status).await
    }

    /// Checks a session exists *and* belongs to the caller.
    ///
    /// Owner-scoped, because starting a run on a session dispatches a
    /// `task` message into its inbox — so an unscoped check let any
    /// caller inject work into another developer's agent.
    async fn ensure_session_owned(&self, session_id: &str, owner_id: &str) -> Result<(), AfgError> {
        self.sessions
            .get(&SessionId(session_id.to_owned()), owner_id)
            .await
            .map_err(|e| match e {
                SessionsError::NotFound => AfgError::NotFound("session"),
                other => AfgError::Storage(other.to_string()),
            })?;
        Ok(())
    }

    async fn handle_gate_failure(
        &self,
        workflow: &Workflow,
        run: &WorkflowRun,
        node_spec: &WorkflowNodeSpec,
        reason: &str,
        detail: Option<&str>,
    ) -> Result<WorkflowRun, AfgError> {
        let retries = self.store.retry_count(&run.id, &node_spec.id).await?;
        if retries < node_spec.max_retries {
            self.store
                .insert_event(
                    &run.id,
                    &node_spec.id,
                    NodeEventKind::Retry,
                    Some(&serde_json::json!({
                        "attempt": retries + 1,
                        "maxRetries": node_spec.max_retries,
                        "reason": reason,
                    })),
                    None,
                )
                .await?;
            let feedback = build_retry_feedback(reason, detail, retries + 1, node_spec.max_retries);
            self.dispatch_node(
                workflow,
                run,
                node_spec,
                &run.initiator_session_id,
                Some(&feedback),
            )
            .await?;
        } else {
            self.store
                .insert_event(
                    &run.id,
                    &node_spec.id,
                    NodeEventKind::Error,
                    Some(&serde_json::json!({
                        "reason": format!("max_retries ({}) exhausted: {reason}", node_spec.max_retries),
                    })),
                    None,
                )
                .await?;
            self.store.fail_run(&run.id).await?;
        }
        self.store.get_run(&run.id).await
    }

    async fn advance_or_complete(
        &self,
        workflow: &Workflow,
        run: &WorkflowRun,
        node_spec: &WorkflowNodeSpec,
    ) -> Result<WorkflowRun, AfgError> {
        let completed = self.store.completed_node_ids(&run.id).await?;
        let mut completed_with_current = completed;
        // `completed_node_ids` reads `advance` events, which are
        // recorded below for the node that just passed — so we must
        // include it here before asking what runs next.
        completed_with_current.push(node_spec.id.clone());

        if let Some(next) = pick_next_node(&workflow.spec, &completed_with_current) {
            self.store
                .insert_event(
                    &run.id,
                    &node_spec.id,
                    NodeEventKind::Advance,
                    Some(&serde_json::json!({ "to": next.id })),
                    None,
                )
                .await?;
            self.dispatch_node(workflow, run, next, &run.initiator_session_id, None)
                .await?;
        } else {
            self.store.complete_run(&run.id).await?;
            self.store
                .insert_event(&run.id, &node_spec.id, NodeEventKind::Complete, None, None)
                .await?;
        }
        self.store.get_run(&run.id).await
    }

    async fn dispatch_node(
        &self,
        workflow: &Workflow,
        run: &WorkflowRun,
        node: &WorkflowNodeSpec,
        target_session_id: &str,
        retry_feedback: Option<&str>,
    ) -> Result<(), AfgError> {
        self.store.set_current_node(&run.id, Some(&node.id)).await?;
        // Recorded on every dispatch, retries included: a retry may go
        // to a different session, and the previous target must stop
        // being able to answer the moment it does.
        self.store
            .set_target_session(&run.id, target_session_id)
            .await?;

        self.store
            .insert_event(
                &run.id,
                &node.id,
                NodeEventKind::Enter,
                Some(&serde_json::json!({
                    "title": node.title,
                    "target": target_session_id,
                    "isRetry": retry_feedback.is_some(),
                })),
                None,
            )
            .await?;

        let payload = serde_json::json!({
            "runId": run.id.0,
            "workflowId": run.workflow_id.0,
            "workflowName": workflow.name,
            "nodeId": node.id,
            "title": node.title,
            "instructions": node.instructions,
            "dependsOn": node.depends_on,
            "retryFeedback": retry_feedback,
        });

        self.messages
            .send(
                &workflow.project,
                NewMessage {
                    from_session: "afg".to_owned(),
                    to_session: Some(target_session_id.to_owned()),
                    message_type: MESSAGE_TYPE_TASK.to_owned(),
                    payload,
                    correlation_id: Some(run.correlation_id.clone()),
                    reply_to: None,
                },
            )
            .await
            .map_err(|e| AfgError::Storage(e.to_string()))?;

        Ok(())
    }
}

fn build_retry_feedback(reason: &str, detail: Option<&str>, attempt: u32, max: u32) -> String {
    let mut msg = format!("Gate failed on attempt {attempt}/{max}. Reason: {reason}");
    if let Some(d) = detail
        && !d.is_empty()
    {
        msg.push_str("\n\nDetails:\n");
        msg.push_str(d);
    }
    msg.push_str("\n\nTry again — address the failure above before sending the next task_result.");
    msg
}

fn resolve_project_path(root: &str, rel: &str) -> Result<PathBuf, AfgError> {
    let rel_path = Path::new(rel);
    if rel_path.is_absolute() {
        return Err(AfgError::PathOutsideProject);
    }
    for component in rel_path.components() {
        if matches!(component, Component::ParentDir | Component::Prefix(_)) {
            return Err(AfgError::PathOutsideProject);
        }
    }
    Ok(PathBuf::from(root).join(rel_path))
}
