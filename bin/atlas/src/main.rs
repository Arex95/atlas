//! `atlas` — driving an Atlas server from a terminal.
//!
//! Two kinds of subcommand, and the split is deliberate.
//!
//! `call` reaches **every** tool the server exposes, including ones
//! added after this binary was built. It is the reason the CLI can
//! claim to exercise all of Atlas rather than the part somebody
//! remembered to wrap.
//!
//! The rest are shortcuts for what gets typed often. They exist for
//! ergonomics and add no capability — each is `call` with the
//! arguments filled in.
//!
//! **Identity comes from the environment, never from a flag.** Inside
//! a terminal Atlas spawned, `ATLAS_SESSION_TOKEN` is already set and
//! every call is attributable to that session and its owner. A flag
//! would invite passing somebody else's, which is the shape this
//! project spent a while removing.

mod client;

use std::fmt::Write as _;

use clap::{Parser, Subcommand};
use client::{Client, ClientError, Credential};
use serde_json::{Value, json};

#[derive(Parser)]
#[command(
    name = "atlas",
    about = "Drive an Atlas server from a terminal.",
    long_about = "Drive an Atlas server from a terminal.\n\nAuthenticates with \
                  ATLAS_SESSION_TOKEN when set — which it is inside any terminal \
                  Atlas spawned — and falls back to ATLAS_MCP_TOKEN.",
    version
)]
struct Cli {
    /// Where the server is.
    #[arg(long, env = "ATLAS_URL", default_value = "http://127.0.0.1:4000")]
    url: String,

    /// Print raw JSON instead of a table. Always on when stdout is not
    /// a terminal, so a pipeline gets JSON without asking.
    #[arg(long, global = true)]
    json: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Call any tool by name. `atlas tools` lists them.
    ///
    /// Arguments are `key=value`; a value that parses as JSON is sent
    /// as JSON, anything else as a string. Pass `--args` for a whole
    /// JSON object when a value is too awkward for that.
    Call {
        tool: String,
        /// `key=value`, repeatable.
        #[arg(value_name = "KEY=VALUE")]
        args: Vec<String>,
        /// A complete JSON object, instead of `key=value` pairs.
        #[arg(long, value_name = "JSON", conflicts_with = "args")]
        json_args: Option<String>,
    },
    /// Every tool the server exposes.
    Tools,
    /// Sessions you own.
    Ls {
        /// Only this project.
        #[arg(long)]
        project: Option<String>,
    },
    /// Send a message to another session, or to everyone on a project.
    Msg {
        /// Session id, or omit for a broadcast.
        #[arg(long)]
        to: Option<String>,
        #[arg(long)]
        project: String,
        /// Who it is from. Defaults to this session.
        #[arg(long)]
        from: Option<String>,
        text: String,
    },
    /// Messages addressed to a session, oldest first.
    Inbox {
        #[arg(long)]
        project: String,
        /// Whose inbox. Defaults to this session's.
        #[arg(long)]
        session: Option<String>,
    },
    /// Issues, from the tracker.
    Issues {
        #[arg(long)]
        project: String,
        /// `open` or `closed`. Both when omitted.
        #[arg(long)]
        status: Option<String>,
        /// Repeatable. An issue must carry every one given.
        #[arg(long)]
        label: Vec<String>,
        /// Restrict to one milestone, by title.
        #[arg(long)]
        milestone: Option<String>,
        /// Group by status into columns instead of listing rows.
        #[arg(long)]
        board: bool,
    },
    /// One issue, with its acceptance criteria.
    Issue {
        #[arg(long)]
        project: String,
        id: String,
    },
    /// Acceptance criteria ticked over total.
    Progress {
        #[arg(long)]
        project: String,
        /// Restrict to one milestone — the completion of a roadmap
        /// rather than of everything open.
        #[arg(long)]
        milestone: Option<String>,
        #[arg(long)]
        label: Vec<String>,
    },
    /// Workflow runs, newest first.
    Runs {
        #[arg(long)]
        workflow: String,
        /// `pending`, `running`, `failed` or `completed`.
        #[arg(long)]
        status: Option<String>,
    },
    /// One run: where it is, and every node event so far.
    Run { id: String },
    /// What the project map found — cycles, layer violations, hubs.
    Findings {
        #[arg(long)]
        project: String,
        /// Only `error`, `warning` or `info`.
        #[arg(long)]
        severity: Option<String>,
    },
    /// What the server knows about itself and this credential.
    Whoami,
}

