//! The optional background loop behind two non-default
//! modes: `auto` (poll on an interval) and `live` (hold an SSE stream
//! open and pass on every notification). `focus` is the
//! default and runs no loop at all — this type exists purely to hold
//! whichever loop is armed, and its status.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use atlas_memory::api::MemoryStore;
use atlas_notes::api::NoteStore;
use atlas_sessions::api::SessionStore;
use chrono::{DateTime, Utc};
use futures_util::StreamExt;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

use crate::internal::domain::{
    ChangeKind, LiveConfig, SyncConfig, SyncError, SyncMode, SyncReport, SyncStatusReport,
};
use crate::internal::infrastructure::client::SyncClient;

/// Reconnect backoff bounds for `live` mode. Starts eager, because
/// the common disconnect is a blip; caps low enough that a server
/// coming back after an outage is noticed within half a minute.
const RECONNECT_MIN: Duration = Duration::from_secs(1);

/// How long a local change may sit unsent in `live` mode.
///
/// `live` reacts to the remote; nothing tells it about writes made
/// here. Fifteen seconds is short enough that a teammate is not
/// waiting on you and long enough that an idle pair of machines is not
/// talking for the sake of it.
const LOCAL_CHANGE_FLOOR: Duration = Duration::from_secs(15);
const RECONNECT_MAX: Duration = Duration::from_secs(30);

#[derive(Clone, Debug, Default)]
struct LastRun {
    synced_at: Option<DateTime<Utc>>,
    report: Option<SyncReport>,
    error: Option<String>,
}

struct RunningLoop {
    handle: JoinHandle<()>,
    mode: SyncMode,
    /// `auto` only.
    interval_secs: Option<u64>,
    /// `live` only — shared with the loop, which flips it as the
    /// stream connects and drops.
    connected: Option<Arc<AtomicBool>>,
}

/// Owns at most one running loop for this process. A personal
/// `atlas-server` instance has exactly one developer using it, so
/// this is process-global state, not per-request.
pub struct SyncSupervisor {
    sessions: SessionStore,
    memory: MemoryStore,
    notes: NoteStore,
    running: Mutex<Option<RunningLoop>>,
    last: Arc<Mutex<LastRun>>,
}

impl SyncSupervisor {
    #[must_use]
    pub fn new(sessions: SessionStore, memory: MemoryStore, notes: NoteStore) -> Self {
        Self {
            sessions,
            memory,
            notes,
            running: Mutex::new(None),
            last: Arc::new(Mutex::new(LastRun::default())),
        }
    }

    /// Starts (or replaces) the background loop. `config` is held
    /// only in memory for the life of the loop — never persisted;
    /// a server restart drops back to `focus` and requires
    /// re-arming `auto` mode.
    ///
    /// # Errors
    /// `BadUrl` if `config.remote_url` doesn't parse — checked
    /// up-front so a bad config fails immediately rather than on
    /// the loop's first silently-logged tick.
    pub async fn set_auto(&self, config: SyncConfig) -> Result<(), SyncError> {
        let client = SyncClient::new(&config.remote_url)?;
        self.stop().await;

        let sessions = self.sessions.clone();
        let memory = self.memory.clone();
        let notes = self.notes.clone();
        let last = Arc::clone(&self.last);
        let interval_secs = config.interval_secs;
        let handle = tokio::spawn(async move {
            let mut ticker = tokio::time::interval(Duration::from_secs(interval_secs.max(1)));
            loop {
                ticker.tick().await;
                run_sessions_pass(
                    &client,
                    &config.bearer_token,
                    &config.owner_id,
                    &sessions,
                    &last,
                )
                .await;
                run_memory_pass(
                    &client,
                    &config.bearer_token,
                    &config.owner_id,
                    &memory,
                    &last,
                )
                .await;
                run_notes_pass(
                    &client,
                    &config.bearer_token,
                    &config.owner_id,
                    &notes,
                    &last,
                )
                .await;
            }
        });

        *self.running.lock().await = Some(RunningLoop {
            handle,
            mode: SyncMode::Auto,
            interval_secs: Some(interval_secs),
            connected: None,
        });
        Ok(())
    }

