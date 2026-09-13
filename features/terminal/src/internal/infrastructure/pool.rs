use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use atlas_sessions::api::{SessionId, SessionStore, SessionsError};
use portable_pty::{Child, CommandBuilder, MasterPty, PtySize, native_pty_system};

use crate::internal::domain::{RestoreOutcome, SpawnedTerminal, TerminalError, TerminalOutput};

/// One live PTY process. `_master` is never read after setup — it
/// exists in this struct purely so it isn't dropped, which would
/// tear down the PTY out from under the reader thread and the
/// writer.
struct PtyProcess {
    /// Who this terminal belongs to.
    ///
    /// Recorded at spawn, when ownership was checked against the
    /// session registry, so every later call can be authorised in
    /// memory without a database round trip per keystroke. Without it
    /// `write` was reachable for any session id a caller cared to
    /// name, which is arbitrary command execution in somebody else's
    /// shell — verified against a real container before this existed.
    owner_id: String,
    child: Box<dyn Child + Send + Sync>,
    writer: Box<dyn Write + Send>,
    buffer: Arc<Mutex<Vec<u8>>>,
    pid: u32,
    resolved_path: PathBuf,
    _master: Box<dyn MasterPty + Send>,
}

/// Every live PTY this `atlas-server` process holds, keyed by
/// session. Pure in-memory state (ephemeral state) — restarting the
/// server always starts empty, by design.
pub struct PtyPool {
    processes: Mutex<HashMap<SessionId, PtyProcess>>,
    sessions: SessionStore,
    workspace_root: PathBuf,
    /// What a terminal is told to call Atlas back on.
    server_url: String,
}

impl PtyPool {
    #[must_use]
    /// `server_url` is what a terminal is told to call Atlas back on.
    /// The pool cannot derive it — a listen address is not necessarily
    /// a reachable one — so the binary that knows passes it in.
    pub fn new(sessions: SessionStore, workspace_root: PathBuf, server_url: String) -> Self {
        Self {
            processes: Mutex::new(HashMap::new()),
            sessions,
            workspace_root,
            server_url,
        }
    }

