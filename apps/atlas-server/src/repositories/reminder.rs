use crate::models::Reminder;
use sqlx::SqlitePool;

pub async fn find_all(
    pool: &SqlitePool,
    project_id: Option<&str>,
    status: Option<&str>,
) -> sqlx::Result<Vec<Reminder>> {
    match (project_id, status) {
        (Some(pid), Some(s)) => sqlx::query_as::<_, Reminder>(
            "SELECT * FROM reminders WHERE project_id = ? AND status = ? ORDER BY due_at ASC",
        )
        .bind(pid)
        .bind(s)
        .fetch_all(pool)
        .await,
        (Some(pid), None) => sqlx::query_as::<_, Reminder>(
            "SELECT * FROM reminders WHERE project_id = ? ORDER BY due_at ASC",
        )
        .bind(pid)
        .fetch_all(pool)
        .await,
        (None, Some(s)) => sqlx::query_as::<_, Reminder>(
            "SELECT * FROM reminders WHERE status = ? ORDER BY due_at ASC",
        )
        .bind(s)
        .fetch_all(pool)
        .await,
        (None, None) => sqlx::query_as::<_, Reminder>(
            "SELECT * FROM reminders ORDER BY due_at ASC",
        )
        .fetch_all(pool)
        .await,
    }
}

pub async fn find_by_id(pool: &SqlitePool, id: &str) -> sqlx::Result<Option<Reminder>> {
    sqlx::query_as::<_, Reminder>("SELECT * FROM reminders WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await
}

pub async fn create(
    pool: &SqlitePool,
    id: &str,
    project_id: Option<&str>,
    session_id: Option<&str>,
    title: &str,
    description: &str,
    due_at: &str,
    kind: &str,
) -> sqlx::Result<Reminder> {
    sqlx::query(
        "INSERT INTO reminders (id, project_id, session_id, title, description, due_at, kind) VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(id)
    .bind(project_id)
    .bind(session_id)
    .bind(title)
    .bind(description)
    .bind(due_at)
    .bind(kind)
    .execute(pool)
    .await?;

    sqlx::query_as::<_, Reminder>("SELECT * FROM reminders WHERE id = ?")
        .bind(id)
        .fetch_one(pool)
        .await
}

pub async fn find_for_session(
    pool: &SqlitePool,
    session_id: &str,
    status: Option<&str>,
) -> sqlx::Result<Vec<ReminderMcpRow>> {
    match status {
        Some(s) => sqlx::query_as::<_, ReminderMcpRow>(
            "SELECT id, title, due_at, status FROM reminders WHERE session_id = ? AND status = ? ORDER BY due_at ASC",
        )
        .bind(session_id)
        .bind(s)
        .fetch_all(pool)
        .await,
        None => sqlx::query_as::<_, ReminderMcpRow>(
            "SELECT id, title, due_at, status FROM reminders WHERE session_id = ? ORDER BY due_at ASC",
        )
        .bind(session_id)
        .fetch_all(pool)
        .await,
    }
}

pub async fn update(
    pool: &SqlitePool,
    id: &str,
    title: &str,
    description: &str,
    due_at: &str,
    status: &str,
) -> sqlx::Result<Reminder> {
    sqlx::query(
        "UPDATE reminders SET title = ?, description = ?, due_at = ?, status = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
    )
    .bind(title)
    .bind(description)
    .bind(due_at)
    .bind(status)
    .bind(id)
    .execute(pool)
    .await?;

    sqlx::query_as::<_, Reminder>("SELECT * FROM reminders WHERE id = ?")
        .bind(id)
        .fetch_one(pool)
        .await
}

pub async fn update_simple(
    pool: &SqlitePool,
    id: &str,
    title: Option<&str>,
    due_at: Option<&str>,
    status: Option<&str>,
) -> sqlx::Result<bool> {
    let res = sqlx::query(
        "UPDATE reminders SET title = COALESCE(?1, title), due_at = COALESCE(?2, due_at), status = COALESCE(?3, status), updated_at = CURRENT_TIMESTAMP WHERE id = ?4",
    )
    .bind(title)
    .bind(due_at)
    .bind(status)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(res.rows_affected() > 0)
}

pub async fn delete(pool: &SqlitePool, id: &str) -> sqlx::Result<bool> {
    let res = sqlx::query("DELETE FROM reminders WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(res.rows_affected() > 0)
}

pub async fn find_for_mcp(
    pool: &SqlitePool,
    project_id: Option<&str>,
    status: Option<&str>,
) -> sqlx::Result<Vec<ReminderMcpRow>> {
    match (project_id, status) {
        (Some(pid), Some(s)) => sqlx::query_as::<_, ReminderMcpRow>(
            "SELECT id, title, due_at, status FROM reminders WHERE project_id = ? AND status = ? ORDER BY due_at ASC",
        )
        .bind(pid)
        .bind(s)
        .fetch_all(pool)
        .await,
        (Some(pid), None) => sqlx::query_as::<_, ReminderMcpRow>(
            "SELECT id, title, due_at, status FROM reminders WHERE project_id = ? ORDER BY due_at ASC",
        )
        .bind(pid)
        .fetch_all(pool)
        .await,
        (None, _) => sqlx::query_as::<_, ReminderMcpRow>(
            "SELECT id, title, due_at, status FROM reminders ORDER BY due_at ASC LIMIT 50",
        )
        .fetch_all(pool)
        .await,
    }
}

#[derive(sqlx::FromRow, serde::Serialize)]
pub struct ReminderMcpRow {
    pub id: String,
    pub title: String,
    pub due_at: String,
    pub status: String,
}
