use std::fmt;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A ULID (repo convention: sortable, not UUID).
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SessionId(pub String);

impl fmt::Display for SessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Two states only — no PTY-derived status (running/exited/crashed)
/// exists here yet, since no PTY spawning happens in this crate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SessionStatus {
    Active,
    Archived,
}

impl SessionStatus {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Archived => "archived",
        }
    }

    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "active" => Some(Self::Active),
            "archived" => Some(Self::Archived),
            _ => None,
        }
    }
}

/// A registered session — metadata only, no PTY.
///
/// `relative_path` is relative to the machine's own
/// `workspace_root` — resolving it into an absolute, usable path is
/// a later feature's job, not this crate's.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Session {
    pub id: SessionId,
    pub project: String,
    pub owner_id: String,
    pub remote_url: String,
    pub branch: String,
    pub relative_path: String,
    pub agent_kind: String,
    pub title: Option<String>,
    /// Run in the terminal each time one is spawned for this session,
    /// so the agent CLI can rehydrate its own context. `None` means
    /// the terminal comes up as a bare shell.
    pub resume_command: Option<String>,
    pub status: SessionStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// What a caller supplies to register a new session. No `id` or
/// timestamps — the store assigns both. No `status` — every new
/// session starts `Active`.
#[derive(Clone, Debug)]
pub struct NewSession {
    pub project: String,
    pub owner_id: String,
    pub remote_url: String,
    pub branch: String,
    pub relative_path: String,
    pub agent_kind: Option<String>,
    pub title: Option<String>,
    /// Optional at creation because the command usually is not known
    /// yet — an agent CLI names its own session id only once it has
    /// started. Set it afterwards with `set_resume_command`.
    pub resume_command: Option<String>,
}
