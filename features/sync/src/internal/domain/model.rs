//! What the sync engine talks about: modes, configuration, the
//! report of a pass, and the kinds of change that travel.

use chrono::{DateTime, Utc};

/// Client-side outcome of one sync pass, `focus` or `auto` alike.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SyncReport {
    /// Sessions sent to the remote (regardless of which side of
    /// last-write-wins ultimately won).
    pub pushed: usize,
    /// Sessions applied locally from the remote's pull response.
    pub pulled: usize,
}

/// propagation switch, now complete.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SyncMode {
    /// Sync only on an explicit ask. The default.
    #[default]
    Focus,
    /// A background loop calls `sync_sessions` on a fixed interval.
    Auto,
    /// A held-open SSE stream triggers a pass the moment the remote
    /// says something changed.
    Live,
}

/// What `auto` mode needs to run unattended. Lives only in process
/// memory (see [`crate::api::SyncSupervisor`]) — never persisted,
/// deliberately paralleling token-custody minimalism.
#[derive(Clone, Debug)]
pub struct SyncConfig {
    pub remote_url: String,
    pub bearer_token: String,
    pub owner_id: String,
    pub interval_secs: u64,
}

/// What `live` mode needs. Deliberately not [`SyncConfig`] with an
/// ignored field: `live` has no poll cadence at all, and a struct
/// carrying an `interval_secs` that means nothing invites someone to
/// set it and expect an effect.
#[derive(Clone, Debug)]
pub struct LiveConfig {
    pub remote_url: String,
    pub bearer_token: String,
    pub owner_id: String,
}

/// What kind of thing changed on the remote. Carries no state — the
/// client answers a notification with the ordinary pull,
/// so this exists only to tell a client whether the change is one it
/// cares about.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChangeKind {
    Sessions,
    /// Personal memory only. Project memory (Type 1) is nobody's
    /// personal event, so a change to it notifies no one.
    Memory,
    /// Notes are Type 2 throughout, so every change to one is its
    /// owner's event — there is no shared variant to exclude.
    Notes,
}

impl ChangeKind {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Sessions => "sessions",
            Self::Memory => "memory",
            Self::Notes => "notes",
        }
    }

    /// `None` for a kind this build doesn't know — a newer server
    /// may announce kinds an older client has no handler for, and
    /// reacting to those by syncing something unrelated would be
    /// worse than ignoring them.
    #[must_use]
    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "sessions" => Some(Self::Sessions),
            "memory" => Some(Self::Memory),
            _ => None,
        }
    }
}

/// One change notification, addressed to a single owner. Server-side
/// fan-out filters on `owner_id` so a subscriber never observes that
/// another developer's state moved at all.
#[derive(Clone, Debug)]
pub struct OwnerChange {
    pub audience: Audience,
    pub kind: ChangeKind,
}

/// Who a change is addressed to.
///
/// Stated in the type rather than left implicit in a string, because
/// the two cases are not variations of one thing: personal state moves
/// for exactly one developer, and project state moves for everyone on
/// the server by definition (project state).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Audience {
    /// One developer. Nobody else is told.
    Owner(String),
    /// Type 1 state, which is shared on purpose.
    ///
    /// The notification carries no content — only that *something*
    /// project-scoped moved — and the pull that answers it is
    /// authenticated and filtered as always. So the most a listener
    /// learns is that the server was busy.
    Everyone,
}

/// Point-in-time view of the supervisor, for the `sync.status` tool.
#[derive(Clone, Debug, Default)]
pub struct SyncStatusReport {
    pub mode: SyncMode,
    pub interval_secs: Option<u64>,
    /// `Some` only in `live` mode. A long-lived
    /// connection's failure modes stay visible — without this a
    /// silent disconnect is indistinguishable from a quiet team.
    pub stream_connected: Option<bool>,
    pub last_synced_at: Option<DateTime<Utc>>,
    pub last_report: Option<SyncReport>,
    pub last_error: Option<String>,
}
