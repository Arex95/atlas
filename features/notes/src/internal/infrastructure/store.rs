//! Storage for personal notes.
//!
//! **Every method takes an owner, and there is no variant that does
//! not.** That is what makes Type 2 classification hold
//! here: not a filter each query must remember, but the only shape a
//! call can have. A note belonging to somebody else is reported as
//! not found, the same rule sessions follow — distinguishing it would
//! answer whether a name exists in another developer's notes.

use chrono::{DateTime, Utc};
use sqlx::{Row, SqlitePool};
use ulid::Ulid;

use crate::internal::domain::{MAX_NAME_LEN, Note, NotesError};

#[derive(Clone)]
pub struct NoteStore {
    pool: SqlitePool,
}

impl NoteStore {
    #[must_use]
    pub const fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Writes a note, replacing whatever was under that name.
    ///
    /// Addressed by name rather than by id so that writing is
    /// idempotent: the same call twice leaves one note, and a terminal
    /// can reach one without having copied an id around. `created_at`
    /// survives a rewrite — the note is the same note.
    ///
    /// # Errors
    /// `EmptyName`, `NameTooLong` or `EmptyOwnerId` on bad input,
    /// `Storage` on any SQL failure.
    pub async fn write(&self, owner_id: &str, name: &str, body: &str) -> Result<Note, NotesError> {
        let name = validate_name(name)?;
        if owner_id.trim().is_empty() {
            return Err(NotesError::EmptyOwnerId);
        }

        let now = Utc::now();
        sqlx::query(
            "INSERT INTO notes (id, owner_id, name, body, created_at, updated_at) \
             VALUES (?, ?, ?, ?, ?, ?) \
             ON CONFLICT(owner_id, name) DO UPDATE SET \
               body = excluded.body, updated_at = excluded.updated_at",
        )
        .bind(Ulid::new().to_string())
        .bind(owner_id)
        .bind(&name)
        .bind(body)
        .bind(now.to_rfc3339())
        .bind(now.to_rfc3339())
        .execute(&self.pool)
        .await?;

        // Read back rather than assembling the return value: the row
        // that was written is not necessarily the one that arrived —
        // `created_at` comes from the original insert on a rewrite.
        self.read(owner_id, &name).await
    }

    /// # Errors
    /// `NotFound` if this owner has no note by that name, `Storage` on
    /// any SQL failure.
    pub async fn read(&self, owner_id: &str, name: &str) -> Result<Note, NotesError> {
        let row = sqlx::query(
            "SELECT id, owner_id, name, body, created_at, updated_at \
             FROM notes WHERE owner_id = ? AND name = ?",
        )
        .bind(owner_id)
        .bind(name.trim())
        .fetch_optional(&self.pool)
        .await?;

        row.map(|r| note_from_row(&r))
            .transpose()?
            .ok_or(NotesError::NotFound)
    }

    /// Every note this owner has, most recently written first.
    ///
    /// Bodies included: a developer's notes are small, and a listing
    /// that omitted them would mean a second call per note to read
    /// anything.
    ///
    /// # Errors
    /// `Storage` on any SQL failure.
    pub async fn list(&self, owner_id: &str) -> Result<Vec<Note>, NotesError> {
        let rows = sqlx::query(
            "SELECT id, owner_id, name, body, created_at, updated_at \
             FROM notes WHERE owner_id = ? ORDER BY updated_at DESC",
        )
        .bind(owner_id)
        .fetch_all(&self.pool)
        .await?;

        rows.iter().map(note_from_row).collect()
    }

    /// # Errors
    /// `NotFound` if this owner has no note by that name — deleting
    /// something that was not there is reported rather than silently
    /// succeeding, because the usual cause is a typo in the name.
    pub async fn delete(&self, owner_id: &str, name: &str) -> Result<(), NotesError> {
        let deleted = sqlx::query("DELETE FROM notes WHERE owner_id = ? AND name = ?")
            .bind(owner_id)
            .bind(name.trim())
            .execute(&self.pool)
            .await?
            .rows_affected();

        if deleted == 0 {
            return Err(NotesError::NotFound);
        }
        Ok(())
    }

