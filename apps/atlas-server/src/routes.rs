use axum::{
    Router,
    body::Body,
    http::{Response, StatusCode, header},
    middleware,
    routing::{delete, get, patch, post},
};
use sqlx::SqlitePool;

use crate::handlers::{
    document::{create_document, delete_document, get_document, list_documents, update_document},
    metric::get_project_metrics,
    notification::{create_notification, delete_notification, list_notifications, mark_all_read},
    orchestrator::{chat, delete_memory, get_memory, list_memory, notify, set_memory},
    project::{create_project, delete_project, get_projects, index_project, update_project},
    profile::{get_profile, update_profile},
    prompt::{create_prompt, delete_prompt, get_prompt, list_prompts, update_prompt},
    reminder::{create_reminder, delete_reminder, list_reminders, update_reminder},
    session::{
        create_message, create_session, delete_message, delete_session, get_live_sessions,
        get_project_sessions, get_saved_sessions, get_session_git, get_session_history,
        list_session_documents, list_session_memory, list_session_reminders, list_session_tasks,
        delete_session_memory, delete_session_document,
        save_session, update_session,
    },
    skill::{create_skill, delete_skill, get_skill, list_skills, update_skill},
    system::{export_db, import_db},
    task::{create_task, delete_task, list_tasks, update_task},
    global::{
        create_global_prompt, create_global_skill, delete_global_memory, delete_global_prompt,
        delete_global_skill, list_global_memory, list_global_prompts, list_global_skills,
        update_global_prompt, update_global_skill, upsert_global_memory,
    },
    search::global_search,
    webhook::trigger_webhook,
};
use crate::middleware::{api_auth, mcp_auth};

pub fn router() -> Router<SqlitePool> {
    let mcp = Router::new()
        .route(
            "/api/mcp",
            get(crate::handlers::mcp::handle_mcp_sse).post(crate::handlers::mcp::handle_mcp_request),
        )
        .route_layer(middleware::from_fn(mcp_auth));

    let api = Router::new()
        // projects
        .route("/api/projects", get(get_projects).post(create_project))
        .route("/api/projects/:slug", patch(update_project).delete(delete_project))
        .route("/api/projects/:slug/index", post(index_project))
        .route("/api/projects/:slug/metrics", get(get_project_metrics))
        // sessions
        .route("/api/projects/:slug/sessions", get(get_project_sessions).post(create_session))
        .route("/api/sessions/saved", get(get_saved_sessions))
        .route("/api/sessions/active", get(get_live_sessions))
        .route("/api/sessions/:id", patch(update_session).delete(delete_session))
        .route("/api/sessions/:id/save", post(save_session))
        .route("/api/sessions/:id/git", get(get_session_git))
        .route("/api/sessions/:id/history", get(get_session_history).post(create_message))
        .route("/api/sessions/:id/history/:msgId", delete(delete_message))
        .route("/api/sessions/:id/send", post(crate::handlers::message::send_cross_session_message))
        .route("/api/sessions/:id/messages", get(crate::handlers::message::get_session_messages))
        .route("/api/sessions/:id/memory", get(list_session_memory))
        .route("/api/sessions/:id/memory/:key", delete(delete_session_memory))
        .route("/api/sessions/:id/documents", get(list_session_documents))
        .route("/api/sessions/:id/documents/:docId", delete(delete_session_document))
        .route("/api/sessions/:id/tasks", get(list_session_tasks))
        .route("/api/sessions/:id/reminders", get(list_session_reminders))
        // orchestrator memory
        .route("/api/orchestrator/memory", get(list_memory).post(set_memory))
        .route("/api/orchestrator/memory/:key", get(get_memory).delete(delete_memory))
        .route("/api/orchestrator/chat", post(chat))
        .route("/api/orchestrator/notify", post(notify))
        // documents
        .route("/api/documents", get(list_documents).post(create_document))
        .route("/api/documents/:id", get(get_document).patch(update_document).delete(delete_document))
        // skills
        .route("/api/skills", get(list_skills).post(create_skill))
        .route("/api/skills/:id", get(get_skill).patch(update_skill).delete(delete_skill))
        // notifications
        .route("/api/notifications", get(list_notifications).post(create_notification))
        .route("/api/notifications/mark-all-read", post(mark_all_read))
        .route("/api/notifications/:id", delete(delete_notification))
        // reminders
        .route("/api/reminders", get(list_reminders).post(create_reminder))
        .route("/api/reminders/:id", patch(update_reminder).delete(delete_reminder))
        // tasks
        .route("/api/tasks", get(list_tasks).post(create_task))
        .route("/api/tasks/:id", patch(update_task).delete(delete_task))
        // prompts
        .route("/api/prompts", get(list_prompts).post(create_prompt))
        .route("/api/prompts/:id", get(get_prompt).patch(update_prompt).delete(delete_prompt))
        // filesystem
        .route("/api/fs/list", get(crate::handlers::fs::list_files))
        .route("/api/fs/read", get(crate::handlers::fs::read_file))
        // profile
        .route("/api/profile", get(get_profile).put(update_profile))
        // global context (read by all agents, written only from UI)
        .route("/api/global/memory", get(list_global_memory).post(upsert_global_memory))
        .route("/api/global/memory/:key", delete(delete_global_memory))
        .route("/api/global/skills", get(list_global_skills).post(create_global_skill))
        .route("/api/global/skills/:id", patch(update_global_skill).delete(delete_global_skill))
        .route("/api/global/prompts", get(list_global_prompts).post(create_global_prompt))
        .route("/api/global/prompts/:id", patch(update_global_prompt).delete(delete_global_prompt))
        // search
        .route("/api/search", get(global_search))
        // webhooks
        .route("/api/webhooks/trigger", post(trigger_webhook))
        // system
        .route("/api/db/export", get(export_db))
        .route("/api/db/import", post(import_db))
        .route_layer(middleware::from_fn(api_auth));

    Router::new()
        .merge(api)
        .merge(mcp)
        .fallback(spa_handler)
}

async fn spa_handler(uri: axum::http::Uri) -> Response<Body> {
    let dist = web_dist_path();
    let path = uri.path().trim_start_matches('/');
    let file_path = if path.is_empty() { dist.join("index.html") } else { dist.join(path) };

    #[allow(clippy::collapsible_if)]
    if file_path.is_file() {
        if let Ok(bytes) = tokio::fs::read(&file_path).await {
            let mime = mime_guess::from_path(&file_path).first_or_octet_stream().to_string();
            return Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, mime)
                .body(Body::from(bytes))
                .unwrap();
        }
    }

    match tokio::fs::read(dist.join("index.html")).await {
        Ok(bytes) => Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
            .body(Body::from(bytes))
            .unwrap(),
        Err(_) => Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::from("Frontend not built. Run: make build-web"))
            .unwrap(),
    }
}

pub fn web_dist_path() -> std::path::PathBuf {
    std::env::var("WEB_DIST")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| {
            // When run from the repo root (cargo run / make start), resolve to apps/web/dist.
            // When the binary is distributed standalone, expect dist/ next to the binary.
            let cwd = std::env::current_dir().unwrap_or_default();
            let candidate = cwd.join("apps/web/dist");
            if candidate.exists() { candidate } else { cwd.join("dist") }
        })
}

#[allow(dead_code)]
fn web_index_path() -> std::path::PathBuf {
    web_dist_path().join("index.html")
}
