use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// One note, belonging to exactly one developer.
///
/// `owner_id` is on the struct rather than implied by the query that
/// fetched it, so a row that travels — to a sync client, to a
/// response body — carries whose it is with it.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Note {
    pub id: String,
    pub owner_id: String,
    /// What the developer addresses it by. Unique per owner.
    pub name: String,
    pub body: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