    /// Starts (or replaces) the `live` loop: holds the remote's SSE
    /// stream open and runs a pass per notification, reconnecting on
    /// its own when the connection drops. Same in-memory-only custody
    /// as [`Self::set_auto`].
    ///
    /// # Errors
    /// `BadUrl` if `config.remote_url` doesn't parse. A remote that
    /// is merely *down* is not an error here — that is what the
    /// reconnect loop is for, and failing the call would make `live`
    /// mode unavailable exactly when it is most wanted.
    pub async fn set_live(&self, config: LiveConfig) -> Result<(), SyncError> {
        let client = SyncClient::new(&config.remote_url)?;
        self.stop().await;

        let sessions = self.sessions.clone();
        let memory = self.memory.clone();
        let notes = self.notes.clone();
        let last = Arc::clone(&self.last);
        let connected = Arc::new(AtomicBool::new(false));
        let connected_in_loop = Arc::clone(&connected);

        let handle = tokio::spawn(async move {
            let mut backoff = RECONNECT_MIN;
            loop {
                match client.events(&config.bearer_token).await {
                    Ok(stream) => {
                        connected_in_loop.store(true, Ordering::Relaxed);
                        backoff = RECONNECT_MIN;

                        // Converge once on connect. Anything that
                        // changed while we were disconnected produced
                        // a notification nobody was listening for —
                        // harmless precisely because the pull that
                        // follows doesn't care how it was triggered.
                        converge(&client, &config, &sessions, &memory, &notes, &last).await;

                        let mut stream = Box::pin(stream);
                        // A floor under the stream, not an interval to
                        // tune.
                        //
                        // The stream announces what changed on the
                        // *remote*. Nothing announces what changed
                        // here: a developer who writes a note locally
                        // produces no event, so a purely reactive loop
                        // never pushes it. That was invisible while the
                        // server announced every push — the resulting
                        // storm ran passes constantly and pushed local
                        // work as a side effect of spinning.
                        //
                        // This bounds how long a local change can sit
                        // unsent. It comes out the day the stores
                        // announce their own writes, which is the real
                        // fix and a larger one.
                        let mut floor = tokio::time::interval(LOCAL_CHANGE_FLOOR);
                        floor.tick().await; // fires immediately; skip it

                        loop {
                            let item = tokio::select! {
                                next = stream.next() => match next {
                                    Some(item) => item,
                                    None => break,
                                },
                                _ = floor.tick() => {
                                    converge(&client, &config, &sessions, &memory, &notes, &last).await;
                                    continue;
                                }
                            };
                            match item {
                                // Routed by kind rather than running
                                // everything: a memory nudge dragging a
                                // sessions pass along would make every
                                // event cost twice what it announces.
                                Ok(ChangeKind::Sessions) => {
                                    run_sessions_pass(
                                        &client,
                                        &config.bearer_token,
                                        &config.owner_id,
                                        &sessions,
                                        &last,
                                    )
                                    .await;
                                }
                                Ok(ChangeKind::Memory) => {
                                    run_memory_pass(
                                        &client,
                                        &config.bearer_token,
                                        &config.owner_id,
                                        &memory,
                                        &last,
                                    )
                                    .await;
                                }
                                Ok(ChangeKind::Notes) => {
                                    run_notes_pass(
                                        &client,
                                        &config.bearer_token,
                                        &config.owner_id,
                                        &notes,
                                        &last,
                                    )
                                    .await;
                                }
                                Err(e) => {
                                    tracing::warn!(error = %e, "live event stream failed, reconnecting");
                                    break;
                                }
                            }
                        }
                        connected_in_loop.store(false, Ordering::Relaxed);
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "could not open live event stream, retrying");
                        last.lock().await.error = Some(e.to_string());
                    }
                }

                tokio::time::sleep(backoff).await;
                backoff = (backoff * 2).min(RECONNECT_MAX);
            }
        });

