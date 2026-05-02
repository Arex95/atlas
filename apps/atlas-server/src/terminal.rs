use crate::constants::{defaults, env, errors, terminal as term_consts};
use portable_pty::{Child, CommandBuilder, MasterPty, PtySize, native_pty_system};
use regex::Regex;
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::{Arc, LazyLock, Mutex as StdMutex, RwLock};
use std::time::{Duration, Instant};
use tokio::runtime::Handle;
use tokio::sync::Mutex;
use tracing::{error, info};

type SpawnResult = Result<
    (
        Box<dyn Write + Send>,
        Box<dyn Child + Send + Sync>,
        Box<dyn MasterPty + Send>,
    ),
    String,
>;

static ANSI_ESCAPE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"[\x1b\x9b]\[[0-9;?]*[a-zA-Z]")
        .expect("ANSI_ESCAPE_RE: static regex pattern invalid")
});

const SCROLLBACK_MAX_BYTES: usize = 100_000;
const SCROLLBACK_WINDOW_BYTES: usize = 2_000;

pub struct PtySession {
    pub writer: Box<dyn Write + Send>,
    pub _child: Box<dyn Child + Send + Sync>,
    pub master: Box<dyn MasterPty + Send>,
    pub output_tx: tokio::sync::broadcast::Sender<String>,
    pub scrollback: Arc<RwLock<String>>,
    pub command_buffer: String,
}

pub struct TerminalManager {
    sessions: Mutex<HashMap<String, PtySession>>,
    pool: SqlitePool,
    pub events_tx: tokio::sync::broadcast::Sender<TerminalEvent>,
}

#[derive(Clone, serde::Serialize)]
pub enum TerminalEvent {
    PathUpdated {
        session_id: String,
        new_path: String,
    },
}

impl TerminalManager {
    pub fn new(pool: SqlitePool) -> Self {
        let (events_tx, _) = tokio::sync::broadcast::channel(100);
        Self {
            sessions: Mutex::new(HashMap::new()),
            pool,
            events_tx,
        }
    }

    pub async fn get_live_sessions(&self) -> Vec<String> {
        let sessions = self.sessions.lock().await;
        sessions.keys().cloned().collect()
    }

