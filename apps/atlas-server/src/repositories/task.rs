use crate::models::Task;
use sqlx::SqlitePool;

pub async fn find_all(
    pool: &SqlitePool,
    project_id: Option<&str>,
    status: Option<&str>,
    parent_id: Option<&str>,
) -> sqlx::Result<Vec<Task>> {
    match (project_id, status, parent_id) {
        (Some(pid), Some(s), Some(par)) => sqlx::query_as::<_, Task>(
            "SELECT id, project_id, title, description, status, priority, due_date, assigned_to, tags, parent_id, created_at, updated_at FROM tasks WHERE project_id = ? AND status = ? AND parent_id = ? ORDER BY due_date ASC NULLS LAST, created_at ASC",
        )
        .bind(pid)
        .bind(s)
        .bind(par)
        .fetch_all(pool)
        .await,
        (Some(pid), Some(s), None) => sqlx::query_as::<_, Task>(
            "SELECT id, project_id, title, description, status, priority, due_date, assigned_to, tags, parent_id, created_at, updated_at FROM tasks WHERE project_id = ? AND status = ? ORDER BY due_date ASC NULLS LAST, created_at ASC",
        )
        .bind(pid)
        .bind(s)
        .fetch_all(pool)
        .await,
        (Some(pid), None, Some(par)) => sqlx::query_as::<_, Task>(
            "SELECT id, project_id, title, description, status, priority, due_date, assigned_to, tags, parent_id, created_at, updated_at FROM tasks WHERE project_id = ? AND parent_id = ? ORDER BY due_date ASC NULLS LAST, created_at ASC",
        )
        .bind(pid)
        .bind(par)
        .fetch_all(pool)
        .await,
        (Some(pid), None, None) => sqlx::query_as::<_, Task>(
            "SELECT id, project_id, title, description, status, priority, due_date, assigned_to, tags, parent_id, created_at, updated_at FROM tasks WHERE project_id = ? ORDER BY due_date ASC NULLS LAST, created_at ASC",
        )
        .bind(pid)
        .fetch_all(pool)
        .await,
        (None, Some(s), Some(par)) => sqlx::query_as::<_, Task>(
            "SELECT id, project_id, title, description, status, priority, due_date, assigned_to, tags, parent_id, created_at, updated_at FROM tasks WHERE status = ? AND parent_id = ? ORDER BY due_date ASC NULLS LAST, created_at ASC",
        )
        .bind(s)
        .bind(par)
        .fetch_all(pool)
        .await,
        (None, Some(s), None) => sqlx::query_as::<_, Task>(
            "SELECT id, project_id, title, description, status, priority, due_date, assigned_to, tags, parent_id, created_at, updated_at FROM tasks WHERE status = ? ORDER BY due_date ASC NULLS LAST, created_at ASC",
        )
        .bind(s)
        .fetch_all(pool)
        .await,
        (None, None, Some(par)) => sqlx::query_as::<_, Task>(
            "SELECT id, project_id, title, description, status, priority, due_date, assigned_to, tags, parent_id, created_at, updated_at FROM tasks WHERE parent_id = ? ORDER BY due_date ASC NULLS LAST, created_at ASC",
        )
        .bind(par)
        .fetch_all(pool)
        .await,
        (None, None, None) => sqlx::query_as::<_, Task>(
            "SELECT id, project_id, title, description, status, priority, due_date, assigned_to, tags, parent_id, created_at, updated_at FROM tasks ORDER BY due_date ASC NULLS LAST, created_at ASC",
        )
        .fetch_all(pool)
        .await,
    }
}