        *self.running.lock().await = Some(RunningLoop {
            handle,
            mode: SyncMode::Live,
            interval_secs: None,
            connected: Some(connected),
        });
        Ok(())
    }

    /// Stops the background loop, if one is running. Idempotent.
    pub async fn set_focus(&self) {
        self.stop().await;
    }

    async fn stop(&self) {
        if let Some(running) = self.running.lock().await.take() {
            running.handle.abort();
        }
    }

    pub async fn status(&self) -> SyncStatusReport {
        let running = self.running.lock().await;
        let last = self.last.lock().await.clone();
        SyncStatusReport {
            mode: running.as_ref().map_or(SyncMode::Focus, |r| r.mode),
            interval_secs: running.as_ref().and_then(|r| r.interval_secs),
            stream_connected: running
                .as_ref()
                .and_then(|r| r.connected.as_ref())
                .map(|flag| flag.load(Ordering::Relaxed)),
            last_synced_at: last.synced_at,
            last_report: last.report,
            last_error: last.error,
        }
    }
}

/// One session sync pass, with its outcome recorded for `sync.status`.
///
/// A transient failure never stops the caller's loop — that would
/// silently turn `auto` or `live` into `focus`-that-never-recovers.
async fn run_sessions_pass(
    client: &SyncClient,
    bearer_token: &str,
    owner_id: &str,
    sessions: &SessionStore,
    last: &Mutex<LastRun>,
) {
    let outcome = client.sync_sessions(bearer_token, owner_id, sessions).await;
    record(outcome, last).await;
}

/// One memory sync pass. Project memory replicates freely and
/// personal memory only under `owner_id` — the client and the server
/// each enforce that independently.
async fn run_memory_pass(
    client: &SyncClient,
    bearer_token: &str,
    owner_id: &str,
    memory: &MemoryStore,
    last: &Mutex<LastRun>,
) {
    let outcome = client.sync_memory(bearer_token, owner_id, memory).await;
    record(outcome, last).await;
}

/// Notes are Type 2 throughout, so unlike memory there is no bucket to
/// route between — every note on both legs is forced under `owner_id`,
/// by the client and by the server independently.
/// Every kind, once. Used where the trigger says nothing about what
/// moved — on connect, and on the floor tick.
async fn converge(
    client: &SyncClient,
    config: &LiveConfig,
    sessions: &SessionStore,
    memory: &MemoryStore,
    notes: &NoteStore,
    last: &Mutex<LastRun>,
) {
    run_sessions_pass(
        client,
        &config.bearer_token,
        &config.owner_id,
        sessions,
        last,
    )
    .await;
    run_memory_pass(client, &config.bearer_token, &config.owner_id, memory, last).await;
    run_notes_pass(client, &config.bearer_token, &config.owner_id, notes, last).await;
}

async fn run_notes_pass(
    client: &SyncClient,
    bearer_token: &str,
    owner_id: &str,
    notes: &NoteStore,
    last: &Mutex<LastRun>,
) {
    let outcome = client.sync_notes(bearer_token, owner_id, notes).await;
    record(outcome, last).await;
}

async fn record(outcome: Result<SyncReport, SyncError>, last: &Mutex<LastRun>) {
    let mut guard = last.lock().await;
    guard.synced_at = Some(Utc::now());
    match outcome {
        Ok(report) => {
            guard.report = Some(report);
            guard.error = None;
        }
        Err(e) => {
            tracing::warn!(error = %e, "sync pass failed");
            guard.error = Some(e.to_string());
        }
    }
}

impl Drop for SyncSupervisor {
    fn drop(&mut self) {
        if let Ok(mut guard) = self.running.try_lock()
            && let Some(running) = guard.take()
        {
            running.handle.abort();
        }
    }
}