#[tokio::main]
async fn main() -> std::process::ExitCode {
    let cli = Cli::parse();
    match run(&cli).await {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("atlas: {e}");
            std::process::ExitCode::FAILURE
        }
    }
}

async fn run(cli: &Cli) -> Result<(), ClientError> {
    let client = Client::from_env(cli.url.clone())?;
    // Piped output is for another program, and a table is not.
    let as_json = cli.json || !std::io::IsTerminal::is_terminal(&std::io::stdout());

    match &cli.command {
        Command::Tools => {
            let value = client.tools().await?;
            if as_json {
                print_json(&value);
            } else {
                print_tools(&value);
            }
        }
        Command::Call {
            tool,
            args,
            json_args,
        } => {
            let arguments = match json_args {
                Some(raw) => serde_json::from_str(raw).map_err(|e| ClientError::Rpc {
                    code: -32602,
                    message: format!("--json-args is not valid JSON: {e}"),
                })?,
                None => pairs_to_object(args)?,
            };
            print_json(&client.call(tool, arguments).await?);
        }
        Command::Ls { project } => {
            let project = project.clone().unwrap_or_default();
            let value = client
                .call("sessions.list", json!({ "project": project }))
                .await?;
            if as_json {
                print_json(&value);
            } else {
                print_sessions(&value);
            }
        }
        Command::Msg {
            to,
            project,
            from,
            text,
        } => {
            let mut arguments = json!({ "project": project, "payload": { "text": text } });
            let object = arguments.as_object_mut().expect("built as an object");
            if let Some(to) = to {
                object.insert("to".to_owned(), json!(to));
            }
            if let Some(from) = from {
                object.insert("from".to_owned(), json!(from));
            }
            print_json(&client.call("messaging.send_message", arguments).await?);
        }
        Command::Inbox { project, session } => {
            let arguments = json!({
                "project": project,
                "for": session.clone().unwrap_or_default(),
            });
            print_json(&client.call("messaging.read_inbox", arguments).await?);
        }
        Command::Whoami => {
            let credential = match client.credential {
                Credential::Session => {
                    "a session token — calls are attributable to that session and its owner"
                }
                Credential::Shared => {
                    "the shared MCP token — it identifies a permitted client, not a person"
                }
            };
            println!("server:     {}", cli.url);
            println!("credential: {credential}");
            // Proves the credential actually works, rather than
            // reporting what an environment variable claims.
            let tools = client.tools().await?;
            let count = tools
                .get("tools")
                .and_then(Value::as_array)
                .map_or(0, Vec::len);
            println!("reachable:  yes, {count} tools");
        }
        // Everything that fetches and renders a report lives in
        // its own function — `run` was becoming a wall.
        report => return run_report(&client, as_json, report).await,
    }
    Ok(())
}

/// The subcommands that fetch something and render it.
///
/// Split out of `run` because they all have the same shape — build
/// arguments, call one tool, print JSON or a table — and reading that
/// shape six times in the middle of the dispatch obscured the
/// commands that do not follow it.
async fn run_report(client: &Client, as_json: bool, command: &Command) -> Result<(), ClientError> {
    match command {
        Command::Issues {
            project,
            status,
            label,
            milestone,
            board,
        } => {
            let mut arguments = json!({ "project": project });
            let object = arguments.as_object_mut().expect("built as an object");
            if let Some(status) = status {
                object.insert("status".to_owned(), json!(status));
            }
            if !label.is_empty() {
                object.insert("labels".to_owned(), json!(label));
            }
            if let Some(milestone) = milestone {
                object.insert("milestone".to_owned(), json!(milestone));
            }
            let value = client.call("tracker.list_issues", arguments).await?;
            if as_json {
                print_json(&value);
            } else if *board {
                print!("{}", render_board(&value));
            } else {
                print!("{}", render_issues(&value));
            }
        }
        Command::Issue { project, id } => {
            let value = client
                .call("tracker.get_issue", json!({ "project": project, "id": id }))
                .await?;
            if as_json {
                print_json(&value);
            } else {
                print!("{}", render_issue(&value));
            }
        }
        Command::Progress {
            project,
            milestone,
            label,
        } => {
            let mut arguments = json!({ "project": project });
            let object = arguments.as_object_mut().expect("built as an object");
            if let Some(milestone) = milestone {
                object.insert("milestone".to_owned(), json!(milestone));
            }
            if !label.is_empty() {
                object.insert("labels".to_owned(), json!(label));
            }
            let value = client.call("tracker.plan_progress", arguments).await?;
            if as_json {
                print_json(&value);
            } else {
                print!("{}", render_progress(&value));
            }
        }
        Command::Runs { workflow, status } => {
            let mut arguments = json!({ "workflow_id": workflow });
            if let Some(status) = status {
                arguments
                    .as_object_mut()
                    .expect("built as an object")
                    .insert("status".to_owned(), json!(status));
            }
            let value = client.call("afg.list_runs", arguments).await?;
            if as_json {
                print_json(&value);
            } else {
                print!("{}", render_runs(&value));
            }
        }
        Command::Run { id } => {
            let value = client.call("afg.get_run", json!({ "run_id": id })).await?;
            if as_json {
                print_json(&value);
            } else {
                print!("{}", render_run(&value));
            }
        }
        Command::Findings { project, severity } => {
            // The tool takes only a project — the severity filter is
            // applied here rather than sent, because inventing a
            // parameter the server does not have would fail at the
            // wire with a worse message than this flag deserves.
            let value = client
                .call("graph.findings", json!({ "project": project }))
                .await?;
            if as_json {
                print_json(&value);
            } else {
                print!("{}", render_findings(&value, severity.as_deref()));
            }
        }
        // `run` routes only the report commands here.
        _ => unreachable!("not a report command"),
    }
    Ok(())
}