pub async fn find_by_id(pool: &SqlitePool, id: &str) -> sqlx::Result<Option<Task>> {
    sqlx::query_as::<_, Task>(
        "SELECT id, project_id, title, description, status, priority, due_date, assigned_to, tags, parent_id, created_at, updated_at FROM tasks WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}

#[allow(clippy::too_many_arguments)]
pub async fn create(
    pool: &SqlitePool,
    id: &str,
    project_id: &str,
    title: &str,
    description: &str,
    status: &str,
    priority: &str,
    due_date: Option<&str>,
    assigned_to: Option<&str>,
    tags_json: &str,
    parent_id: Option<&str>,
) -> sqlx::Result<Task> {
    sqlx::query(
        "INSERT INTO tasks (id, project_id, title, description, status, priority, due_date, assigned_to, tags, parent_id) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(id)
    .bind(project_id)
    .bind(title)
    .bind(description)
    .bind(status)
    .bind(priority)
    .bind(due_date)
    .bind(assigned_to)
    .bind(tags_json)
    .bind(parent_id)
    .execute(pool)
    .await?;

    sqlx::query_as::<_, Task>(
        "SELECT id, project_id, title, description, status, priority, due_date, assigned_to, tags, parent_id, created_at, updated_at FROM tasks WHERE id = ?",
    )
    .bind(id)
    .fetch_one(pool)
    .await
}

#[allow(clippy::too_many_arguments)]
pub async fn update(
    pool: &SqlitePool,
    id: &str,
    title: Option<&str>,
    description: Option<&str>,
    status: Option<&str>,
    priority: Option<&str>,
    due_date: Option<&str>,
    assigned_to: Option<&str>,
    tags_json: Option<&str>,
    parent_id: Option<&str>,
) -> sqlx::Result<Option<Task>> {
    sqlx::query(
        "UPDATE tasks SET
            title       = COALESCE(?1, title),
            description = COALESCE(?2, description),
            status      = COALESCE(?3, status),
            priority    = COALESCE(?4, priority),
            due_date    = CASE WHEN ?5 IS NOT NULL THEN ?5 ELSE due_date END,
            assigned_to = CASE WHEN ?6 IS NOT NULL THEN ?6 ELSE assigned_to END,
            tags        = COALESCE(?7, tags),
            parent_id   = CASE WHEN ?8 IS NOT NULL THEN ?8 ELSE parent_id END,
            updated_at  = CURRENT_TIMESTAMP
         WHERE id = ?9",
    )
    .bind(title)
    .bind(description)
    .bind(status)
    .bind(priority)
    .bind(due_date)
    .bind(assigned_to)
    .bind(tags_json)
    .bind(parent_id)
    .bind(id)
    .execute(pool)
    .await?;

    find_by_id(pool, id).await
}

pub async fn delete(pool: &SqlitePool, id: &str) -> sqlx::Result<bool> {
    let res = sqlx::query("DELETE FROM tasks WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(res.rows_affected() > 0)
}

pub async fn find_for_mcp(
    pool: &SqlitePool,
    project_id: Option<&str>,
    status: Option<&str>,
) -> sqlx::Result<Vec<TaskMcpRow>> {
    match (project_id, status) {
        (Some(pid), Some(s)) => sqlx::query_as::<_, TaskMcpRow>(
            "SELECT id, title, status, priority, due_date, assigned_to FROM tasks WHERE project_id = ? AND status = ? ORDER BY due_date ASC",
        )
        .bind(pid)
        .bind(s)
        .fetch_all(pool)
        .await,
        (Some(pid), None) => sqlx::query_as::<_, TaskMcpRow>(
            "SELECT id, title, status, priority, due_date, assigned_to FROM tasks WHERE project_id = ? ORDER BY due_date ASC",
        )
        .bind(pid)
        .fetch_all(pool)
        .await,
        _ => sqlx::query_as::<_, TaskMcpRow>(
            "SELECT id, title, status, priority, due_date, assigned_to FROM tasks ORDER BY due_date ASC LIMIT 100",
        )
        .fetch_all(pool)
        .await,
    }
}

#[allow(clippy::too_many_arguments)]
pub async fn create_simple(
    pool: &SqlitePool,
    id: &str,
    project_id: &str,
    title: &str,
    description: &str,
    status: &str,
    priority: &str,
    due_date: Option<&str>,
    assigned_to: Option<&str>,
) -> sqlx::Result<bool> {
    let res = sqlx::query(
        "INSERT INTO tasks (id, project_id, title, description, status, priority, due_date, assigned_to, tags) VALUES (?, ?, ?, ?, ?, ?, ?, ?, '[]')",
    )
    .bind(id)
    .bind(project_id)
    .bind(title)
    .bind(description)
    .bind(status)
    .bind(priority)
    .bind(due_date)
    .bind(assigned_to)
    .execute(pool)
    .await?;
    Ok(res.rows_affected() > 0)
}

#[allow(clippy::too_many_arguments)]
pub async fn update_simple(
    pool: &SqlitePool,
    id: &str,
    title: Option<&str>,
    description: Option<&str>,
    status: Option<&str>,
    priority: Option<&str>,
    due_date: Option<&str>,
    assigned_to: Option<&str>,
) -> sqlx::Result<bool> {
    let res = sqlx::query(
        "UPDATE tasks SET title = COALESCE(?1, title), description = COALESCE(?2, description), status = COALESCE(?3, status), priority = COALESCE(?4, priority), due_date = COALESCE(?5, due_date), assigned_to = COALESCE(?6, assigned_to), updated_at = CURRENT_TIMESTAMP WHERE id = ?7",
    )
    .bind(title)
    .bind(description)
    .bind(status)
    .bind(priority)
    .bind(due_date)
    .bind(assigned_to)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(res.rows_affected() > 0)
}

pub async fn find_for_session(
    pool: &SqlitePool,
    session_id: &str,
    status: Option<&str>,
) -> sqlx::Result<Vec<TaskMcpRow>> {
    match status {
        Some(s) => sqlx::query_as::<_, TaskMcpRow>(
            "SELECT id, title, status, priority, due_date, assigned_to FROM tasks WHERE session_id = ? AND status = ? ORDER BY due_date ASC NULLS LAST, created_at ASC",
        )
        .bind(session_id)
        .bind(s)
        .fetch_all(pool)
        .await,
        None => sqlx::query_as::<_, TaskMcpRow>(
            "SELECT id, title, status, priority, due_date, assigned_to FROM tasks WHERE session_id = ? ORDER BY due_date ASC NULLS LAST, created_at ASC",
        )
        .bind(session_id)
        .fetch_all(pool)
        .await,
    }
}

#[allow(clippy::too_many_arguments)]
pub async fn create_for_session(
    pool: &SqlitePool,
    id: &str,
    session_id: &str,
    title: &str,
    description: &str,
    status: &str,
    priority: &str,
    due_date: Option<&str>,
    assigned_to: Option<&str>,
) -> sqlx::Result<bool> {
    let res = sqlx::query(
        "INSERT INTO tasks (id, session_id, title, description, status, priority, due_date, assigned_to, tags) VALUES (?, ?, ?, ?, ?, ?, ?, ?, '[]')",
    )
    .bind(id)
    .bind(session_id)
    .bind(title)
    .bind(description)
    .bind(status)
    .bind(priority)
    .bind(due_date)
    .bind(assigned_to)
    .execute(pool)
    .await?;
    Ok(res.rows_affected() > 0)
}

#[derive(sqlx::FromRow, serde::Serialize)]
pub struct TaskMcpRow {
    pub id: String,
    pub title: String,
    pub status: String,
    pub priority: String,
    pub due_date: Option<String>,
    pub assigned_to: Option<String>,
}