    /// Owner-scoped, cursor-paged read for the sync engine: every note
    /// owned by `owner_id` with `updated_at` strictly after `since`,
    /// oldest first. `None` returns everything the owner has.
    ///
    /// # Errors
    /// `Storage` on any SQL failure.
    pub async fn list_since(
        &self,
        owner_id: &str,
        since: Option<DateTime<Utc>>,
    ) -> Result<Vec<Note>, NotesError> {
        let mut sql = String::from(
            "SELECT id, owner_id, name, body, created_at, updated_at \
             FROM notes WHERE owner_id = ?",
        );
        if since.is_some() {
            sql.push_str(" AND updated_at > ?");
        }
        sql.push_str(" ORDER BY updated_at ASC");

        let mut query = sqlx::query(&sql).bind(owner_id);
        if let Some(cursor) = since {
            query = query.bind(cursor.to_rfc3339());
        }
        let rows = query.fetch_all(&self.pool).await?;
        rows.iter().map(note_from_row).collect()
    }

    /// Applies a note arriving from another machine, under the
    /// authenticated owner rather than the one the row claims.
    ///
    /// A client that lies about `owner_id` writes into its own notes,
    /// so there is no path here that touches another developer's —
    /// the same rule sessions and memory follow.
    ///
    /// Last-write-wins on `updated_at`, compared against the note with
    /// the same *name* rather than the same id: two machines that
    /// wrote the same note independently produced different ids for
    /// what the developer considers one note.
    ///
    /// # Errors
    /// `EmptyName` or `NameTooLong` on bad input, `Storage` on any SQL
    /// failure.
    /// Applies a row that arrived over sync.
    ///
    /// Returns the authoritative row and **whether it actually moved**.
    /// The caller needs the second half: a sync push that changed
    /// nothing must not announce a change, or a `live` subscriber
    /// answers the announcement with a pass, that pass pushes, the push
    /// announces again, and two machines spin at the speed of the
    /// network with nobody having done anything.
    pub async fn upsert_for_sync(
        &self,
        incoming: Note,
        owner_id: &str,
    ) -> Result<(Note, bool), NotesError> {
        let name = validate_name(&incoming.name)?;

        if let Ok(existing) = self.read(owner_id, &name).await
            && incoming.updated_at <= existing.updated_at
        {
            return Ok((existing, false));
        }

        sqlx::query(
            "INSERT INTO notes (id, owner_id, name, body, created_at, updated_at) \
             VALUES (?, ?, ?, ?, ?, ?) \
             ON CONFLICT(owner_id, name) DO UPDATE SET \
               body = excluded.body, updated_at = excluded.updated_at",
        )
        .bind(&incoming.id)
        .bind(owner_id)
        .bind(&name)
        .bind(&incoming.body)
        .bind(incoming.created_at.to_rfc3339())
        .bind(incoming.updated_at.to_rfc3339())
        .execute(&self.pool)
        .await?;

        // Written, so it moved.
        self.read(owner_id, &name).await.map(|n| (n, true))
    }
}

fn validate_name(name: &str) -> Result<String, NotesError> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(NotesError::EmptyName);
    }
    // Counted in characters, not bytes: a name of accented text is not
    // longer than the same name in ASCII to the person who typed it.
    if trimmed.chars().count() > MAX_NAME_LEN {
        return Err(NotesError::NameTooLong(MAX_NAME_LEN));
    }
    Ok(trimmed.to_owned())
}

fn note_from_row(row: &sqlx::sqlite::SqliteRow) -> Result<Note, NotesError> {
    Ok(Note {
        id: row.get("id"),
        owner_id: row.get("owner_id"),
        name: row.get("name"),
        body: row.get("body"),
        created_at: parse_ts(&row.get::<String, _>("created_at"))?,
        updated_at: parse_ts(&row.get::<String, _>("updated_at"))?,
    })
}

fn parse_ts(raw: &str) -> Result<DateTime<Utc>, NotesError> {
    DateTime::parse_from_rfc3339(raw)
        .map(|t| t.with_timezone(&Utc))
        .map_err(|e| NotesError::Storage(format!("stored timestamp {raw:?} unparsable: {e}")))
}