/// `key=value` pairs into a JSON object.
///
/// A value that parses as JSON is sent as JSON, so `limit=10` is a
/// number and `payload={"a":1}` is an object. Anything else is a
/// string, which is what makes `title=my session` work without
/// quoting gymnastics.
fn pairs_to_object(pairs: &[String]) -> Result<Value, ClientError> {
    let mut object = serde_json::Map::new();
    for pair in pairs {
        let (key, raw) = pair.split_once('=').ok_or_else(|| ClientError::Rpc {
            code: -32602,
            message: format!("{pair:?} is not key=value"),
        })?;
        let value = serde_json::from_str(raw).unwrap_or_else(|_| Value::String(raw.to_owned()));
        object.insert(key.to_owned(), value);
    }
    Ok(Value::Object(object))
}

fn print_json(value: &Value) {
    match serde_json::to_string_pretty(value) {
        Ok(text) => println!("{text}"),
        Err(_) => println!("{value}"),
    }
}

fn print_tools(value: &Value) {
    let Some(tools) = value.get("tools").and_then(Value::as_array) else {
        print_json(value);
        return;
    };
    for tool in tools {
        let name = tool.get("name").and_then(Value::as_str).unwrap_or("?");
        let description = tool
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or("")
            // Descriptions are written for a model and run to
            // paragraphs; a list needs the first sentence.
            .split(['.', '\n'])
            .next()
            .unwrap_or("");
        println!("{name:<28} {description}");
    }
    println!("\n{} tools. `atlas call <name>` runs one.", tools.len());
}

fn print_sessions(value: &Value) {
    let Some(sessions) = value.as_array() else {
        print_json(value);
        return;
    };
    if sessions.is_empty() {
        println!("no sessions");
        return;
    }
    for session in sessions {
        let field = |k: &str| session.get(k).and_then(Value::as_str).unwrap_or("?");
        println!(
            "{:<28} {:<10} {:<8} {}",
            field("id"),
            field("agent_kind"),
            field("status"),
            session
                .get("title")
                .and_then(Value::as_str)
                .unwrap_or(field("relative_path")),
        );
    }
}

/// One issue per line: id, status, milestone, title.
///
/// Every renderer below returns its text rather than printing it, so
/// what a command shows can be asserted on. A `println!` is only
/// observable by a person looking at it.
fn render_issues(value: &Value) -> String {
    let Some(issues) = value.as_array() else {
        return pretty(value);
    };
    if issues.is_empty() {
        return "no issues\n".to_owned();
    }
    let mut out = String::new();
    for issue in issues {
        let _ = writeln!(
            out,
            "#{:<6} {:<7} {:<14} {}",
            str_field(issue, "id"),
            str_field(issue, "status"),
            issue
                .get("milestone")
                .and_then(Value::as_str)
                .unwrap_or("—"),
            str_field(issue, "title"),
        );
    }
    let _ = writeln!(out, "\n{} issues", issues.len());
    out
}