    /// Spawning an already-running session is not an error — it
    /// returns the existing handle. "Is this already running?" is a
    /// legitimate question an agent asks before deciding to spawn.
    ///
    /// # Errors
    /// `SessionNotFound` if no session has this id, `PathNotFound`
    /// if the resolved directory doesn't exist on this machine,
    /// `Spawn` if the PTY or process could not be started.
    ///
    /// # Panics
    /// If the internal pool mutex is poisoned by a previous panic
    /// on another thread.
    pub async fn spawn(
        &self,
        session_id: &SessionId,
        owner_id: &str,
    ) -> Result<SpawnedTerminal, TerminalError> {
        if let Some(existing) = self
            .processes
            .lock()
            .expect("pty pool poisoned")
            .get(session_id)
        {
            if existing.owner_id != owner_id {
                return Err(TerminalError::SessionNotFound);
            }
            return Ok(SpawnedTerminal {
                session_id: session_id.clone(),
                pid: existing.pid,
                resolved_path: existing.resolved_path.clone(),
            });
        }

        // Owner-scoped, not `get_unscoped`: a session belonging to
        // somebody else must be indistinguishable from one that does
        // not exist, or a caller learns which ids are real by probing.
        let session = self
            .sessions
            .get(session_id, owner_id)
            .await
            .map_err(|e| match e {
                SessionsError::NotFound => TerminalError::SessionNotFound,
                other => TerminalError::Io(other.to_string()),
            })?;
        let resolved_path = self.workspace_root.join(&session.relative_path);
        if !resolved_path.is_dir() {
            return Err(TerminalError::PathNotFound(resolved_path));
        }

        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows: 24,
                cols: 80,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| TerminalError::Spawn(e.to_string()))?;

        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_owned());
        let mut cmd = CommandBuilder::new(shell);
        cmd.cwd(&resolved_path);

        self.hand_the_terminal_its_identity(&mut cmd, session_id, owner_id)
            .await?;

        let child = pair
            .slave
            .spawn_command(cmd)
            .map_err(|e| TerminalError::Spawn(e.to_string()))?;
        drop(pair.slave);
        let pid = child.process_id().unwrap_or(0);

        let writer = pair
            .master
            .take_writer()
            .map_err(|e| TerminalError::Spawn(e.to_string()))?;
        let mut reader = pair
            .master
            .try_clone_reader()
            .map_err(|e| TerminalError::Spawn(e.to_string()))?;

        let buffer = Arc::new(Mutex::new(Vec::new()));
        let buffer_for_thread = Arc::clone(&buffer);
        std::thread::spawn(move || {
            let mut chunk = [0u8; 4096];
            loop {
                match reader.read(&mut chunk) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => buffer_for_thread
                        .lock()
                        .expect("pty output buffer poisoned")
                        .extend_from_slice(&chunk[..n]),
                }
            }
        });

        self.processes.lock().expect("pty pool poisoned").insert(
            session_id.clone(),
            PtyProcess {
                owner_id: owner_id.to_owned(),
                child,
                writer,
                buffer,
                pid,
                resolved_path: resolved_path.clone(),
                _master: pair.master,
            },
        );

        // After the process is in the pool, so a resume command that
        // wedges cannot leave a running PTY nobody can reach.
        //
        // Only on a real spawn, never on the idempotent early return
        // above: `terminal.spawn` is safe to call repeatedly, and a
        // caller polling it would otherwise re-run this command every
        // time.
        if let Some(command) = session.resume_command.as_deref() {
            let command = command.trim();
            if !command.is_empty() {
                // Typed into the shell exactly as the owner would have
                // typed it. Atlas does not interpret it, wrap it, or
                // check whether it worked — its output lands in the
                // buffer like anything else, and deciding it succeeded
                // would be a guess about somebody else's program.
                self.write(session_id, owner_id, format!("{command}\n").as_bytes())?;
            }
        }

        Ok(SpawnedTerminal {
            session_id: session_id.clone(),
            pid,
            resolved_path,
        })
    }

    /// Make the resolved path exist on this
    /// machine, cloning it from the session's `remote_url`/`branch`
    /// if it isn't there yet. Idempotent — restoring an
    /// already-present path is a no-op, not an error, since "is this
    /// already here?" is the normal way a caller uses this before
    /// spawning.
    ///
    /// Deliberately does not attempt to resolve a missing
    /// credential, only to name it: Atlas is explicit that it
    /// "detects and warns, does not resolve" that case.
    ///
    /// # Errors
    /// `SessionNotFound` if no session has this id,
    /// `CredentialsMissing` if the clone failed in a way that looks
    /// like an auth problem, `CloneFailed` for any other clone
    /// failure.
    pub async fn restore(
        &self,
        session_id: &SessionId,
        owner_id: &str,
    ) -> Result<RestoreOutcome, TerminalError> {
        let session = self
            .sessions
            .get(session_id, owner_id)
            .await
            .map_err(|e| match e {
                SessionsError::NotFound => TerminalError::SessionNotFound,
                other => TerminalError::Io(other.to_string()),
            })?;
        let resolved_path = self.workspace_root.join(&session.relative_path);
        if resolved_path.is_dir() {
            return Ok(RestoreOutcome::AlreadyPresent);
        }

        let output = tokio::process::Command::new("git")
            // Never block the server process on an interactive
            // username/password prompt for a remote with no usable
            // credential — fail fast instead, so `restore` returns
            // `CredentialsMissing` rather than hanging forever.
            .env("GIT_TERMINAL_PROMPT", "0")
            .arg("clone")
            .arg("--branch")
            .arg(&session.branch)
            .arg("--single-branch")
            .arg(&session.remote_url)
            .arg(&resolved_path)
            .output()
            .await
            .map_err(|e| TerminalError::CloneFailed(e.to_string()))?;

        if output.status.success() {
            return Ok(RestoreOutcome::Cloned);
        }

        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        Err(classify_clone_failure(stderr))
    }

    /// Puts this session's identity into the terminal's environment.
    ///
    /// What closes the loop: an agent working in this terminal can call
    /// Atlas back, and every call it makes is attributable to this
    /// session and its owner rather than to a shared secret.
    ///
    /// The token is minted for this terminal rather than reusing the
    /// one `sessions.create` returned, because only that one's hash is
    /// stored and it cannot be recovered. Both resolve to the same
    /// identity, and deleting the session revokes every one of them
    /// through the foreign key.
    ///
    /// `ATLAS_MCP_TOKEN` is *removed* rather than merely not set.
    /// `CommandBuilder` inherits this process's environment, so the
    /// server's own shared token reaches every terminal it spawns
    /// unless something takes it out — verified against a running
    /// container, where `env | grep -c ATLAS_MCP_TOKEN` answered 1. A
    /// terminal that has a specific identity should not also hold the
    /// shared one: on a team server it reaches nobody's data but is
    /// still a secret in every developer's shell, printable by any
    /// `env` an agent runs.
    async fn hand_the_terminal_its_identity(
        &self,
        cmd: &mut CommandBuilder,
        session_id: &SessionId,
        owner_id: &str,
    ) -> Result<(), TerminalError> {
        let session_token = self
            .sessions
            .issue_token(session_id, owner_id)
            .await
            .map_err(|e| match e {
                SessionsError::NotFound => TerminalError::SessionNotFound,
                other => TerminalError::Io(other.to_string()),
            })?;
        cmd.env("ATLAS_SESSION_TOKEN", &session_token);
        cmd.env("ATLAS_SESSION_ID", &session_id.0);
        cmd.env("ATLAS_URL", &self.server_url);
        cmd.env_remove("ATLAS_MCP_TOKEN");
        Ok(())
    }

    /// # Errors
    /// `NotRunning` if no PTY is live for that session, `Io` if the
    /// write itself fails.
    ///
    /// # Panics
    /// If the internal pool mutex is poisoned by a previous panic
    /// on another thread.
    pub fn write(
        &self,
        session_id: &SessionId,
        owner_id: &str,
        input: &[u8],
    ) -> Result<(), TerminalError> {
        let mut guard = self.processes.lock().expect("pty pool poisoned");
        let process = guard
            .get_mut(session_id)
            .filter(|p| p.owner_id == owner_id)
            .ok_or(TerminalError::NotRunning)?;
        process
            .writer
            .write_all(input)
            .map_err(|e| TerminalError::Io(e.to_string()))
    }

    /// `since_offset` past the current buffer length is not an
    /// error — it just means "nothing new yet," the normal case for
    /// a poll loop that outran the process's output.
    ///
    /// # Errors
    /// `NotRunning` if no PTY is live for that session.
    ///
    /// # Panics
    /// If the internal pool mutex or the output buffer mutex is
    /// poisoned by a previous panic on another thread.
    pub fn read_output(
        &self,
        session_id: &SessionId,
        owner_id: &str,
        since_offset: usize,
    ) -> Result<TerminalOutput, TerminalError> {
        let guard = self.processes.lock().expect("pty pool poisoned");
        let process = guard
            .get(session_id)
            .filter(|p| p.owner_id == owner_id)
            .ok_or(TerminalError::NotRunning)?;
        let buffer = process.buffer.lock().expect("pty output buffer poisoned");
        let start = since_offset.min(buffer.len());
        Ok(TerminalOutput {
            data: buffer[start..].to_vec(),
            next_offset: buffer.len(),
        })
    }

    /// # Errors
    /// `NotRunning` if no PTY is live for that session.
    ///
    /// # Panics
    /// If the internal pool mutex is poisoned by a previous panic
    /// on another thread.
    pub fn close(&self, session_id: &SessionId, owner_id: &str) -> Result<(), TerminalError> {
        let mut guard = self.processes.lock().expect("pty pool poisoned");
        // Checked before removing: a mismatched owner must not be able
        // to take somebody's terminal out of the pool and then be told
        // it was not theirs.
        if guard.get(session_id).is_none_or(|p| p.owner_id != owner_id) {
            return Err(TerminalError::NotRunning);
        }
        let mut process = guard.remove(session_id).ok_or(TerminalError::NotRunning)?;
        let _ = process.child.kill();
        Ok(())
    }
}

