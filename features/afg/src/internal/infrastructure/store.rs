use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::{Row, SqlitePool};
use tokio::sync::broadcast;
use ulid::Ulid;

use crate::internal::domain::{
    AfgError, NodeEventKind, RunDetail, RunId, RunStatus, Workflow, WorkflowId, WorkflowNodeEvent,
    WorkflowRun, WorkflowSpec,
};
use crate::internal::infrastructure::hub::RunEventHub;

/// Owns every SQL statement AFG runs. Pure local persistence, not a
/// hexagonal port (scoped that pattern to the external
/// tracker boundary specifically) — nothing external to swap here.
///
/// It also owns the live-view fan-out, deliberately: [`Self::insert_event`]
/// is the single place a node event becomes real, so publishing there
/// makes "recorded" and "broadcast" the same act. Putting the publish
/// a layer up in the runtime instead would leave eight call sites that
/// each have to remember, and a ninth added later that doesn't.
#[derive(Clone)]
pub struct AfgStore {
    pool: SqlitePool,
    events: RunEventHub,
}

impl AfgStore {
    #[must_use]
    pub fn new(pool: SqlitePool) -> Self {
        Self {
            pool,
            events: RunEventHub::new(),
        }
    }

    /// A receiver for every node event recorded from here on. Live
    /// watchers subscribe *before* reading history, so an event
    /// landing in between is seen rather than lost.
    #[must_use]
    pub fn subscribe_events(&self) -> broadcast::Receiver<WorkflowNodeEvent> {
        self.events.subscribe()
    }

    /// Insert a new workflow, or update the existing row's spec and
    /// bump its version if `(project, name)` already exists.
    ///
    /// # Errors
    /// Any SQL failure, or a spec that fails to serialize.
    pub async fn upsert_workflow(
        &self,
        project: &str,
        project_root: &str,
        source_path: Option<&str>,
        spec: &WorkflowSpec,
    ) -> Result<Workflow, AfgError> {
        let spec_json = serde_json::to_string(spec)
            .map_err(|e| AfgError::Validation(format!("spec not serializable: {e}")))?;
        let now = Utc::now();

        let existing_id: Option<String> =
            sqlx::query_scalar("SELECT id FROM workflows WHERE project = ? AND name = ?")
                .bind(project)
                .bind(&spec.name)
                .fetch_optional(&self.pool)
                .await?;

        let (id, version) = if let Some(id) = existing_id {
            let version: i64 = sqlx::query_scalar("SELECT version FROM workflows WHERE id = ?")
                .bind(&id)
                .fetch_one(&self.pool)
                .await?;
            let version = version + 1;
            sqlx::query(
                "UPDATE workflows SET project_root = ?, source_path = ?, spec_json = ?, version = ?, updated_at = ? WHERE id = ?",
            )
            .bind(project_root)
            .bind(source_path)
            .bind(&spec_json)
            .bind(version)
            .bind(now.to_rfc3339())
            .bind(&id)
            .execute(&self.pool)
            .await?;
            (id, version)
        } else {
            let id = Ulid::new().to_string();
            sqlx::query(
                "INSERT INTO workflows (id, project, project_root, name, source_path, spec_json, version, created_at, updated_at) \
                 VALUES (?, ?, ?, ?, ?, ?, 1, ?, ?)",
            )
            .bind(&id)
            .bind(project)
            .bind(project_root)
            .bind(&spec.name)
            .bind(source_path)
            .bind(&spec_json)
            .bind(now.to_rfc3339())
            .bind(now.to_rfc3339())
            .execute(&self.pool)
            .await?;
            (id, 1)
        };

        Ok(Workflow {
            id: WorkflowId(id),
            project: project.to_owned(),
            name: spec.name.clone(),
            project_root: Some(project_root.to_owned()),
            source_path: source_path.map(str::to_owned),
            spec: spec.clone(),
            version,
            created_at: now,
            updated_at: now,
        })
    }

    /// # Errors
    /// `NotFound` if no workflow has this id, `Storage` on any SQL
    /// failure or an unparsable stored spec.
    pub async fn get_workflow(&self, id: &WorkflowId) -> Result<Workflow, AfgError> {
        let row = sqlx::query(
            "SELECT id, project, project_root, name, source_path, spec_json, version, created_at, updated_at \
             FROM workflows WHERE id = ?",
        )
        .bind(&id.0)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(AfgError::NotFound("workflow"))?;
        workflow_from_row(&row)
    }

