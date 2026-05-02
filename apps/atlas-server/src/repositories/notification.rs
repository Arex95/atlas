use crate::models::Notification;
use sqlx::SqlitePool;

pub async fn find_all(
    pool: &SqlitePool,
    project_id: Option<&str>,
    status: Option<&str>,
) -> sqlx::Result<Vec<Notification>> {
    match (project_id, status) {
        (Some(pid), Some(s)) => sqlx::query_as::<_, Notification>(
            "SELECT * FROM notifications WHERE project_id = ? AND status = ? ORDER BY created_at DESC LIMIT 100",
        )
        .bind(pid)
        .bind(s)
        .fetch_all(pool)
        .await,
        (Some(pid), None) => sqlx::query_as::<_, Notification>(
            "SELECT * FROM notifications WHERE project_id = ? ORDER BY created_at DESC LIMIT 100",
        )
        .bind(pid)
        .fetch_all(pool)
        .await,
        (None, Some(s)) => sqlx::query_as::<_, Notification>(
            "SELECT * FROM notifications WHERE status = ? ORDER BY created_at DESC LIMIT 100",
        )
        .bind(s)
        .fetch_all(pool)
        .await,
        (None, None) => sqlx::query_as::<_, Notification>(
            "SELECT * FROM notifications ORDER BY created_at DESC LIMIT 100",
        )
        .fetch_all(pool)
        .await,
    }
}

pub async fn create(
    pool: &SqlitePool,
    id: &str,
    project_id: Option<&str>,
    session_id: Option<&str>,
    title: Option<&str>,
    message: &str,
    kind: &str,
) -> sqlx::Result<Notification> {
    sqlx::query(
        "INSERT INTO notifications (id, project_id, session_id, title, message, type) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(id)
    .bind(project_id)
    .bind(session_id)
    .bind(title)
    .bind(message)
    .bind(kind)
    .execute(pool)
    .await?;

    sqlx::query_as::<_, Notification>("SELECT * FROM notifications WHERE id = ?")
        .bind(id)
        .fetch_one(pool)
        .await
}

pub async fn mark_all_read(pool: &SqlitePool) -> sqlx::Result<()> {
    sqlx::query("UPDATE notifications SET status = 'read' WHERE status = 'unread'")
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn delete(pool: &SqlitePool, id: &str) -> sqlx::Result<bool> {
    let res = sqlx::query("DELETE FROM notifications WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(res.rows_affected() > 0)
}
