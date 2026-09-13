use std::fmt;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A ULID (repo convention: sortable, not UUID). Doubles as the
/// ordering key for `read_inbox` — no separate sequence needed.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct MessageId(pub String);

impl fmt::Display for MessageId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// One coordination-protocol message.
///
/// `to_session: None` is a broadcast to every reader in `project`
/// (project state, shared); `Some` addresses one session directly
/// (Type 2, personal). `message_type` is a free string — this crate
/// does not define AFG's `task`/`task_result`/`spawn`/`close`
/// envelope conventions; those layer on top when AFG lands.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Message {
    pub id: MessageId,
    pub project: String,
    #[serde(rename = "from")]
    pub from_session: String,
    #[serde(rename = "to")]
    pub to_session: Option<String>,
    #[serde(rename = "type")]
    pub message_type: String,
    pub payload: serde_json::Value,
    pub correlation_id: Option<String>,
    pub reply_to: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// What a caller supplies to send a message. No `id` or
/// `created_at` — the store assigns both.
#[derive(Clone, Debug)]
pub struct NewMessage {
    pub from_session: String,
    pub to_session: Option<String>,
    pub message_type: String,
    pub payload: serde_json::Value,
    pub correlation_id: Option<String>,
    pub reply_to: Option<String>,
}
