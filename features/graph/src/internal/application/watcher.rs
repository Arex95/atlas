//! Keeping the graph fresh.
//!
//! A stale index is worse than no index: an agent trusts what it
//! reads, and a graph describing the repository as it was an hour ago
//! sends it somewhere that has since moved. Slice 1 left reindexing to
//! whoever remembered to ask; this watches instead.
//!
//! Three things this has to get right, and each has a failure mode
//! that shows up only on a real repository:
//!
//! * **Filter the events the walker would ignore.** A `cargo build`
//!   writes thousands of files under `target/`. A watcher that does
//!   not filter reindexes continuously for as long as the build runs,
//!   and the machine never settles.
//! * **Debounce.** Saving a file emits several events; a `git
//!   checkout` emits thousands. Reindexing per event would spend all
//!   its time on work each next event invalidates.
//! * **Never overlap two reindexes of the same project.** They race on
//!   the same rows, and the loser wastes a full pass.
//!
//! Like `auto` and `live` sync, what is being watched lives only in
//! process memory. A restart watches nothing until asked again.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use tokio::sync::{Mutex, mpsc};
use tokio::task::JoinHandle;

use crate::internal::application::Indexer;
use crate::internal::domain::{GraphError, IndexStats};
use crate::internal::infrastructure::{GraphStore, is_ignored_path};

/// How long to wait for the filesystem to go quiet before reindexing.
///
/// A full pass over this repository takes about 400 ms, so anything
/// much shorter would spend more time indexing than waiting. Long
/// enough to collapse an editor's save burst, short enough that a
/// change is in the graph before an agent asks about it.
const DEBOUNCE: Duration = Duration::from_millis(500);

/// Filesystem events buffered before the channel starts dropping them.
///
/// A dropped event is survivable here in a way it is not elsewhere:
/// every reindex is a full rebuild, so one that runs slightly later
/// than it should still produces the correct graph. Losing an event
/// costs latency, never correctness.
const CHANNEL_CAPACITY: usize = 1024;

struct Watch {
    handle: JoinHandle<()>,
    root: PathBuf,
    /// Shared with the task so `status` reports what actually
    /// happened, not what was asked for.
    last: Arc<Mutex<LastRun>>,
}

#[derive(Clone, Debug, Default)]
struct LastRun {
    reindexes: u64,
    last_stats: Option<IndexStats>,
    last_error: Option<String>,
}

/// One entry in [`GraphWatcher::status`].
#[derive(Clone, Debug)]
pub struct WatchStatus {
    pub project: String,
    pub root: String,
    /// Reindexes triggered by file changes since watching began. Does
    /// not count the one that runs when watching starts.
    pub reindexes: u64,
    pub last_stats: Option<IndexStats>,
    pub last_error: Option<String>,
}

/// Owns every running watcher for this process, keyed by project.
pub struct GraphWatcher {
    store: GraphStore,
    watches: Mutex<HashMap<String, Watch>>,
}

impl GraphWatcher {
    #[must_use]
    pub fn new(store: GraphStore) -> Self {
        Self {
            store,
            watches: Mutex::new(HashMap::new()),
        }
    }

