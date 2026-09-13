//! Every SQL statement agent memory runs.
//!
//! The two buckets get **separate methods**, never one method with a
//! scope argument, so that personal access
//! through "a distinct repository method with the owner filter baked
//! in", enforced "not on the caller's honour". A single
//! `get(scope, ..)` would put the filter back in the caller's hands,
//! and the day one call site passes the wrong scope, personal memory
//! leaks. There is deliberately no method that reads both.

use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::{Row, SqlitePool};
use ulid::Ulid;

use crate::internal::domain::{MemoryEntry, MemoryError, MemoryScope, validate_key};

#[derive(Clone)]
pub struct MemoryStore {
    pool: SqlitePool,
}

impl MemoryStore {
    #[must_use]
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    // ---- Type 1: project memory (shareable) -------------------------

    /// Writes, or overwrites, one project-scoped memory.
    ///
    /// # Errors
    /// `EmptyKey`/`EmptyProject` on blank input, `Storage` on any SQL
    /// failure.
    pub async fn remember_project(
        &self,
        project: &str,
        key: &str,
        value: &Value,
    ) -> Result<MemoryEntry, MemoryError> {
        validate_key(key)?;
        if project.trim().is_empty() {
            return Err(MemoryError::EmptyProject);
        }
        let now = Utc::now();
        sqlx::query(
            "INSERT INTO agent_memory \
             (id, state_type, project, owner_id, key, value, created_at, updated_at) \
             VALUES (?, 'project', ?, NULL, ?, ?, ?, ?) \
             ON CONFLICT (project, key) WHERE state_type = 'project' \
             DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
        )
        .bind(Ulid::new().to_string())
        .bind(project)
        .bind(key)
        .bind(value.to_string())
        .bind(now.to_rfc3339())
        .bind(now.to_rfc3339())
        .execute(&self.pool)
        .await?;

        self.recall_project(project, key).await
    }

    /// # Errors
    /// `NotFound` if nothing is stored under that key for that
    /// project, `Corrupt` if the stored value stopped being JSON,
    /// `Storage` on any SQL failure.
    pub async fn recall_project(
        &self,
        project: &str,
        key: &str,
    ) -> Result<MemoryEntry, MemoryError> {
        let row = sqlx::query(
            "SELECT id, state_type, project, owner_id, key, value, created_at, updated_at \
             FROM agent_memory WHERE state_type = 'project' AND project = ? AND key = ?",
        )
        .bind(project)
        .bind(key)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(MemoryError::NotFound)?;
        entry_from_row(&row)
    }

    /// # Errors
    /// `Corrupt` if a stored value stopped being JSON, `Storage` on
    /// any SQL failure.
    pub async fn list_project(&self, project: &str) -> Result<Vec<MemoryEntry>, MemoryError> {
        let rows = sqlx::query(
            "SELECT id, state_type, project, owner_id, key, value, created_at, updated_at \
             FROM agent_memory WHERE state_type = 'project' AND project = ? ORDER BY key",
        )
        .bind(project)
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(entry_from_row).collect()
    }

    /// # Errors
    /// `NotFound` if there was nothing to forget, `Storage` on any
    /// SQL failure.
    pub async fn forget_project(&self, project: &str, key: &str) -> Result<(), MemoryError> {
        let result = sqlx::query(
            "DELETE FROM agent_memory WHERE state_type = 'project' AND project = ? AND key = ?",
        )
        .bind(project)
        .bind(key)
        .execute(&self.pool)
        .await?;
        if result.rows_affected() == 0 {
            return Err(MemoryError::NotFound);
        }
        Ok(())
    }

    // ---- Type 2: personal memory (private to its owner) -------------

    /// Writes, or overwrites, one personal memory. Keyed by owner
    /// alone: personal state belongs with "one developer working on
    /// **any** project", so this deliberately has no project scope.
    ///
    /// # Errors
    /// `EmptyKey`/`EmptyOwner` on blank input, `Storage` on any SQL
    /// failure.
    pub async fn remember_personal(
        &self,
        owner_id: &str,
        key: &str,
        value: &Value,
    ) -> Result<MemoryEntry, MemoryError> {
        validate_key(key)?;
        if owner_id.trim().is_empty() {
            return Err(MemoryError::EmptyOwner);
        }
        let now = Utc::now();
        sqlx::query(
            "INSERT INTO agent_memory \
             (id, state_type, project, owner_id, key, value, created_at, updated_at) \
             VALUES (?, 'personal', NULL, ?, ?, ?, ?, ?) \
             ON CONFLICT (owner_id, key) WHERE state_type = 'personal' \
             DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
        )
        .bind(Ulid::new().to_string())
        .bind(owner_id)
        .bind(key)
        .bind(value.to_string())
        .bind(now.to_rfc3339())
        .bind(now.to_rfc3339())
        .execute(&self.pool)
        .await?;

        self.recall_personal(owner_id, key).await
    }

