use chrono::{DateTime, Utc};
use sqlx::{Row, SqlitePool};
use ulid::Ulid;

use crate::internal::domain::{Message, MessageId, MessagingError, NewMessage};

/// Owns every SQL statement the messaging feature runs.
///
/// Pure local persistence, not a hexagonal port (scoped
/// that pattern to the external tracker boundary specifically) —
/// there is no vendor to swap out here.
#[derive(Clone)]
pub struct MessageStore {
    pool: SqlitePool,
}

/// Hard ceiling on `read_inbox`'s page size regardless of what a
/// caller asks for — an unbounded inbox read is a footgun waiting
/// for a busy project.
const MAX_LIMIT: u32 = 200;
const DEFAULT_LIMIT: u32 = 50;

impl MessageStore {
    #[must_use]
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// # Errors
    /// `EmptyProject`/`EmptyFromSession` on blank input, `Storage`
    /// on any SQL failure.
    pub async fn send(&self, project: &str, input: NewMessage) -> Result<Message, MessagingError> {
        if project.trim().is_empty() {
            return Err(MessagingError::EmptyProject);
        }
        if input.from_session.trim().is_empty() {
            return Err(MessagingError::EmptyFromSession);
        }

        let id = MessageId(Ulid::new().to_string());
        let created_at = Utc::now();
        let payload_json = serde_json::to_string(&input.payload)
            .map_err(|e| MessagingError::Storage(format!("payload not serializable: {e}")))?;

        sqlx::query(
            "INSERT INTO messages \
             (id, project, from_session, to_session, message_type, payload_json, correlation_id, reply_to, created_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id.0)
        .bind(project)
        .bind(&input.from_session)
        .bind(&input.to_session)
        .bind(&input.message_type)
        .bind(&payload_json)
        .bind(&input.correlation_id)
        .bind(&input.reply_to)
        .bind(created_at.to_rfc3339())
        .execute(&self.pool)
        .await?;

        Ok(Message {
            id,
            project: project.to_owned(),
            from_session: input.from_session,
            to_session: input.to_session,
            message_type: input.message_type,
            payload: input.payload,
            correlation_id: input.correlation_id,
            reply_to: input.reply_to,
            created_at,
        })
    }

    /// Every broadcast (`to_session IS NULL`) plus every direct
    /// message addressed to `for_session`, in `project`, oldest
    /// first. `since` (a previously-seen [`MessageId`]) excludes
    /// that message and everything before it — ULIDs sort
    /// lexicographically by creation time, so string comparison is
    /// enough, no timestamp math.
    ///
    /// # Errors
    /// Any SQL failure.
    pub async fn read_inbox(
        &self,
        project: &str,
        for_session: &str,
        since: Option<&MessageId>,
        limit: Option<u32>,
    ) -> Result<Vec<Message>, MessagingError> {
        let limit = limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT);

        let mut sql = String::from(
            "SELECT id, project, from_session, to_session, message_type, payload_json, \
             correlation_id, reply_to, created_at \
             FROM messages \
             WHERE project = ? AND (to_session IS NULL OR to_session = ?)",
        );
        if since.is_some() {
            sql.push_str(" AND id > ?");
        }
        sql.push_str(" ORDER BY id ASC LIMIT ?");

        let mut q = sqlx::query(&sql).bind(project).bind(for_session);
        if let Some(cursor) = since {
            q = q.bind(&cursor.0);
        }
        q = q.bind(i64::from(limit));

        let rows = q.fetch_all(&self.pool).await?;
        rows.iter().map(message_from_row).collect()
    }
}

fn message_from_row(row: &sqlx::sqlite::SqliteRow) -> Result<Message, MessagingError> {
    let payload_json: String = row.get("payload_json");
    let payload = serde_json::from_str(&payload_json)
        .map_err(|e| MessagingError::Storage(format!("stored payload not valid JSON: {e}")))?;
    let created_at: String = row.get("created_at");
    let created_at = DateTime::parse_from_rfc3339(&created_at)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|e| MessagingError::Storage(format!("stored created_at unparsable: {e}")))?;

    Ok(Message {
        id: MessageId(row.get("id")),
        project: row.get("project"),
        from_session: row.get("from_session"),
        to_session: row.get("to_session"),
        message_type: row.get("message_type"),
        payload,
        correlation_id: row.get("correlation_id"),
        reply_to: row.get("reply_to"),
        created_at,
    })
}