/// The same issues, grouped into a column per status.
///
/// A board in a terminal is columns, not cards you drag: the status
/// changes because a command or an agent changed it, and dragging was
/// never the part that carried the meaning.
fn render_board(value: &Value) -> String {
    let Some(issues) = value.as_array() else {
        return pretty(value);
    };
    let mut out = String::new();
    for status in ["open", "closed"] {
        let column: Vec<&Value> = issues
            .iter()
            .filter(|i| str_field(i, "status") == status)
            .collect();
        let _ = writeln!(out, "\n{}  ({})", status.to_uppercase(), column.len());
        let _ = writeln!(out, "{}", "─".repeat(40));
        if column.is_empty() {
            out.push_str("  —\n");
        }
        for issue in column {
            let _ = writeln!(
                out,
                "  #{:<6} {}",
                str_field(issue, "id"),
                str_field(issue, "title")
            );
        }
    }
    out
}

fn render_issue(value: &Value) -> String {
    let mut out = format!(
        "#{} {}\n",
        str_field(value, "id"),
        str_field(value, "title")
    );
    let _ = writeln!(out, "status:    {}", str_field(value, "status"));
    let _ = writeln!(
        out,
        "milestone: {}",
        value
            .get("milestone")
            .and_then(Value::as_str)
            .unwrap_or("—")
    );
    if let Some(labels) = value.get("labels").and_then(Value::as_array)
        && !labels.is_empty()
    {
        let names: Vec<&str> = labels.iter().filter_map(Value::as_str).collect();
        let _ = writeln!(out, "labels:    {}", names.join(", "));
    }
    if let Some(body) = value.get("description").and_then(Value::as_str)
        && !body.is_empty()
    {
        let _ = writeln!(out, "\n{body}");
    }
    out
}

/// Ticked over total, with a bar and the issues behind it.
fn render_progress(value: &Value) -> String {
    let done = value.get("done").and_then(Value::as_u64).unwrap_or(0);
    let total = value.get("total").and_then(Value::as_u64).unwrap_or(0);
    if total == 0 {
        return "no acceptance criteria found\n".to_owned();
    }
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_precision_loss,
        clippy::cast_sign_loss,
        reason = "a 30-cell bar; done <= total, so the result is within 0..=30"
    )]
    let filled = ((done as f64 / total as f64) * 30.0).round() as usize;
    let filled = filled.min(30);
    let mut out = format!(
        "[{}{}]  {done}/{total} criteria  ({}%)\n",
        "█".repeat(filled),
        "░".repeat(30 - filled),
        done * 100 / total,
    );

    let Some(by_issue) = value.get("by_issue").and_then(Value::as_array) else {
        return out;
    };
    out.push('\n');
    for issue in by_issue {
        let d = issue.get("done").and_then(Value::as_u64).unwrap_or(0);
        let t = issue.get("total").and_then(Value::as_u64).unwrap_or(0);
        // An issue with no criteria contributed nothing to the number
        // above, so listing it here would only pad the output.
        if t == 0 {
            continue;
        }
        let _ = writeln!(
            out,
            "  {}  #{:<6} {:<48} {d}/{t}",
            if d == t { "✓" } else { " " },
            str_field(issue, "id"),
            str_field(issue, "title"),
        );
    }
    out
}

fn render_runs(value: &Value) -> String {
    let Some(runs) = value.as_array() else {
        return pretty(value);
    };
    if runs.is_empty() {
        return "no runs\n".to_owned();
    }
    let mut out = String::new();
    for run in runs {
        let _ = writeln!(
            out,
            "{:<28} {:<10} at {}",
            str_field(run, "id"),
            str_field(run, "status"),
            run.get("current_node_id")
                .and_then(Value::as_str)
                .unwrap_or("—"),
        );
    }
    out
}

/// A run's state, then its node events in order — the auditable trail
/// of what the runtime decided, and why.
fn render_run(value: &Value) -> String {
    let Some(run) = value.get("run") else {
        return pretty(value);
    };
    let mut out = format!("{}\n", str_field(run, "id"));
    let _ = writeln!(out, "status: {}", str_field(run, "status"));
    let _ = writeln!(
        out,
        "node:   {}",
        run.get("current_node_id")
            .and_then(Value::as_str)
            .unwrap_or("—")
    );

    let Some(events) = value.get("events").and_then(Value::as_array) else {
        return out;
    };
    let _ = writeln!(out, "\n{} events", events.len());
    for event in events {
        let _ = writeln!(
            out,
            "  {:<12} {:<12} {}",
            str_field(event, "kind"),
            str_field(event, "node_id"),
            event.get("at").and_then(Value::as_str).unwrap_or(""),
        );
        if let Some(detail) = event.get("detail").and_then(Value::as_str)
            && !detail.is_empty()
        {
            let _ = writeln!(out, "               {detail}");
        }
    }
    out
}

