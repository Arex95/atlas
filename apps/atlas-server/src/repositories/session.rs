use crate::models::{AiSession, Project, ConversationTurn};
use sqlx::SqlitePool;

pub async fn find_project_by_slug(pool: &SqlitePool, slug: &str) -> sqlx::Result<Option<Project>> {
    sqlx::query_as::<_, Project>("SELECT * FROM projects WHERE slug = ?")
        .bind(slug)
        .fetch_optional(pool)
        .await
}

#[allow(clippy::too_many_arguments)]
pub async fn create(
    pool: &SqlitePool,
    id: &str,
    project_id: &str,
    provider: &str,
    model: &str,
    mode: &str,
    working_directory: &str,
    title: Option<&str>,
    author: &str,
) -> sqlx::Result<AiSession> {
    sqlx::query_as::<_, AiSession>(
        "INSERT INTO ai_sessions (id, project_id, provider, model, mode, working_directory, title, author, is_saved)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
         RETURNING *",
    )
    .bind(id)
    .bind(project_id)
    .bind(provider)
    .bind(model)
    .bind(mode)
    .bind(working_directory)
    .bind(title)
    .bind(author)
    .bind(0)
    .fetch_one(pool)
    .await
}

pub async fn save(
    pool: &SqlitePool,
    id: &str,
    custom_name: Option<&str>,
    custom_description: Option<&str>,
    color: Option<&str>,
    icon: Option<&str>,
) -> sqlx::Result<AiSession> {
    sqlx::query_as::<_, AiSession>(
        "UPDATE ai_sessions
         SET is_saved = 1,
             custom_name = ?,
             custom_description = ?,
             color = ?,
             icon = ?,
             last_activity_at = CURRENT_TIMESTAMP
         WHERE id = ?
         RETURNING *",
    )
    .bind(custom_name)
    .bind(custom_description)
    .bind(color)
    .bind(icon)
    .bind(id)
    .fetch_one(pool)
    .await
}

pub async fn find_messages(pool: &SqlitePool, session_id: &str) -> sqlx::Result<Vec<ConversationTurn>> {
    sqlx::query_as::<_, ConversationTurn>(
        "SELECT * FROM conversation_turns WHERE session_id = ? ORDER BY created_at ASC",
    )
    .bind(session_id)
    .fetch_all(pool)
    .await
}

pub async fn delete_message(pool: &SqlitePool, message_id: &str) -> sqlx::Result<bool> {
    let rows = sqlx::query("DELETE FROM conversation_turns WHERE id = ?")
        .bind(message_id)
        .execute(pool)
        .await?
        .rows_affected();
    Ok(rows > 0)
}

pub async fn find_history(pool: &SqlitePool, session_id: &str, limit: i64) -> sqlx::Result<Vec<ConversationTurn>> {
    sqlx::query_as::<_, ConversationTurn>(
        "SELECT * FROM conversation_turns WHERE session_id = ? ORDER BY created_at ASC LIMIT ?",
    )
    .bind(session_id)
    .bind(limit)
    .fetch_all(pool)
    .await
}

pub async fn create_message(
    pool: &SqlitePool,
    id: &str,
    session_id: &str,
    role: &str,
    content: &str,
) -> sqlx::Result<ConversationTurn> {
    sqlx::query_as::<_, ConversationTurn>(
        "INSERT INTO conversation_turns (id, session_id, role, content, created_at) VALUES (?, ?, ?, ?, STRFTIME('%Y-%m-%dT%H:%M:%fZ', 'now')) RETURNING *",
    )
    .bind(id)
    .bind(session_id)
    .bind(role)
    .bind(content)
    .fetch_one(pool)
    .await
}

pub async fn find_by_project_id(pool: &SqlitePool, project_id: &str) -> sqlx::Result<Vec<AiSession>> {
    sqlx::query_as::<_, AiSession>(
        "SELECT * FROM ai_sessions WHERE project_id = ? ORDER BY started_at DESC",
    )
    .bind(project_id)
    .fetch_all(pool)
    .await
}