    /// # Errors
    /// `NotFound` if nothing is stored under that key for that owner,
    /// `Corrupt` if the stored value stopped being JSON, `Storage` on
    /// any SQL failure.
    pub async fn recall_personal(
        &self,
        owner_id: &str,
        key: &str,
    ) -> Result<MemoryEntry, MemoryError> {
        let row = sqlx::query(
            "SELECT id, state_type, project, owner_id, key, value, created_at, updated_at \
             FROM agent_memory WHERE state_type = 'personal' AND owner_id = ? AND key = ?",
        )
        .bind(owner_id)
        .bind(key)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(MemoryError::NotFound)?;
        entry_from_row(&row)
    }

    /// # Errors
    /// `Corrupt` if a stored value stopped being JSON, `Storage` on
    /// any SQL failure.
    pub async fn list_personal(&self, owner_id: &str) -> Result<Vec<MemoryEntry>, MemoryError> {
        let rows = sqlx::query(
            "SELECT id, state_type, project, owner_id, key, value, created_at, updated_at \
             FROM agent_memory WHERE state_type = 'personal' AND owner_id = ? ORDER BY key",
        )
        .bind(owner_id)
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(entry_from_row).collect()
    }

    /// # Errors
    /// `NotFound` if there was nothing to forget, `Storage` on any
    /// SQL failure.
    pub async fn forget_personal(&self, owner_id: &str, key: &str) -> Result<(), MemoryError> {
        let result = sqlx::query(
            "DELETE FROM agent_memory WHERE state_type = 'personal' AND owner_id = ? AND key = ?",
        )
        .bind(owner_id)
        .bind(key)
        .execute(&self.pool)
        .await?;
        if result.rows_affected() == 0 {
            return Err(MemoryError::NotFound);
        }
        Ok(())
    }
}

// ---- replication -----------------------------------------
//
// Last-write-wins here keys on the *logical* key — `(project, key)` or
// `(owner_id, key)` — not on the row id, which is where this differs
// from session sync on purpose. Two machines can each remember the
// same key without ever having seen each other's row, so they hold
// different ids for the same fact; matching on id would treat them as
// unrelated and hit the unique index instead of merging. "The newest
// write to this key wins" is the semantics the data actually has.

impl MemoryStore {
    /// Every project-scoped entry, optionally only those touched since
    /// `since`. Type 1 replicates freely, so this is not
    /// filtered by caller.
    ///
    /// # Errors
    /// `Corrupt` on an unreadable stored value, `Storage` on any SQL
    /// failure.
    pub async fn list_project_since(
        &self,
        since: Option<DateTime<Utc>>,
    ) -> Result<Vec<MemoryEntry>, MemoryError> {
        self.list_since_inner("project", None, since).await
    }

    /// Every entry belonging to `owner_id`. Type 2 replicates as
    /// private-to-owner, so the filter is not optional and
    /// there is no variant of this without it.
    ///
    /// # Errors
    /// `Corrupt` on an unreadable stored value, `Storage` on any SQL
    /// failure.
    pub async fn list_personal_since(
        &self,
        owner_id: &str,
        since: Option<DateTime<Utc>>,
    ) -> Result<Vec<MemoryEntry>, MemoryError> {
        self.list_since_inner("personal", Some(owner_id), since)
            .await
    }