/// Findings, most severe first, with the evidence they rest on.
///
/// The evidence line prints even when there is nothing wrong: "no
/// findings" from a graph that resolved a tenth of its imports is not
/// the same statement as "no findings" from a dense one, and a reader
/// who cannot see which one they got will read the stronger.
fn render_findings(value: &Value, severity: Option<&str>) -> String {
    let Some(findings) = value.get("findings").and_then(Value::as_array) else {
        return pretty(value);
    };
    let shown: Vec<&Value> = findings
        .iter()
        .filter(|f| severity.is_none_or(|s| str_field(f, "severity").eq_ignore_ascii_case(s)))
        .collect();

    let mut out = String::new();
    if shown.is_empty() {
        out.push_str("no findings\n");
    }
    for finding in shown {
        let _ = writeln!(
            out,
            "{:<8} {:<24} {}",
            str_field(finding, "severity").to_uppercase(),
            str_field(finding, "kind"),
            str_field(finding, "detail"),
        );
    }

    if let Some(evidence) = value.get("evidence") {
        let n = |k: &str| evidence.get(k).and_then(Value::as_u64).unwrap_or(0);
        let _ = writeln!(
            out,
            "\nfrom {} files, {} imports resolved, {} not",
            n("files"),
            n("resolved_imports"),
            n("unresolved_imports"),
        );
    }
    out
}

/// Fallback when a payload is not the shape a renderer expects — the
/// raw JSON, rather than a confident table of nothing.
fn pretty(value: &Value) -> String {
    serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string())
}