    /// # Errors
    /// Any SQL failure.
    /// Create a run, pinning `spec` to it.
    ///
    /// The spec is stored on the run rather than read from the
    /// workflow each time, because the workflow row now tracks a file
    /// in the repository. A second run started after somebody edits
    /// that file refreshes the row — and must not thereby rewrite the
    /// rules of the run already in flight, whose agent was dispatched
    /// under the old ones.
    pub async fn create_run(
        &self,
        workflow_id: &WorkflowId,
        initiator_session_id: &str,
        spec: &WorkflowSpec,
    ) -> Result<WorkflowRun, AfgError> {
        let spec_json = serde_json::to_string(spec)
            .map_err(|e| AfgError::Validation(format!("spec not serializable: {e}")))?;
        let id = RunId(Ulid::new().to_string());
        let correlation_id = format!("wf-{}", id.0);
        let now = Utc::now();
        sqlx::query(
            "INSERT INTO workflow_runs \
             (id, workflow_id, status, correlation_id, initiator_session_id, started_at, updated_at, spec_json) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id.0)
        .bind(&workflow_id.0)
        .bind(RunStatus::Running.as_str())
        .bind(&correlation_id)
        .bind(initiator_session_id)
        .bind(now.to_rfc3339())
        .bind(now.to_rfc3339())
        .bind(&spec_json)
        .execute(&self.pool)
        .await?;

        Ok(WorkflowRun {
            id,
            workflow_id: workflow_id.clone(),
            status: RunStatus::Running,
            current_node_id: None,
            correlation_id,
            initiator_session_id: initiator_session_id.to_owned(),
            target_session_id: None,
            started_at: now,
            updated_at: now,
            completed_at: None,
        })
    }

    /// # Errors
    /// `NotFound` if no run has this id, `Storage` on any SQL
    /// failure.
    /// The workflow a run is executing, with the spec **it started
    /// with** rather than whatever the file says now.
    ///
    /// Everything else — the project root a gate's shell runs in, the
    /// name, the id — comes from the live row, because those describe
    /// where the run is happening rather than what it agreed to do.
    ///
    /// # Errors
    /// `NotFound` if the workflow is gone, `Validation` if the pinned
    /// spec will not parse, `Storage` on any SQL failure.
    pub async fn workflow_for_run(&self, run: &WorkflowRun) -> Result<Workflow, AfgError> {
        let mut workflow = self.get_workflow(&run.workflow_id).await?;
        let pinned: Option<String> =
            sqlx::query_scalar("SELECT spec_json FROM workflow_runs WHERE id = ?")
                .bind(&run.id.0)
                .fetch_optional(&self.pool)
                .await?
                .flatten();
        if let Some(json) = pinned {
            workflow.spec = serde_json::from_str(&json)
                .map_err(|e| AfgError::Validation(format!("pinned spec unreadable: {e}")))?;
        }
        Ok(workflow)
    }

    pub async fn get_run(&self, id: &RunId) -> Result<WorkflowRun, AfgError> {
        let row = sqlx::query(
            "SELECT id, workflow_id, status, current_node_id, correlation_id, target_session_id, \
             initiator_session_id, started_at, updated_at, completed_at \
             FROM workflow_runs WHERE id = ?",
        )
        .bind(&id.0)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(AfgError::NotFound("workflow run"))?;
        run_from_row(&row)
    }

    /// # Errors
    /// `NotFound` if no run has this id, `Storage` on any SQL
    /// failure.
    pub async fn get_run_detail(&self, id: &RunId) -> Result<RunDetail, AfgError> {
        let run = self.get_run(id).await?;
        let rows = sqlx::query(
            "SELECT id, run_id, node_id, kind, payload_json, message_id, at \
             FROM workflow_node_events WHERE run_id = ? ORDER BY at ASC, id ASC",
        )
        .bind(&id.0)
        .fetch_all(&self.pool)
        .await?;
        let events = rows.iter().map(event_from_row).collect::<Result<_, _>>()?;
        Ok(RunDetail { run, events })
    }

    /// # Errors
    /// Any SQL failure.
    pub async fn list_runs(
        &self,
        workflow_id: &WorkflowId,
        status: Option<RunStatus>,
    ) -> Result<Vec<WorkflowRun>, AfgError> {
        let mut sql = String::from(
            "SELECT id, workflow_id, status, current_node_id, correlation_id, target_session_id, \
             initiator_session_id, started_at, updated_at, completed_at \
             FROM workflow_runs WHERE workflow_id = ?",
        );
        if status.is_some() {
            sql.push_str(" AND status = ?");
        }
        sql.push_str(" ORDER BY started_at ASC");

        let mut q = sqlx::query(&sql).bind(&workflow_id.0);
        if let Some(s) = status {
            q = q.bind(s.as_str());
        }
        let rows = q.fetch_all(&self.pool).await?;
        rows.iter().map(run_from_row).collect()
    }

    /// # Errors
    /// Any SQL failure.
    pub async fn set_current_node(
        &self,
        run_id: &RunId,
        node_id: Option<&str>,
    ) -> Result<(), AfgError> {
        sqlx::query("UPDATE workflow_runs SET current_node_id = ?, updated_at = ? WHERE id = ?")
            .bind(node_id)
            .bind(Utc::now().to_rfc3339())
            .bind(&run_id.0)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// The workflow and node a session is currently executing, if it
    /// is executing one.
    ///
    /// One indexed lookup on `target_session_id`, because this is
    /// asked on every MCP call a session makes. A join to the workflow
    /// rather than a second round trip for the same reason.
    ///
    /// # Errors
    /// `Storage` on any SQL failure.
    pub async fn current_node_for_session(
        &self,
        session_id: &str,
    ) -> Result<Option<(String, String)>, AfgError> {
        let row: Option<(String, String)> = sqlx::query_as(
            "SELECT w.spec_json, r.current_node_id \
             FROM workflow_runs r JOIN workflows w ON w.id = r.workflow_id \
             WHERE r.target_session_id = ? AND r.status = 'running' \
               AND r.current_node_id IS NOT NULL \
             LIMIT 1",
        )
        .bind(session_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    /// Records which session the current node was dispatched to.
    ///
    /// This is the authorisation fact for the whole run: a task result
    /// is accepted only from the session a node was actually sent to.
    /// Written on every dispatch, retries included, because a retry may
    /// go to a different target and the previous one must stop being
    /// able to answer.
    ///
    /// # Errors
    /// Any SQL failure.
    pub async fn set_target_session(
        &self,
        run_id: &RunId,
        session_id: &str,
    ) -> Result<(), AfgError> {
        sqlx::query("UPDATE workflow_runs SET target_session_id = ? WHERE id = ?")
            .bind(session_id)
            .bind(&run_id.0)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// # Errors
    /// Any SQL failure.
    pub async fn complete_run(&self, run_id: &RunId) -> Result<(), AfgError> {
        let now = Utc::now();
        sqlx::query(
            "UPDATE workflow_runs SET status = ?, current_node_id = NULL, updated_at = ?, completed_at = ? WHERE id = ?",
        )
        .bind(RunStatus::Completed.as_str())
        .bind(now.to_rfc3339())
        .bind(now.to_rfc3339())
        .bind(&run_id.0)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// # Errors
    /// Any SQL failure.
    pub async fn fail_run(&self, run_id: &RunId) -> Result<(), AfgError> {
        let now = Utc::now();
        sqlx::query(
            "UPDATE workflow_runs SET status = ?, updated_at = ?, completed_at = ? WHERE id = ?",
        )
        .bind(RunStatus::Failed.as_str())
        .bind(now.to_rfc3339())
        .bind(now.to_rfc3339())
        .bind(&run_id.0)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// # Errors
    /// Any SQL failure.
    pub async fn insert_event(
        &self,
        run_id: &RunId,
        node_id: &str,
        kind: NodeEventKind,
        payload: Option<&Value>,
        message_id: Option<&str>,
    ) -> Result<WorkflowNodeEvent, AfgError> {
        let id = Ulid::new().to_string();
        let now = Utc::now();
        let payload_json = payload.map(std::string::ToString::to_string);
        sqlx::query(
            "INSERT INTO workflow_node_events (id, run_id, node_id, kind, payload_json, message_id, at) \
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(&run_id.0)
        .bind(node_id)
        .bind(kind.as_str())
        .bind(&payload_json)
        .bind(message_id)
        .bind(now.to_rfc3339())
        .execute(&self.pool)
        .await?;

        let event = WorkflowNodeEvent {
            id,
            run_id: run_id.clone(),
            node_id: node_id.to_owned(),
            kind,
            payload: payload.cloned(),
            message_id: message_id.map(str::to_owned),
            at: now,
        };
        // Published only after the row is committed: a watcher must
        // never see an event that a later reader of the same run
        // wouldn't find in its history.
        self.events.publish(&event);
        Ok(event)
    }

    /// Count how many `retry` events this run has already recorded
    /// for `node_id` — the runtime compares this against the node's
    /// `maxRetries`.
    ///
    /// # Errors
    /// Any SQL failure.
    pub async fn retry_count(&self, run_id: &RunId, node_id: &str) -> Result<u32, AfgError> {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM workflow_node_events WHERE run_id = ? AND node_id = ? AND kind = ?",
        )
        .bind(&run_id.0)
        .bind(node_id)
        .bind(NodeEventKind::Retry.as_str())
        .fetch_one(&self.pool)
        .await?;
        Ok(u32::try_from(count).unwrap_or(u32::MAX))
    }

    /// Every node id that has recorded a `complete` event for this
    /// run — used to compute which node to dispatch next.
    ///
    /// # Errors
    /// Any SQL failure.
    pub async fn completed_node_ids(&self, run_id: &RunId) -> Result<Vec<String>, AfgError> {
        let rows: Vec<String> = sqlx::query_scalar(
            "SELECT DISTINCT node_id FROM workflow_node_events WHERE run_id = ? AND kind = ?",
        )
        .bind(&run_id.0)
        .bind(NodeEventKind::Advance.as_str())
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }
}

fn workflow_from_row(row: &sqlx::sqlite::SqliteRow) -> Result<Workflow, AfgError> {
    let spec_json: String = row.get("spec_json");
    let spec: WorkflowSpec = serde_json::from_str(&spec_json)
        .map_err(|e| AfgError::Storage(format!("stored spec unparsable: {e}")))?;
    Ok(Workflow {
        id: WorkflowId(row.get("id")),
        project: row.get("project"),
        name: row.get("name"),
        project_root: row.get("project_root"),
        source_path: row.get("source_path"),
        spec,
        version: row.get("version"),
        created_at: parse_ts(&row.get::<String, _>("created_at"))?,
        updated_at: parse_ts(&row.get::<String, _>("updated_at"))?,
    })
}

fn run_from_row(row: &sqlx::sqlite::SqliteRow) -> Result<WorkflowRun, AfgError> {
    let status_raw: String = row.get("status");
    let status = RunStatus::parse(&status_raw)
        .ok_or_else(|| AfgError::Storage(format!("stored status {status_raw:?} unknown")))?;
    let completed_at: Option<String> = row.get("completed_at");
    Ok(WorkflowRun {
        id: RunId(row.get("id")),
        workflow_id: WorkflowId(row.get("workflow_id")),
        status,
        current_node_id: row.get("current_node_id"),
        correlation_id: row.get("correlation_id"),
        initiator_session_id: row.get("initiator_session_id"),
        target_session_id: row.get("target_session_id"),
        started_at: parse_ts(&row.get::<String, _>("started_at"))?,
        updated_at: parse_ts(&row.get::<String, _>("updated_at"))?,
        completed_at: completed_at.map(|s| parse_ts(&s)).transpose()?,
    })
}

fn event_from_row(row: &sqlx::sqlite::SqliteRow) -> Result<WorkflowNodeEvent, AfgError> {
    let kind_raw: String = row.get("kind");
    let kind = parse_kind(&kind_raw)
        .ok_or_else(|| AfgError::Storage(format!("stored event kind {kind_raw:?} unknown")))?;
    let payload_json: Option<String> = row.get("payload_json");
    Ok(WorkflowNodeEvent {
        id: row.get("id"),
        run_id: RunId(row.get("run_id")),
        node_id: row.get("node_id"),
        kind,
        payload: payload_json
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok()),
        message_id: row.get("message_id"),
        at: parse_ts(&row.get::<String, _>("at"))?,
    })
}

fn parse_kind(s: &str) -> Option<NodeEventKind> {
    match s {
        "enter" => Some(NodeEventKind::Enter),
        "exec" => Some(NodeEventKind::Exec),
        "gate_pass" => Some(NodeEventKind::GatePass),
        "gate_fail" => Some(NodeEventKind::GateFail),
        "retry" => Some(NodeEventKind::Retry),
        "advance" => Some(NodeEventKind::Advance),
        "complete" => Some(NodeEventKind::Complete),
        "error" => Some(NodeEventKind::Error),
        _ => None,
    }
}

fn parse_ts(s: &str) -> Result<DateTime<Utc>, AfgError> {
    DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|e| AfgError::Storage(format!("stored timestamp unparsable: {e}")))
}