    async fn list_since_inner(
        &self,
        state_type: &str,
        owner_id: Option<&str>,
        since: Option<DateTime<Utc>>,
    ) -> Result<Vec<MemoryEntry>, MemoryError> {
        let mut sql = String::from(
            "SELECT id, state_type, project, owner_id, key, value, created_at, updated_at \
             FROM agent_memory WHERE state_type = ?",
        );
        if owner_id.is_some() {
            sql.push_str(" AND owner_id = ?");
        }
        if since.is_some() {
            sql.push_str(" AND updated_at > ?");
        }
        sql.push_str(" ORDER BY updated_at ASC");

        let mut q = sqlx::query(&sql).bind(state_type);
        if let Some(owner) = owner_id {
            q = q.bind(owner);
        }
        if let Some(cursor) = since {
            q = q.bind(cursor.to_rfc3339());
        }
        let rows = q.fetch_all(&self.pool).await?;
        rows.iter().map(entry_from_row).collect()
    }

    /// Applies an incoming project entry, newest write winning.
    ///
    /// # Errors
    /// `EmptyKey`/`EmptyProject` if the incoming row is malformed,
    /// `Storage` on any SQL failure.
    /// Applies a row that arrived over sync.
    ///
    /// Returns the authoritative row and **whether it actually moved**.
    /// The caller needs the second half: a sync push that changed
    /// nothing must not announce a change, or a `live` subscriber
    /// answers the announcement with a pass, that pass pushes, the push
    /// announces again, and two machines spin at the speed of the
    /// network with nobody having done anything.
    pub async fn upsert_project_for_sync(
        &self,
        incoming: &MemoryEntry,
    ) -> Result<(MemoryEntry, bool), MemoryError> {
        let project = incoming
            .project
            .as_deref()
            .ok_or(MemoryError::EmptyProject)?;
        validate_key(&incoming.key)?;

        match self.recall_project(project, &incoming.key).await {
            Ok(existing) if incoming.updated_at <= existing.updated_at => {
                return Ok((existing, false));
            }
            Ok(_) | Err(MemoryError::NotFound) => {}
            Err(other) => return Err(other),
        }
        self.remember_project(project, &incoming.key, &incoming.value)
            .await
            .map(|e| (e, true))
    }

    /// Applies an incoming personal entry under `owner_id`, newest
    /// write winning.
    ///
    /// `owner_id` comes from the authenticated caller and the incoming
    /// row's own `owner_id` is ignored outright — a client cannot even
    /// attempt to write into someone else's personal memory, rather
    /// than attempting it and being refused.
    ///
    /// # Errors
    /// `EmptyKey`/`EmptyOwner` if the incoming row is malformed,
    /// `Storage` on any SQL failure.
    /// Applies a row that arrived over sync.
    ///
    /// Returns the authoritative row and **whether it actually moved**.
    /// The caller needs the second half: a sync push that changed
    /// nothing must not announce a change, or a `live` subscriber
    /// answers the announcement with a pass, that pass pushes, the push
    /// announces again, and two machines spin at the speed of the
    /// network with nobody having done anything.
    pub async fn upsert_personal_for_sync(
        &self,
        incoming: &MemoryEntry,
        owner_id: &str,
    ) -> Result<(MemoryEntry, bool), MemoryError> {
        validate_key(&incoming.key)?;

        match self.recall_personal(owner_id, &incoming.key).await {
            Ok(existing) if incoming.updated_at <= existing.updated_at => {
                return Ok((existing, false));
            }
            Ok(_) | Err(MemoryError::NotFound) => {}
            Err(other) => return Err(other),
        }
        self.remember_personal(owner_id, &incoming.key, &incoming.value)
            .await
            .map(|e| (e, true))
    }
}

fn entry_from_row(row: &sqlx::sqlite::SqliteRow) -> Result<MemoryEntry, MemoryError> {
    let raw_scope: String = row.get("state_type");
    let scope = MemoryScope::parse(&raw_scope)
        .ok_or_else(|| MemoryError::Corrupt(format!("unknown state_type {raw_scope:?}")))?;
    let raw_value: String = row.get("value");
    let value =
        serde_json::from_str(&raw_value).map_err(|e| MemoryError::Corrupt(e.to_string()))?;

    Ok(MemoryEntry {
        id: row.get("id"),
        scope,
        project: row.get("project"),
        owner_id: row.get("owner_id"),
        key: row.get("key"),
        value,
        created_at: parse_ts(&row.get::<String, _>("created_at"))?,
        updated_at: parse_ts(&row.get::<String, _>("updated_at"))?,
    })
}

fn parse_ts(s: &str) -> Result<DateTime<Utc>, MemoryError> {
    DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|e| MemoryError::Corrupt(format!("stored timestamp unparsable: {e}")))
}