    pub async fn get_or_create_session(
        &self,
        session_id: String,
        project_id: String,
        working_directory: String,
    ) -> Result<(tokio::sync::broadcast::Receiver<String>, String), String> {
        {
            let sessions = self.sessions.lock().await;
            if let Some(session) = sessions.get(&session_id) {
                info!(
                    "[PTY] Session {} already running, returning subscriber and scrollback",
                    session_id
                );
                let scrollback_arc = session.scrollback.clone();
                let output_tx = session.output_tx.clone();
                drop(sessions);
                let scrollback = scrollback_arc.read().unwrap().clone();
                return Ok((output_tx.subscribe(), scrollback));
            }
        }

        info!(
            "[PTY] Spawning new session: {} in {}",
            session_id, working_directory
        );

        // Load persisted scrollback from DB before spawning the PTY so the
        // client gets historical output even after a server restart.
        let initial_scrollback = sqlx::query_scalar::<_, String>(
            "SELECT content FROM session_scrollback WHERE session_id = ?",
        )
        .bind(&session_id)
        .fetch_optional(&self.pool)
        .await
        .unwrap_or(None)
        .unwrap_or_default();

        let (tx, rx) = tokio::sync::broadcast::channel::<String>(1024);
        let tx_callback = tx.clone();

        let scrollback = Arc::new(RwLock::new(initial_scrollback.clone()));
        let scrollback_clone = scrollback.clone();

        // Debounce state: only flush to DB at most once every 3 seconds.
        let last_flush: Arc<StdMutex<Instant>> = Arc::new(StdMutex::new(Instant::now()));
        let last_flush_clone = last_flush.clone();

        let pool_flush = self.pool.clone();
        let session_id_spawn = session_id.clone();
        let project_id_spawn = project_id.clone();

        let result = tokio::task::spawn_blocking(move || -> SpawnResult {
            let pty_system = native_pty_system();

            let pair = pty_system
                .openpty(PtySize {
                    rows: 24,
                    cols: 80,
                    pixel_width: 0,
                    pixel_height: 0,
                })
                .map_err(|e| format!("Failed to open PTY: {e}"))?;

            let mut cmd = CommandBuilder::new("bash");
            cmd.args(["-i"]);
            cmd.env(term_consts::TERM_VAR, term_consts::TERM_TYPE);

            cmd.env(env::ATLAS_PROJECT_ID, &project_id_spawn);
            cmd.env(env::ATLAS_SESSION_ID, &session_id_spawn);
            let server_url = std::env::var(env::ATLAS_SERVER_URL)
                .unwrap_or_else(|_| defaults::SERVER_URL.to_string());
            cmd.env(env::ATLAS_SERVER_URL, &server_url);
            if let Ok(token) = std::env::var(env::ATLAS_MCP_TOKEN)
                && !token.is_empty()
            {
                cmd.env(env::ATLAS_MCP_TOKEN, &token);
            }

            let bin_path = std::env::var(env::ATLAS_BIN_DIR).unwrap_or_else(|_| {
                std::env::current_exe()
                    .ok()
                    .and_then(|p| p.parent().map(|p| p.to_path_buf()))
                    .map(|p| p.join("../bin").to_string_lossy().to_string())
                    .unwrap_or_else(|| defaults::BIN_DIR.to_string())
            });
            if let Ok(current_path) = std::env::var("PATH") {
                cmd.env("PATH", format!("{}:{}", bin_path, current_path));
            }

            if !working_directory.is_empty() && std::path::Path::new(&working_directory).exists() {
                cmd.cwd(&working_directory);
            }

            let child = pair.slave.spawn_command(cmd).map_err(|e| e.to_string())?;
            let writer = pair.master.take_writer().map_err(|e| e.to_string())?;
            let mut reader = pair.master.try_clone_reader().map_err(|e| e.to_string())?;
            let master = pair.master;

            let tokio_handle = Handle::current();
            std::thread::spawn(move || {
                let flush_interval = Duration::from_secs(3);
                let mut buf = [0u8; 8192];
                loop {
                    match reader.read(&mut buf) {
                        Ok(0) => break,
                        Ok(n) => {
                            let output = String::from_utf8_lossy(&buf[..n]).to_string();
                            let _ = tx_callback.send(output.clone());

                            let content_snapshot = {
                                let mut sb = scrollback_clone.write().unwrap();
                                sb.push_str(&output);

                                if sb.len() > SCROLLBACK_MAX_BYTES {
                                    let mut start = sb.len() - SCROLLBACK_MAX_BYTES;
                                    while start < sb.len() && !sb.is_char_boundary(start) {
                                        start += 1;
                                    }
                                    *sb = sb[start..].to_string();
                                }

                                // Check debounce inside the write-lock so we
                                // capture a consistent snapshot in one step.
                                let mut last = last_flush_clone.lock().unwrap();
                                if last.elapsed() >= flush_interval {
                                    *last = Instant::now();
                                    Some(sb.clone())
                                } else {
                                    None
                                }
                            };

                            if let Some(content) = content_snapshot {
                                let pool = pool_flush.clone();
                                let sid = session_id_spawn.clone();
                                tokio_handle.spawn(async move {
                                    let _ = sqlx::query(
                                        "INSERT INTO session_scrollback (session_id, content, updated_at) \
                                         VALUES (?, ?, STRFTIME('%Y-%m-%dT%H:%M:%fZ', 'now')) \
                                         ON CONFLICT(session_id) DO UPDATE \
                                         SET content = excluded.content, \
                                             updated_at = excluded.updated_at",
                                    )
                                    .bind(&sid)
                                    .bind(&content)
                                    .execute(&pool)
                                    .await;
                                });
                            }
                        }
                        Err(_) => break,
                    }
                }
            });

            Ok((writer, child, master))
        })
        .await
        .map_err(|e| e.to_string())?;

        match result {
            Ok((writer, child, master)) => {
                let mut sessions = self.sessions.lock().await;
                sessions.insert(
                    session_id.clone(),
                    PtySession {
                        writer,
                        _child: child,
                        master,
                        output_tx: tx.clone(),
                        scrollback,
                        command_buffer: String::new(),
                    },
                );

                let _ = tx.send(term_consts::WELCOME.to_string());
                let _ = tx.send(term_consts::WELCOME_HINT.to_string());

                info!(
                    "[PTY] New session {} registered with scrollback (persisted: {} bytes)",
                    session_id,
                    initial_scrollback.len()
                );
                Ok((rx, initial_scrollback))
            }
            Err(e) => {
                error!("[PTY] Failed to spawn session {}: {}", session_id, e);
                Err(e)
            }
        }
    }

