use crate::handlers::{err_internal, ok};
use axum::{
    extract::{Path, State},
    response::IntoResponse,
};
use serde::Serialize;
use sqlx::SqlitePool;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectMetrics {
    pub total_sessions: i64,
    pub active_sessions: i64,
    pub saved_sessions: i64,
    pub total_documents: i64,
    pub total_skills: i64,
    pub total_notifications: i64,
    pub unread_notifications: i64,
    pub pending_reminders: i64,
    pub memory_keys: i64,
    pub total_tasks: i64,
    pub open_tasks: i64,
    pub blocked_tasks: i64,
    pub done_tasks: i64,
    pub overdue_tasks: i64,
    pub health_score: i64,
}

pub async fn get_project_metrics(
    State(pool): State<SqlitePool>,
    Path(slug): Path<String>,
) -> impl IntoResponse {
    #[derive(sqlx::FromRow)]
    struct ProjectRow {
        id: String,
    }

    let project = sqlx::query_as::<_, ProjectRow>("SELECT id FROM projects WHERE slug = ?")
        .bind(&slug)
        .fetch_optional(&pool)
        .await;

    let project_id = match project {
        Ok(Some(p)) => p.id,
        Ok(None) => return err_internal("metric", "project not found").into_response(),
        Err(e) => return err_internal("metric", e).into_response(),
    };

    macro_rules! count {
        ($query:expr) => {
            sqlx::query_scalar::<_, i64>($query)
                .bind(&project_id)
                .fetch_one(&pool)
                .await
                .unwrap_or(0)
        };
        ($query:expr, $($bind:expr),+) => {
            sqlx::query_scalar::<_, i64>($query)
                $(.bind($bind))+
                .fetch_one(&pool)
                .await
                .unwrap_or(0)
        };
    }

    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();

    let total_sessions = count!("SELECT COUNT(*) FROM ai_sessions WHERE project_id = ?");
    let active_sessions = count!(
        "SELECT COUNT(*) FROM ai_sessions WHERE project_id = ? AND status IN ('running', 'starting')"
    );
    let saved_sessions =
        count!("SELECT COUNT(*) FROM ai_sessions WHERE project_id = ? AND is_saved = 1");
    let total_documents =
        count!("SELECT COUNT(*) FROM project_documents WHERE project_id = ?");
    let total_skills = count!("SELECT COUNT(*) FROM agent_skills WHERE project_id = ?");
    let total_notifications =
        count!("SELECT COUNT(*) FROM notifications WHERE project_id = ?");
    let unread_notifications = count!(
        "SELECT COUNT(*) FROM notifications WHERE project_id = ? AND status = 'unread'"
    );
    let pending_reminders = count!(
        "SELECT COUNT(*) FROM reminders WHERE project_id = ? AND status = 'pending'"
    );
    let memory_keys = count!("SELECT COUNT(*) FROM agent_memory WHERE project_id = ?");
    let total_tasks = count!("SELECT COUNT(*) FROM tasks WHERE project_id = ?");
    let open_tasks = count!(
        "SELECT COUNT(*) FROM tasks WHERE project_id = ? AND status IN ('todo', 'in-progress')"
    );
    let blocked_tasks =
        count!("SELECT COUNT(*) FROM tasks WHERE project_id = ? AND status = 'blocked'");
    let done_tasks =
        count!("SELECT COUNT(*) FROM tasks WHERE project_id = ? AND status = 'done'");
    let overdue_tasks = count!(
        "SELECT COUNT(*) FROM tasks WHERE project_id = ? AND status != 'done' AND due_date IS NOT NULL AND due_date < ?",
        &project_id,
        &today
    );
    let overdue_reminders = count!(
        "SELECT COUNT(*) FROM reminders WHERE project_id = ? AND status = 'pending' AND due_at < ?",
        &project_id,
        &today
    );

    let health_score = {
        let mut score = 100i64;
        score -= (blocked_tasks * 10).min(30);
        score -= (overdue_tasks * 5).min(25);
        score -= (overdue_reminders * 5).min(15);
        if active_sessions > 0 {
            score = (score + 5).min(100);
        }
        score.max(0)
    };

    ok(ProjectMetrics {
        total_sessions,
        active_sessions,
        saved_sessions,
        total_documents,
        total_skills,
        total_notifications,
        unread_notifications,
        pending_reminders,
        memory_keys,
        total_tasks,
        open_tasks,
        blocked_tasks,
        done_tasks,
        overdue_tasks,
        health_score,
    })
    .into_response()
}