    /// Indexes `root` once, then keeps it indexed as files change.
    ///
    /// Watching a project already watched replaces the watch rather
    /// than adding a second — two watchers on one root would reindex
    /// twice per change and race each other.
    ///
    /// The initial index runs before returning, so a caller that gets
    /// `Ok` knows the graph is current rather than merely promised.
    ///
    /// # Errors
    /// Whatever the initial index fails with, or `Walk` if the path
    /// cannot be watched.
    pub async fn watch(&self, project: &str, root: &Path) -> Result<IndexStats, GraphError> {
        let stats = Indexer::new(self.store.clone())
            .reindex(project, root)
            .await?;

        self.unwatch(project).await;

        let (tx, mut rx) = mpsc::channel::<()>(CHANNEL_CAPACITY);
        let watched_root = root.to_path_buf();

        // The `notify` watcher must outlive this function or it stops
        // delivering, so it is moved into the task below.
        let mut fs_watcher = RecommendedWatcher::new(
            move |result: notify::Result<Event>| {
                let Ok(event) = result else { return };
                if !is_interesting(&event) {
                    return;
                }
                // Full channel means a burst larger than the buffer,
                // and the debounce is about to coalesce it anyway.
                let _ = tx.try_send(());
            },
            notify::Config::default(),
        )
        .map_err(|e| GraphError::Walk(e.to_string()))?;

        fs_watcher
            .watch(root, RecursiveMode::Recursive)
            .map_err(|e| GraphError::Walk(e.to_string()))?;

        let last = Arc::new(Mutex::new(LastRun::default()));
        let task_last = Arc::clone(&last);
        let store = self.store.clone();
        let task_project = project.to_owned();
        let task_root = watched_root.clone();

        let handle = tokio::spawn(async move {
            // Moved in so the watcher lives exactly as long as the
            // task that consumes its events.
            let _fs_watcher = fs_watcher;
            let indexer = Indexer::new(store);

            while rx.recv().await.is_some() {
                // Drain the burst: wait for quiet, swallowing whatever
                // arrives meanwhile. One reindex covers all of it,
                // because every reindex is a full rebuild.
                // Quiet, or the sender is gone — the last batch
                // either way.
                while let Ok(Some(())) = tokio::time::timeout(DEBOUNCE, rx.recv()).await {}

                let outcome = indexer.reindex(&task_project, &task_root).await;
                let mut guard = task_last.lock().await;
                guard.reindexes += 1;
                match outcome {
                    Ok(stats) => {
                        guard.last_stats = Some(stats);
                        guard.last_error = None;
                    }
                    Err(e) => {
                        // A failed pass does not stop watching. The
                        // usual cause is a file that vanished mid-walk
                        // during a branch switch, and the next event
                        // is moments away.
                        tracing::warn!(project = task_project, error = %e, "reindex failed, still watching");
                        guard.last_error = Some(e.to_string());
                    }
                }
            }
        });

        self.watches.lock().await.insert(
            project.to_owned(),
            Watch {
                handle,
                root: watched_root,
                last,
            },
        );

        Ok(stats)
    }

    /// Stops watching a project. Idempotent — unwatching something
    /// that was never watched is not an error, because a caller
    /// cleaning up should not have to check first.
    pub async fn unwatch(&self, project: &str) -> bool {
        match self.watches.lock().await.remove(project) {
            Some(watch) => {
                watch.handle.abort();
                true
            }
            None => false,
        }
    }

    /// What is being watched, and how each watch is doing.
    pub async fn status(&self) -> Vec<WatchStatus> {
        let watches = self.watches.lock().await;
        let mut out = Vec::with_capacity(watches.len());
        for (project, watch) in watches.iter() {
            let last = watch.last.lock().await.clone();
            out.push(WatchStatus {
                project: project.clone(),
                root: watch.root.display().to_string(),
                reindexes: last.reindexes,
                last_stats: last.last_stats,
                last_error: last.last_error,
            });
        }
        out.sort_by(|a, b| a.project.cmp(&b.project));
        out
    }
}

/// Whether an event is worth a reindex.
///
/// Access events are dropped outright: reading a file changes nothing,
/// and on some platforms every read the indexer itself performs would
/// otherwise schedule the next one.
fn is_interesting(event: &Event) -> bool {
    if matches!(event.kind, EventKind::Access(_)) {
        return false;
    }
    event.paths.iter().any(|p| !is_ignored_path(p))
}

impl Drop for GraphWatcher {
    fn drop(&mut self) {
        if let Ok(mut guard) = self.watches.try_lock() {
            for (_, watch) in guard.drain() {
                watch.handle.abort();
            }
        }
    }
}
