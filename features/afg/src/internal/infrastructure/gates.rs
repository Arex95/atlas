//! Runners for AFG acceptance criteria.
//!
//! One [`run_gate`] entry point dispatches by criterion type and
//! returns a [`GateOutcome`]; the runtime decides whether to
//! advance, retry, or fail the run based on it.
//!
//! Only `shell` and `schema-validate` land in this issue —
//! `project-map-metric` needs Project Map (0% in this rebuild) and
//! `llm-judge` needs a live model call; both are deliberate
//! follow-ups.

use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use jsonschema::Validator;
use regex::Regex;
use serde_json::Value;
use tokio::io::AsyncReadExt;
use tokio::process::Command;

use crate::internal::domain::AcceptanceCriterion;

#[derive(Debug, Clone)]
pub enum GateOutcome {
    Pass {
        /// Short human-readable summary for the event timeline.
        summary: String,
    },
    Fail {
        /// One-line reason shown in the timeline and returned to the
        /// agent as retry feedback on the next dispatch.
        reason: String,
        /// Optional longer diagnostic (stdout, schema error, …)
        /// attached to the event payload for post-mortem.
        detail: Option<String>,
    },
}

pub struct GateInput<'a> {
    /// Absolute path of the project root; the default cwd for shell
    /// gates and the base for `cwd` overrides.
    pub project_root: &'a str,
    pub run_id: &'a str,
    pub node_id: &'a str,
    /// The payload the agent sent back in `task_result` — `shell`
    /// gates receive it as `ATLAS_WORKFLOW_TASK_RESULT`,
    /// `schema-validate` validates it directly.
    pub task_result_payload: Option<&'a Value>,
}

pub async fn run_gate(input: &GateInput<'_>, criterion: &AcceptanceCriterion) -> GateOutcome {
    match criterion {
        AcceptanceCriterion::Shell {
            command,
            cwd,
            expect_exit,
            stdout_matches,
            timeout_secs,
        } => {
            run_shell(
                input.project_root,
                command,
                cwd.as_deref(),
                *expect_exit,
                stdout_matches.as_deref(),
                *timeout_secs,
                input.run_id,
                input.node_id,
                input.task_result_payload,
            )
            .await
        }
        AcceptanceCriterion::SchemaValidate { schema } => {
            run_schema(schema, input.task_result_payload)
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn run_shell(
    project_root: &str,
    command: &str,
    cwd_override: Option<&str>,
    expect_exit: i32,
    stdout_matches: Option<&str>,
    timeout_secs: u64,
    run_id: &str,
    node_id: &str,
    task_result_payload: Option<&Value>,
) -> GateOutcome {
    let cwd: PathBuf = match cwd_override {
        Some(rel) => PathBuf::from(project_root).join(rel),
        None => PathBuf::from(project_root),
    };

    let regex = match stdout_matches {
        Some(pat) => match Regex::new(pat) {
            Ok(r) => Some(r),
            Err(e) => {
                return GateOutcome::Fail {
                    reason: "shell.stdout_matches is not a valid regex".to_owned(),
                    detail: Some(e.to_string()),
                };
            }
        },
        None => None,
    };

    let mut shell_cmd = Command::new("bash");
    shell_cmd.arg("-lc").arg(command);
    shell_cmd.current_dir(&cwd);
    shell_cmd.stdout(Stdio::piped());
    shell_cmd.stderr(Stdio::piped());
    // Give the gate enough context to act on the agent's payload —
    // apply the fix, then run the real test — not just static
    // checks. ATLAS_WORKFLOW_TASK_RESULT is empty with no payload.
    shell_cmd.env("ATLAS_WORKFLOW_RUN_ID", run_id);
    shell_cmd.env("ATLAS_WORKFLOW_NODE_ID", node_id);
    shell_cmd.env(
        "ATLAS_WORKFLOW_TASK_RESULT",
        task_result_payload
            .map(std::string::ToString::to_string)
            .unwrap_or_default(),
    );
    shell_cmd.env("ATLAS_PROJECT_ROOT", project_root);

    let mut child = match shell_cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            return GateOutcome::Fail {
                reason: format!("shell: failed to spawn: {e}"),
                detail: None,
            };
        }
    };

    let mut stdout = child.stdout.take();
    let mut stderr = child.stderr.take();
    let wait = tokio::time::timeout(Duration::from_secs(timeout_secs), child.wait()).await;

    let status = match wait {
        Ok(Ok(s)) => s,
        Ok(Err(e)) => {
            return GateOutcome::Fail {
                reason: format!("shell: wait failed: {e}"),
                detail: None,
            };
        }
        Err(_) => {
            let _ = child.kill().await;
            return GateOutcome::Fail {
                reason: format!("shell: timeout after {timeout_secs}s"),
                detail: None,
            };
        }
    };

    let out = read_all(&mut stdout).await;
    let err = read_all(&mut stderr).await;
    let combined = format!("{out}{err}");

    let exit_code = status.code().unwrap_or(-1);
    if exit_code != expect_exit {
        return GateOutcome::Fail {
            reason: format!("shell: exit {exit_code} (expected {expect_exit})"),
            detail: Some(truncate(&combined, 4096)),
        };
    }

    if let Some(r) = regex
        && !r.is_match(&combined)
    {
        return GateOutcome::Fail {
            reason: format!("shell: stdout did not match regex '{}'", r.as_str()),
            detail: Some(truncate(&combined, 4096)),
        };
    }

    GateOutcome::Pass {
        summary: format!("shell '{}' exit {exit_code}", short(command, 60)),
    }
}

async fn read_all(handle: &mut Option<impl tokio::io::AsyncRead + Unpin>) -> String {
    if let Some(h) = handle {
        let mut buf = Vec::with_capacity(1024);
        let _ = h.read_to_end(&mut buf).await;
        String::from_utf8_lossy(&buf).into_owned()
    } else {
        String::new()
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_owned()
    } else {
        let mut end = max;
        while end > 0 && !s.is_char_boundary(end) {
            end -= 1;
        }
        format!("{}\n… (+{} bytes truncated)", &s[..end], s.len() - end)
    }
}

fn short(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_owned()
    } else {
        format!("{}…", &s[..max])
    }
}

fn run_schema(schema: &Value, payload: Option<&Value>) -> GateOutcome {
    let Some(payload) = payload else {
        return GateOutcome::Fail {
            reason: "schema-validate: task_result had no payload to validate".to_owned(),
            detail: None,
        };
    };

    let validator = match Validator::new(schema) {
        Ok(v) => v,
        Err(e) => {
            return GateOutcome::Fail {
                reason: format!("schema-validate: invalid schema: {e}"),
                detail: None,
            };
        }
    };

    if let Err(err) = validator.validate(payload) {
        let detail = err.to_string();
        return GateOutcome::Fail {
            reason: format!("schema-validate: {}", short(&detail, 200)),
            detail: Some(detail),
        };
    }
    GateOutcome::Pass {
        summary: "schema-validate ok".to_owned(),
    }
}
