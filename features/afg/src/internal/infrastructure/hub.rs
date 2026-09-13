//! In-process fan-out for AFG's live run view (these
//! events carry their payload, because workflow node events are
//! append-only and immutable).
//!
//! Subscribers filter by run — a watcher of one run is not interested
//! in another's, and there is no per-run channel to create and reap.

use tokio::sync::broadcast;

use crate::internal::domain::WorkflowNodeEvent;

/// How far behind a watcher may fall before it starts missing
/// events. Generous, because unlike a sync notification a missed
/// node event has no later pull to recover it — a lagging watcher
/// is told its view is incomplete rather than quietly given a
/// timeline with a hole in it.
const CHANNEL_CAPACITY: usize = 512;

#[derive(Clone)]
pub struct RunEventHub {
    tx: broadcast::Sender<WorkflowNodeEvent>,
}

impl RunEventHub {
    #[must_use]
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(CHANNEL_CAPACITY);
        Self { tx }
    }

    /// Fire-and-forget. With nobody watching there are no
    /// subscribers and this is a no-op — the common case, and why
    /// the write path can call it unconditionally.
    pub fn publish(&self, event: &WorkflowNodeEvent) {
        let _ = self.tx.send(event.clone());
    }

    #[must_use]
    pub fn subscribe(&self) -> broadcast::Receiver<WorkflowNodeEvent> {
        self.tx.subscribe()
    }
}

impl Default for RunEventHub {
    fn default() -> Self {
        Self::new()
    }
}
