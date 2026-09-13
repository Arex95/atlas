use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Which of buckets an entry lives in.
///
/// Deliberately not a field callers pass around freely: the store
/// exposes a separate method per bucket, and this exists so a *read*
/// result can say which one it came from, not so a write can choose
/// at runtime.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryScope {
    /// Type 1 — about a project, shareable with everyone on it.
    Project,
    /// Type 2 — about a person, private to them across every project.
    Personal,
}

impl MemoryScope {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Project => "project",
            Self::Personal => "personal",
        }
    }

    #[must_use]
    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "project" => Some(Self::Project),
            "personal" => Some(Self::Personal),
            _ => None,
        }
    }
}

/// One remembered key/value.
///
/// `project` and `owner_id` are mutually exclusive and the database
/// enforces it — see the CHECK in the migration. Which one is
/// populated follows from `scope`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MemoryEntry {
    pub id: String,
    pub scope: MemoryScope,
    pub project: Option<String>,
    pub owner_id: Option<String>,
    pub key: String,
    pub value: Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