    pub async fn write_input(
        &self,
        session_id: &str,
        data: &str,
    ) -> Result<(Option<String>, Option<String>), String> {
        let has_newline = data.as_bytes().iter().any(|&b| b == 13 || b == 10);

        if has_newline {
            let scrollback = {
                let sessions = self.sessions.lock().await;
                sessions
                    .get(session_id)
                    .ok_or(errors::SESSION_NOT_FOUND)?
                    .scrollback
                    .clone()
            };

            let cmd_trimmed = {
                let sb = scrollback.read().unwrap();
                let window_size = SCROLLBACK_WINDOW_BYTES.min(sb.len());
                let mut start = sb.len() - window_size;
                while start < sb.len() && !sb.is_char_boundary(start) {
                    start += 1;
                }
                let sb_window = &sb[start..];
                let sb_no_ansi = ANSI_ESCAPE_RE.replace_all(sb_window, "").to_string();

                let mut sb_clean = String::new();
                for c in sb_no_ansi.chars() {
                    if c == '\x08' || c == '\x7f' {
                        sb_clean.pop();
                    } else {
                        sb_clean.push(c);
                    }
                }

                let lines: Vec<&str> = sb_clean.split(['\n', '\r']).collect();
                let last_non_empty = lines
                    .iter()
                    .rev()
                    .find(|l| !l.trim().is_empty())
                    .unwrap_or(&"");

                let cmd_candidate = if let Some(pos) = last_non_empty.rfind("$ ") {
                    &last_non_empty[pos + 2..]
                } else if let Some(pos) = last_non_empty.rfind("# ") {
                    &last_non_empty[pos + 2..]
                } else if let Some(pos) = last_non_empty.rfind("> ") {
                    &last_non_empty[pos + 2..]
                } else {
                    last_non_empty
                };

                cmd_candidate.trim().to_lowercase()
            };

            info!("[SECURITY] Sniffed command: '{}'", cmd_trimmed);

            let is_dangerous = term_consts::DANGEROUS_COMMANDS
                .iter()
                .any(|&d| cmd_trimmed == d || cmd_trimmed.starts_with(&format!("{d} ")));

            if is_dangerous {
                info!("[SECURITY] BLOCKED DANGEROUS COMMAND: {}", cmd_trimmed);
                let mut sessions = self.sessions.lock().await;
                let session = sessions.get_mut(session_id).ok_or(errors::SESSION_NOT_FOUND)?;
                let _ = session.writer.write_all(b"\x03\x15\x03");
                let _ = session.output_tx.send(format!(
                    "\r\n\x1b[1;31m[SECURITY] Command Intercepted: {cmd_trimmed}\x1b[0m\r\n"
                ));
                session.command_buffer = cmd_trimmed.clone();
                return Ok((Some(cmd_trimmed), None));
            }

            let mut sessions = self.sessions.lock().await;
            let session = sessions.get_mut(session_id).ok_or(errors::SESSION_NOT_FOUND)?;
            let _ = session.writer.write_all(data.as_bytes());

            if let Some(pid) = session._child.process_id() {
                let proc_path = format!("/proc/{}/cwd", pid);
                let pool = self.pool.clone();
                let session_id_str = session_id.to_string();
                let events_tx = self.events_tx.clone();

                tokio::spawn(async move {
                    tokio::time::sleep(tokio::time::Duration::from_millis(150)).await;

                    if let Ok(actual_cwd) = tokio::fs::read_link(&proc_path).await {
                        let new_path = actual_cwd.to_string_lossy().to_string();
                        info!("[PTY] Detected CWD for {}: {}", session_id_str, new_path);

                        let _ = sqlx::query(
                            "UPDATE ai_sessions SET working_directory = ? WHERE id = ?",
                        )
                        .bind(&new_path)
                        .bind(&session_id_str)
                        .execute(&pool)
                        .await;

                        let _ = events_tx.send(TerminalEvent::PathUpdated {
                            session_id: session_id_str,
                            new_path,
                        });
                    }
                });
            }

            session.command_buffer.clear();
            return Ok((None, None));
        }

        let mut sessions = self.sessions.lock().await;
        let session = sessions.get_mut(session_id).ok_or(errors::SESSION_NOT_FOUND)?;

        if data == "\x7f" || data == "\x08" {
            session.command_buffer.pop();
        } else {
            session.command_buffer.push_str(data);
        }

        let _ = session.writer.write_all(data.as_bytes());
        Ok((None, None))
    }