pub async fn find_saved(pool: &SqlitePool) -> sqlx::Result<Vec<AiSession>> {
    sqlx::query_as::<_, AiSession>(
        "SELECT * FROM ai_sessions WHERE is_saved = 1 ORDER BY started_at DESC",
    )
    .fetch_all(pool)
    .await
}

pub async fn update(
    pool: &SqlitePool,
    id: &str,
    custom_name: Option<&str>,
    custom_description: Option<&str>,
    color: Option<&str>,
    icon: Option<&str>,
    linked_task_id: Option<&str>,
) -> sqlx::Result<Option<AiSession>> {
    sqlx::query_as::<_, AiSession>(
        "UPDATE ai_sessions
         SET custom_name        = COALESCE(?, custom_name),
             custom_description = COALESCE(?, custom_description),
             color              = COALESCE(?, color),
             icon               = COALESCE(?, icon),
             linked_task_id     = COALESCE(?, linked_task_id),
             last_activity_at   = CURRENT_TIMESTAMP
         WHERE id = ?
         RETURNING *",
    )
    .bind(custom_name)
    .bind(custom_description)
    .bind(color)
    .bind(icon)
    .bind(linked_task_id)
    .bind(id)
    .fetch_optional(pool)
    .await
}

pub async fn delete(pool: &SqlitePool, id: &str) -> sqlx::Result<()> {
    sqlx::query("DELETE FROM ai_sessions WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn find_active(
    pool: &SqlitePool,
    project_id: Option<&str>,
) -> sqlx::Result<Vec<ActiveSessionRow>> {
    if let Some(pid) = project_id {
        sqlx::query_as::<_, ActiveSessionRow>(
            "SELECT id, title, working_directory, status FROM ai_sessions \
             WHERE (status = 'running' OR status = 'starting') AND project_id = ? LIMIT 50",
        )
        .bind(pid)
        .fetch_all(pool)
        .await
    } else {
        sqlx::query_as::<_, ActiveSessionRow>(
            "SELECT id, title, working_directory, status FROM ai_sessions \
             WHERE status = 'running' OR status = 'starting' LIMIT 50",
        )
        .fetch_all(pool)
        .await
    }
}

pub async fn find_inbox(
    pool: &SqlitePool,
    session_id: &str,
    limit: i64,
) -> sqlx::Result<Vec<InboxMessageRow>> {
    sqlx::query_as::<_, InboxMessageRow>(
        "SELECT id, from_id, content, timestamp FROM messages WHERE session_id = ? ORDER BY timestamp DESC LIMIT ?",
    )
    .bind(session_id)
    .bind(limit)
    .fetch_all(pool)
    .await
}

pub async fn find_project_id(pool: &SqlitePool, session_id: &str) -> sqlx::Result<Option<String>> {
    sqlx::query_scalar::<_, String>("SELECT project_id FROM ai_sessions WHERE id = ?")
        .bind(session_id)
        .fetch_optional(pool)
        .await
}

#[derive(sqlx::FromRow, serde::Serialize)]
pub struct ActiveSessionRow {
    pub id: String,
    pub title: Option<String>,
    pub working_directory: String,
    pub status: String,
}

#[derive(sqlx::FromRow, serde::Serialize)]
pub struct InboxMessageRow {
    pub id: String,
    pub from_id: String,
    pub content: String,
    pub timestamp: String,
}

/// Resolve a valid session id: checks if `requested` exists in ai_sessions,
/// otherwise falls back to `caller` (the session from the MCP header).
/// Returns None if neither exists.
pub async fn resolve_session_id(
    pool: &SqlitePool,
    requested: &str,
    caller: &str,
) -> Option<String> {
    let exists: Option<(String,)> = sqlx::query_as(
        "SELECT id FROM ai_sessions WHERE id = ? LIMIT 1",
    )
    .bind(requested)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten();

    if exists.is_some() {
        return Some(requested.to_string());
    }

    // Fall back to caller session
    if caller != crate::constants::terminal::MCP_AGENT_ID {
        let fallback: Option<(String,)> = sqlx::query_as(
            "SELECT id FROM ai_sessions WHERE id = ? LIMIT 1",
        )
        .bind(caller)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten();
        if fallback.is_some() {
            return Some(caller.to_string());
        }
    }

    None
}
