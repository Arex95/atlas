//! Server-side fan-out for `live` mode: one in-process
//! broadcast every held-open SSE stream subscribes to.
//!
//! Nothing here is persisted. A subscriber that is disconnected when
//! a change lands misses the notification entirely, and that is
//! by design — reconnecting triggers an ordinary pull, which reaches
//! the same state whether it replays one missed event or a hundred.

use tokio::sync::broadcast;

use crate::internal::domain::{Audience, ChangeKind, OwnerChange};

/// How far behind a subscriber may fall before it starts missing
/// notifications. This is lag tolerance, not a work queue: falling
/// behind costs nothing but a coalesced catch-up, since the pull that
/// answers any single event already brings everything.
const CHANNEL_CAPACITY: usize = 64;

#[derive(Clone)]
pub struct ChangeHub {
    tx: broadcast::Sender<OwnerChange>,
}

impl ChangeHub {
    #[must_use]
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(CHANNEL_CAPACITY);
        Self { tx }
    }

    /// Fire-and-forget. With nobody in `live` mode there are no
    /// subscribers and this is a no-op — the common case, and the
    /// reason a write path can call it unconditionally.
    pub fn publish(&self, owner_id: &str, kind: ChangeKind) {
        self.publish_to(Audience::Owner(owner_id.to_owned()), kind);
    }

    /// Announce a change to everyone listening.
    ///
    /// For Type 1 state, which has no owner to address: project memory
    /// that only its writer hears about is not shared state, it is
    /// personal state with a misleading name.
    pub fn publish_to(&self, audience: Audience, kind: ChangeKind) {
        let _ = self.tx.send(OwnerChange { audience, kind });
    }

    #[must_use]
    pub fn subscribe(&self) -> broadcast::Receiver<OwnerChange> {
        self.tx.subscribe()
    }
}

impl Default for ChangeHub {
    fn default() -> Self {
        Self::new()
    }
}