    pub async fn force_write_input(&self, session_id: &str, data: &str) {
        let mut sessions = self.sessions.lock().await;
        if let Some(session) = sessions.get_mut(session_id) {
            let _ = session.writer.write_all(data.as_bytes());
            session.command_buffer.clear();
        }
    }

    pub async fn resize_session(&self, session_id: &str, rows: u16, cols: u16) {
        let sessions = self.sessions.lock().await;
        if let Some(session) = sessions.get(session_id) {
            let _ = session.master.resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            });
            info!("[PTY] Session {} resized to {}x{}", session_id, cols, rows);
        }
    }

    pub async fn kill_session(&self, session_id: &str) {
        let mut sessions = self.sessions.lock().await;
        if let Some(mut session) = sessions.remove(session_id) {
            let _ = session._child.kill();
            info!("[PTY] Session {} terminated and process killed", session_id);
        }
    }

    pub async fn inject_message(
        &self,
        session_id: &str,
        from_id: &str,
        content: &str,
    ) -> Result<(), String> {
        if let Err(e) = self.save_message_to_db(session_id, from_id, content).await {
            error!(
                "[ORCHESTRATOR] Failed to save message to DB for session {}: {}",
                session_id, e
            );
        }

        if session_id == term_consts::MCP_AGENT_ID {
            return Ok(());
        }

        let injection = if let Some(stripped) = content.strip_prefix('/') {
            format!("\x1b[0m\x03{stripped}\r\n")
        } else {
            format!("\x1b[0m\r\n# [ATLAS MESSAGE FROM {from_id}]: {content}\r\n")
        };

        {
            let mut sessions = self.sessions.lock().await;
            let session = sessions
                .get_mut(session_id)
                .ok_or_else(|| errors::SESSION_NOT_FOUND.to_string())?;
            let _ = session.writer.write_all(injection.as_bytes());
            let _ = session.writer.flush();
        }

        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        {
            let mut sessions = self.sessions.lock().await;
            if let Some(session) = sessions.get_mut(session_id) {
                let _ = session.writer.write_all(b"\r");
                let _ = session.writer.flush();
            }
        }

        Ok(())
    }

    async fn save_message_to_db(
        &self,
        session_id: &str,
        from_id: &str,
        content: &str,
    ) -> Result<(), String> {
        let msg_id = ulid::Ulid::new().to_string();

        sqlx::query("INSERT INTO messages (id, session_id, from_id, content) VALUES (?, ?, ?, ?)")
            .bind(msg_id)
            .bind(session_id)
            .bind(from_id)
            .bind(content)
            .execute(&self.pool)
            .await
            .map_err(|e| e.to_string())?;

        Ok(())
    }
}
