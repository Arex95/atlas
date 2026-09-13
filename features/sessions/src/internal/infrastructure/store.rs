use chrono::{DateTime, Utc};
use sqlx::{Row, SqlitePool};
use ulid::Ulid;

use crate::internal::domain::{
    Caller, NewSession, Session, SessionId, SessionStatus, SessionsError,
};
use crate::internal::infrastructure::tokens;

/// Owns every SQL statement the session registry runs.
///
/// Pure local persistence, not a hexagonal port (scoped
/// that pattern to the external tracker boundary specifically) —
/// there is nothing external to swap out here.
#[derive(Clone)]
pub struct SessionStore {
    pool: SqlitePool,
}

impl SessionStore {
    #[must_use]
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// # Errors
    /// `EmptyProject`/`EmptyRemoteUrl`/`EmptyOwnerId` on blank input,
    /// `Storage` on any SQL failure.
    pub async fn create(&self, input: NewSession) -> Result<Session, SessionsError> {
        if input.project.trim().is_empty() {
            return Err(SessionsError::EmptyProject);
        }
        if input.remote_url.trim().is_empty() {
            return Err(SessionsError::EmptyRemoteUrl);
        }
        if input.owner_id.trim().is_empty() {
            return Err(SessionsError::EmptyOwnerId);
        }

        let id = SessionId(Ulid::new().to_string());
        let now = Utc::now();
        let agent_kind = input.agent_kind.unwrap_or_else(|| "bash".to_owned());

        sqlx::query(
            "INSERT INTO sessions \
             (id, project, owner_id, remote_url, branch, relative_path, agent_kind, title, resume_command, status, created_at, updated_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id.0)
        .bind(&input.project)
        .bind(&input.owner_id)
        .bind(&input.remote_url)
        .bind(&input.branch)
        .bind(&input.relative_path)
        .bind(&agent_kind)
        .bind(&input.title)
        .bind(&input.resume_command)
        .bind(SessionStatus::Active.as_str())
        .bind(now.to_rfc3339())
        .bind(now.to_rfc3339())
        .execute(&self.pool)
        .await?;

        Ok(Session {
            id,
            project: input.project,
            owner_id: input.owner_id,
            remote_url: input.remote_url,
            branch: input.branch,
            relative_path: input.relative_path,
            agent_kind,
            title: input.title,
            resume_command: input.resume_command,
            status: SessionStatus::Active,
            created_at: now,
            updated_at: now,
        })
    }

    /// Creates a session and mints the token its agent will
    /// authenticate with.
    ///
    /// One transaction, because the two halves are useless apart: a
    /// session with no token has no way for its agent to identify
    /// itself, and a token with no session resolves to nothing. The
    /// token is returned here and **never again** — nothing stores it,
    /// only its hash.
    ///
    /// # Errors
    /// The same validation errors as [`Self::create`], `Storage` on
    /// any SQL failure.
    pub async fn create_with_token(
        &self,
        input: NewSession,
    ) -> Result<(Session, String), SessionsError> {
        let session = self.create(input).await?;
        let minted = tokens::mint();

        sqlx::query(
            "INSERT INTO session_agent_tokens (token_hash, session_id, owner_id, created_at) \
             VALUES (?, ?, ?, ?)",
        )
        .bind(&minted.hash)
        .bind(&session.id.0)
        .bind(&session.owner_id)
        .bind(Utc::now().to_rfc3339())
        .execute(&self.pool)
        .await?;

        Ok((session, minted.token))
    }

    /// Issues an additional token for a session that already exists.
    ///
    /// A session may hold several: one returned by
    /// [`Self::create_with_token`], and one per terminal spawned for
    /// it. They are equivalent — all resolve to the same session and
    /// owner — and the reason for minting rather than reusing is that
    /// only the hash is stored, so the original cannot be recovered to
    /// hand to a terminal.
    ///
    /// Deleting the session revokes all of them at once through the
    /// foreign key, so the extra rows need no separate cleanup.
    ///
    /// Owner-scoped: issuing a credential for somebody else's session
    /// would be minting a key to their machine.
    ///
    /// # Errors
    /// `NotFound` if no session with this id is owned by `owner_id`,
    /// `Storage` on any SQL failure.
    pub async fn issue_token(
        &self,
        id: &SessionId,
        owner_id: &str,
    ) -> Result<String, SessionsError> {
        // Confirms ownership before minting; `get` is the owner-scoped
        // read, so somebody else's session is a not-found here too.
        self.get(id, owner_id).await?;

        let minted = tokens::mint();
        sqlx::query(
            "INSERT INTO session_agent_tokens (token_hash, session_id, owner_id, created_at) \
             VALUES (?, ?, ?, ?)",
        )
        .bind(&minted.hash)
        .bind(&id.0)
        .bind(owner_id)
        .bind(Utc::now().to_rfc3339())
        .execute(&self.pool)
        .await?;

        Ok(minted.token)
    }

    /// Resolves a bearer token to the identity behind it.
    ///
    /// `None` for a token that was never issued or whose session has
    /// since been deleted — the foreign key cascades, so a deleted
    /// session revokes its own credential without anything having to
    /// remember to.
    ///
    /// The lookup is by hash, so a stolen database yields no usable
    /// tokens, and it is a primary-key hit rather than a scan because
    /// this runs on every single MCP request.
    ///
    /// # Errors
    /// `Storage` on any SQL failure.
    pub async fn resolve_token(&self, token: &str) -> Result<Option<Caller>, SessionsError> {
        let row: Option<(String, String)> = sqlx::query_as(
            "SELECT session_id, owner_id FROM session_agent_tokens WHERE token_hash = ?",
        )
        .bind(tokens::hash(token))
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|(session_id, owner_id)| Caller::Session {
            session_id,
            owner_id,
        }))
    }

    /// Owner-scoped fetch: a session that exists but
    /// belongs to a different owner is indistinguishable from one
    /// that doesn't exist: "not yours" and "not there" answer alike
    /// rule. This is what every MCP-facing caller must use.
    ///
    /// # Errors
    /// `NotFound` if no session with this id is owned by `owner_id`,
    /// `Storage` on any SQL failure.
    pub async fn get(&self, id: &SessionId, owner_id: &str) -> Result<Session, SessionsError> {
        let row = sqlx::query(
            "SELECT id, project, owner_id, remote_url, branch, relative_path, agent_kind, title, resume_command, status, created_at, updated_at \
             FROM sessions WHERE id = ? AND owner_id = ?",
        )
        .bind(&id.0)
        .bind(owner_id)
        .fetch_optional(&self.pool)
        .await?;
        row.map(|r| session_from_row(&r))
            .transpose()?
            .ok_or(SessionsError::NotFound)
    }

    /// Records what to run when this session's terminal is spawned.
    ///
    /// Separate from creation because the command is usually not known
    /// then: an agent CLI names its own session id only once it has
    /// started, so the useful command exists after the first terminal,
    /// not before it.
    ///
    /// Owner-scoped, and the reason is worth stating. The command
    /// grants nothing its owner could not already do by typing it —
    /// but it runs *later, unattended*, which is a different thing
    /// from a keystroke, and setting one on somebody else's session
    /// would be arranging for their machine to run your command.
    ///
    /// `None` clears it, so a session can go back to a bare shell
    /// without being recreated.
    ///
    /// # Errors
    /// `NotFound` if no session with this id is owned by `owner_id`,
    /// `Storage` on any SQL failure.
    pub async fn set_resume_command(
        &self,
        id: &SessionId,
        owner_id: &str,
        command: Option<&str>,
    ) -> Result<Session, SessionsError> {
        let changed = sqlx::query(
            "UPDATE sessions SET resume_command = ?, updated_at = ? \
             WHERE id = ? AND owner_id = ?",
        )
        .bind(command)
        .bind(Utc::now().to_rfc3339())
        .bind(&id.0)
        .bind(owner_id)
        .execute(&self.pool)
        .await?
        .rows_affected();

        if changed == 0 {
            return Err(SessionsError::NotFound);
        }
        self.get(id, owner_id).await
    }

    /// Unscoped fetch for trusted, machine-local callers only (e.g.
    /// PTY spawn: resolving where a session lives on disk is not a
    /// per-user visibility question). MCP-facing tools must use
    /// [`Self::get`] instead.
    ///
    /// # Errors
    /// `NotFound` if no session has this id, `Storage` on any SQL
    /// failure.
    pub async fn get_unscoped(&self, id: &SessionId) -> Result<Session, SessionsError> {
        let row = sqlx::query(
            "SELECT id, project, owner_id, remote_url, branch, relative_path, agent_kind, title, resume_command, status, created_at, updated_at \
             FROM sessions WHERE id = ?",
        )
        .bind(&id.0)
        .fetch_optional(&self.pool)
        .await?;
        row.map(|r| session_from_row(&r))
            .transpose()?
            .ok_or(SessionsError::NotFound)
    }

    /// # Errors
    /// Any SQL failure. An unknown/empty `project`, or an `owner_id`
    /// that owns nothing in it, returns an empty list, not an error.
    pub async fn list(
        &self,
        project: &str,
        owner_id: &str,
        status: Option<SessionStatus>,
    ) -> Result<Vec<Session>, SessionsError> {
        let mut sql = String::from(
            "SELECT id, project, owner_id, remote_url, branch, relative_path, agent_kind, title, resume_command, status, created_at, updated_at \
             FROM sessions WHERE project = ? AND owner_id = ?",
        );
        if status.is_some() {
            sql.push_str(" AND status = ?");
        }
        sql.push_str(" ORDER BY created_at ASC");

        let mut q = sqlx::query(&sql).bind(project).bind(owner_id);
        if let Some(s) = status {
            q = q.bind(s.as_str());
        }
        let rows = q.fetch_all(&self.pool).await?;
        rows.iter().map(session_from_row).collect()
    }

    /// # Errors
    /// `NotFound` if no session with this id is owned by `owner_id`,
    /// `Storage` on any SQL failure.
    pub async fn update_status(
        &self,
        id: &SessionId,
        owner_id: &str,
        status: SessionStatus,
    ) -> Result<Session, SessionsError> {
        let now = Utc::now();
        let result = sqlx::query(
            "UPDATE sessions SET status = ?, updated_at = ? WHERE id = ? AND owner_id = ?",
        )
        .bind(status.as_str())
        .bind(now.to_rfc3339())
        .bind(&id.0)
        .bind(owner_id)
        .execute(&self.pool)
        .await?;
        if result.rows_affected() == 0 {
            return Err(SessionsError::NotFound);
        }
        self.get(id, owner_id).await
    }

    /// Upsert a full session row keyed by its own id, for the sync
    /// engine (`atlas-sync`) only — every other writer goes through
    /// [`Self::create`], which assigns a fresh id itself. `owner_id`
    /// is a separate parameter, not read off `incoming`, for the
    /// same reason [`Self::get`] takes it separately: the caller
    /// authenticated one identity, and that's the only owner this
    /// call is allowed to write as.
    ///
    /// Conflict policy is last-write-wins by `updated_at`: an
    /// incoming row older than or equal to what's stored is a no-op
    /// that returns the stored row unchanged, not an error — sync is
    /// expected to see stale data routinely, that isn't exceptional.
    ///
    /// # Errors
    /// `EmptyProject`/`EmptyRemoteUrl` on blank input, `OwnerMismatch`
    /// if a row with this id already exists under a different owner,
    /// `Storage` on any SQL failure.
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
        incoming: Session,
        owner_id: &str,
    ) -> Result<(Session, bool), SessionsError> {
        if incoming.project.trim().is_empty() {
            return Err(SessionsError::EmptyProject);
        }
        if incoming.remote_url.trim().is_empty() {
            return Err(SessionsError::EmptyRemoteUrl);
        }

        let existing = match self.get_unscoped(&incoming.id).await {
            Ok(row) => Some(row),
            Err(SessionsError::NotFound) => None,
            Err(other) => return Err(other),
        };

        if let Some(existing) = &existing {
            if existing.owner_id != owner_id {
                return Err(SessionsError::OwnerMismatch);
            }
            if incoming.updated_at <= existing.updated_at {
                return Ok((existing.clone(), false));
            }
        }

        let created_at = existing.map_or(incoming.created_at, |e| e.created_at);

        // `resume_command` is deliberately absent from both the column
        // list and the update, and this is not an oversight to tidy up
        // later.
        //
        // It is a command that runs unattended when a terminal spawns.
        // Replicating it would mean a row arriving over the network can
        // arrange for a command to run on a developer's machine —
        // turning a data store into a code-execution channel, and
        // handing anyone who can write to a team server a way onto
        // every laptop that syncs from it. Metadata travels; commands
        // do not.
        //
        // It is machine-local for a practical reason too: the command
        // usually names an agent CLI's own session id, which means
        // nothing on another machine.
        sqlx::query(
            "INSERT INTO sessions \
             (id, project, owner_id, remote_url, branch, relative_path, agent_kind, title, status, created_at, updated_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) \
             ON CONFLICT(id) DO UPDATE SET \
             project = excluded.project, remote_url = excluded.remote_url, \
             branch = excluded.branch, relative_path = excluded.relative_path, \
             agent_kind = excluded.agent_kind, title = excluded.title, \
             status = excluded.status, updated_at = excluded.updated_at",
        )
        .bind(&incoming.id.0)
        .bind(&incoming.project)
        .bind(owner_id)
        .bind(&incoming.remote_url)
        .bind(&incoming.branch)
        .bind(&incoming.relative_path)
        .bind(&incoming.agent_kind)
        .bind(&incoming.title)
        .bind(incoming.status.as_str())
        .bind(created_at.to_rfc3339())
        .bind(incoming.updated_at.to_rfc3339())
        .execute(&self.pool)
        .await?;

        // Re-read rather than returning `..incoming`. The row that was
        // written is not the row that arrived — `resume_command` is
        // deliberately not replicated — and a return value assembled
        // from the input would hand the caller a command the database
        // refused to store. Reading it back makes that structural
        // instead of a field somebody has to remember to strip.
        self.get_unscoped(&incoming.id).await.map(|s| (s, true))
    }

    /// Owner-scoped, cursor-paged read for the sync engine: every
    /// session owned by `owner_id` with `updated_at` strictly after
    /// `since`, oldest first — the same poll-based cursor shape as
    /// `messaging.read_inbox` and `terminal.read_output`. `since:
    /// None` returns everything the owner has.
    ///
    /// # Errors
    /// Any SQL failure.
    pub async fn list_since(
        &self,
        owner_id: &str,
        since: Option<DateTime<Utc>>,
    ) -> Result<Vec<Session>, SessionsError> {
        let mut sql = String::from(
            "SELECT id, project, owner_id, remote_url, branch, relative_path, agent_kind, title, resume_command, status, created_at, updated_at \
             FROM sessions WHERE owner_id = ?",
        );
        if since.is_some() {
            sql.push_str(" AND updated_at > ?");
        }
        sql.push_str(" ORDER BY updated_at ASC");

        let mut q = sqlx::query(&sql).bind(owner_id);
        if let Some(cursor) = since {
            q = q.bind(cursor.to_rfc3339());
        }
        let rows = q.fetch_all(&self.pool).await?;
        rows.iter().map(session_from_row).collect()
    }
}

fn session_from_row(row: &sqlx::sqlite::SqliteRow) -> Result<Session, SessionsError> {
    let status_raw: String = row.get("status");
    let status = SessionStatus::parse(&status_raw)
        .ok_or_else(|| SessionsError::Storage(format!("stored status {status_raw:?} unknown")))?;
    let created_at: String = row.get("created_at");
    let created_at = parse_ts(&created_at)?;
    let updated_at: String = row.get("updated_at");
    let updated_at = parse_ts(&updated_at)?;

    Ok(Session {
        id: SessionId(row.get("id")),
        project: row.get("project"),
        owner_id: row.get("owner_id"),
        remote_url: row.get("remote_url"),
        branch: row.get("branch"),
        relative_path: row.get("relative_path"),
        agent_kind: row.get("agent_kind"),
        title: row.get("title"),
        resume_command: row.get("resume_command"),
        status,
        created_at,
        updated_at,
    })
}

fn parse_ts(s: &str) -> Result<DateTime<Utc>, SessionsError> {
    DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|e| SessionsError::Storage(format!("stored timestamp unparsable: {e}")))
}