/// `git`'s own error text is the only signal available here — there
/// is no structured exit code for "this needed a credential I don't
/// have." Matched case-insensitively against the phrases git and
/// common credential helpers actually emit.
fn classify_clone_failure(stderr: String) -> TerminalError {
    let lower = stderr.to_lowercase();
    let looks_like_credentials = [
        "permission denied",
        "could not read username",
        "authentication failed",
        "terminal prompts disabled",
    ]
    .iter()
    .any(|needle| lower.contains(needle));
    if looks_like_credentials {
        TerminalError::CredentialsMissing(stderr)
    } else {
        TerminalError::CloneFailed(stderr)
    }
}

#[cfg(test)]
mod tests {
    use super::classify_clone_failure;
    use crate::internal::domain::TerminalError;

    #[test]
    fn permission_denied_is_credentials_missing() {
        let err = classify_clone_failure(
            "git@gitlab.com: Permission denied (publickey).\nfatal: Could not read from remote repository.".to_owned(),
        );
        assert!(matches!(err, TerminalError::CredentialsMissing(_)));
    }

    #[test]
    fn https_prompt_disabled_is_credentials_missing() {
        let err = classify_clone_failure(
            "fatal: could not read Username for 'https://gitlab.com': terminal prompts disabled"
                .to_owned(),
        );
        assert!(matches!(err, TerminalError::CredentialsMissing(_)));
    }

    #[test]
    fn missing_repository_is_a_generic_clone_failure() {
        let err = classify_clone_failure(
            "fatal: repository 'file:///does/not/exist.git' does not exist".to_owned(),
        );
        assert!(matches!(err, TerminalError::CloneFailed(_)));
    }
}