fn str_field<'a>(value: &'a Value, key: &str) -> &'a str {
    value.get(key).and_then(Value::as_str).unwrap_or("?")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_board_puts_each_issue_under_its_status() {
        let issues = json!([
            { "id": "1", "title": "first",  "status": "open"   },
            { "id": "2", "title": "second", "status": "closed" },
            { "id": "3", "title": "third",  "status": "open"   }
        ]);
        let out = render_board(&issues);
        let open = out.find("OPEN").unwrap();
        let closed = out.find("CLOSED").unwrap();

        assert!(out.contains("OPEN  (2)"));
        assert!(out.contains("CLOSED  (1)"));
        // Each issue under its own heading, not merely present.
        assert!((open..closed).contains(&out.find("first").unwrap()));
        assert!((open..closed).contains(&out.find("third").unwrap()));
        assert!(out.find("second").unwrap() > closed);
    }

    #[test]
    fn an_empty_column_says_so_rather_than_vanishing() {
        // A board missing a column reads as "there is no such status".
        let out = render_board(&json!([{ "id": "1", "title": "x", "status": "open" }]));
        assert!(out.contains("CLOSED  (0)"));
    }

    #[test]
    fn an_issue_with_no_milestone_shows_a_dash_not_a_question_mark() {
        // `?` is what an unreadable field renders as; no milestone is
        // a known answer and must not look like a failure.
        let out = render_issues(&json!([{ "id": "1", "title": "x", "status": "open" }]));
        assert!(out.contains('—'), "{out}");
        assert!(!out.contains('?'), "{out}");
    }

    #[test]
    fn progress_reports_criteria_not_issues() {
        // The whole point of plan_progress: an issue with sixteen
        // criteria and one ticked is 1/16, not "0 of 1 done".
        let out = render_progress(&json!({
            "done": 1, "total": 16,
            "by_issue": [{ "id": "7", "title": "big one", "done": 1, "total": 16 }]
        }));
        assert!(out.contains("1/16 criteria"));
        assert!(out.contains("(6%)"));
    }

    #[test]
    fn a_full_bar_does_not_overflow_its_cells() {
        let out = render_progress(&json!({ "done": 9, "total": 9, "by_issue": [] }));
        assert!(out.contains("(100%)"));
        assert!(
            !out.contains('░'),
            "a complete plan drew empty cells: {out}"
        );
    }

    #[test]
    fn an_issue_with_no_criteria_is_left_out_of_the_breakdown() {
        // It contributed nothing to the total, so listing it would pad
        // the output with rows that cannot move.
        let out = render_progress(&json!({
            "done": 1, "total": 2,
            "by_issue": [
                { "id": "1", "title": "counted", "done": 1, "total": 2 },
                { "id": "2", "title": "no criteria", "done": 0, "total": 0 }
            ]
        }));
        assert!(out.contains("counted"));
        assert!(!out.contains("no criteria"), "{out}");
    }

    #[test]
    fn nothing_to_show_says_so_instead_of_printing_an_empty_table() {
        assert_eq!(render_issues(&json!([])), "no issues\n");
        assert_eq!(render_runs(&json!([])), "no runs\n");
    }

    #[test]
    fn findings_can_be_narrowed_to_one_severity() {
        let payload = json!({
            "findings": [
                { "kind": "layer_violation", "severity": "error",   "detail": "a" },
                { "kind": "large_file",      "severity": "info",    "detail": "b" }
            ],
            "evidence": { "files": 10, "resolved_imports": 5, "unresolved_imports": 1 }
        });
        let errors = render_findings(&payload, Some("error"));
        assert!(errors.contains("layer_violation"));
        assert!(!errors.contains("large_file"));
        assert!(render_findings(&payload, None).contains("large_file"));
    }

    #[test]
    fn findings_always_report_what_they_were_drawn_from() {
        // "no findings" from a graph that resolved almost nothing is a
        // weaker statement than the same words from a dense one, and
        // the reader cannot tell them apart without this line.
        let empty = json!({
            "findings": [],
            "evidence": { "files": 260, "resolved_imports": 254, "unresolved_imports": 332 }
        });
        let out = render_findings(&empty, None);
        assert!(out.contains("no findings"));
        assert!(
            out.contains("from 260 files, 254 imports resolved, 332 not"),
            "{out}"
        );
    }

    #[test]
    fn an_unexpected_payload_falls_back_to_raw_json() {
        // Better an odd-looking blob than a confident table of "?".
        let out = render_issues(&json!({ "error": "not a list" }));
        assert!(out.contains("not a list"));
    }

    #[test]
    fn a_run_shows_its_event_trail_in_order() {
        let out = render_run(&json!({
            "run": { "id": "01RUN", "status": "completed", "current_node_id": null },
            "events": [
                { "kind": "enter",     "node_id": "build",  "at": "2026-09-12T00:00:00Z" },
                { "kind": "gate_pass", "node_id": "verify", "at": "2026-09-12T00:01:00Z" }
            ]
        }));
        assert!(out.contains("2 events"));
        assert!(out.find("enter").unwrap() < out.find("gate_pass").unwrap());
        // A finished run has no current node; "?" would read as a bug.
        assert!(out.contains("node:   —"), "{out}");
    }

    #[test]
    fn a_numeric_value_is_sent_as_a_number() {
        let object = pairs_to_object(&["limit=10".to_owned()]).unwrap();
        assert_eq!(object, json!({ "limit": 10 }));
    }

    #[test]
    fn a_plain_word_is_sent_as_a_string() {
        // Not valid JSON on its own, and quoting it at the shell to
        // make it so would be a trap nobody remembers.
        let object = pairs_to_object(&["project=your-org/your-project".to_owned()]).unwrap();
        assert_eq!(object, json!({ "project": "your-org/your-project" }));
    }

    #[test]
    fn an_embedded_json_object_survives_as_an_object() {
        let object = pairs_to_object(&[r#"payload={"a":1}"#.to_owned()]).unwrap();
        assert_eq!(object, json!({ "payload": { "a": 1 } }));
    }

    #[test]
    fn a_value_containing_an_equals_sign_keeps_it() {
        // Split on the *first* `=` only; a base64 value or a query
        // string would otherwise lose its tail.
        let object = pairs_to_object(&["q=a=b=c".to_owned()]).unwrap();
        assert_eq!(object, json!({ "q": "a=b=c" }));
    }

    #[test]
    fn a_pair_with_no_equals_is_refused_by_name() {
        let err = pairs_to_object(&["nonsense".to_owned()]).unwrap_err();
        assert!(format!("{err}").contains("nonsense"), "{err}");
    }

    #[test]
    fn true_and_null_are_sent_as_themselves() {
        let object = pairs_to_object(&["a=true".to_owned(), "b=null".to_owned()]).unwrap();
        assert_eq!(object, json!({ "a": true, "b": null }));
    }
}
