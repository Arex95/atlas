use crate::constants::{defaults, env, errors};
use crate::dtos::session::{CreateMessageRequest, CreateSessionRequest, SaveSessionRequest};
use crate::handlers::{err, err_internal, ok, ok_list, validate};
use crate::services::session as svc;
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use sqlx::SqlitePool;

pub async fn create_session(
    State(pool): State<SqlitePool>,
    Path(slug): Path<String>,
    Json(payload): Json<CreateSessionRequest>,
) -> impl IntoResponse {
    let default_author =
        std::env::var(env::ATLAS_DEFAULT_AUTHOR).unwrap_or_else(|_| defaults::AUTHOR.to_string());

    match svc::create(&pool, &slug, &payload, &default_author).await {
        Ok(Some(session)) => ok(session).into_response(),
        Ok(None) => err(StatusCode::NOT_FOUND, errors::PROJECT_NOT_FOUND).into_response(),
        Err(e) => err_internal("sessions", e).into_response(),
    }
}

pub async fn save_session(
    State(pool): State<SqlitePool>,
    Path(id): Path<String>,
    Json(payload): Json<SaveSessionRequest>,
) -> impl IntoResponse {
    if let Err(rejection) = validate(&payload) {
        return rejection.into_response();
    }

    match svc::save(
        &pool,
        &id,
        Some(payload.custom_name.as_str()),
        payload.custom_description.as_deref(),
        payload.color.as_deref(),
        payload.icon.as_deref(),
    )
    .await
    {
        Ok(session) => ok(session).into_response(),
        Err(e) => err_internal("sessions", e).into_response(),
    }
}

pub async fn get_session_history(
    State(pool): State<SqlitePool>,
    Path(session_id): Path<String>,
) -> impl IntoResponse {
    match svc::get_history(&pool, &session_id).await {
        Ok(messages) => ok_list(messages).into_response(),
        Err(e) => {
            tracing::error!("[sessions] get_session_history failed: {}", e);
            err(StatusCode::INTERNAL_SERVER_ERROR, errors::SESSION_HISTORY_FAILED).into_response()
        }
    }
}

pub async fn create_message(
    State(pool): State<SqlitePool>,
    Path(session_id): Path<String>,
    Json(payload): Json<CreateMessageRequest>,
) -> impl IntoResponse {
    match svc::create_message(&pool, &session_id, &payload.role, &payload.content).await {
        Ok(msg) => ok(msg).into_response(),
        Err(e) => {
            tracing::error!("[sessions] create_message failed: {}", e);
            err(StatusCode::INTERNAL_SERVER_ERROR, errors::MESSAGE_CREATE_FAILED).into_response()
        }
    }
}

pub async fn delete_message(
    State(pool): State<SqlitePool>,
    Path((_session_id, msg_id)): Path<(String, String)>,
) -> impl IntoResponse {
    match svc::delete_message(&pool, &msg_id).await {
        Ok(true) => ok(()).into_response(),
        Ok(false) => err(StatusCode::NOT_FOUND, "Message not found").into_response(),
        Err(e) => err_internal("sessions", e).into_response(),
    }
}

pub async fn get_project_sessions(
    State(pool): State<SqlitePool>,
    Path(slug): Path<String>,
) -> impl IntoResponse {
    match svc::list_for_project(&pool, &slug).await {
        Ok(Some(sessions)) => ok_list(sessions).into_response(),
        Ok(None) => err(StatusCode::NOT_FOUND, errors::PROJECT_NOT_FOUND).into_response(),
        Err(e) => err_internal("sessions", e).into_response(),
    }
}

pub async fn get_saved_sessions(State(pool): State<SqlitePool>) -> impl IntoResponse {
    match svc::list_saved(&pool).await {
        Ok(sessions) => ok_list(sessions).into_response(),
        Err(e) => err_internal("sessions", e).into_response(),
    }
}

pub async fn update_session(
    State(pool): State<SqlitePool>,
    Path(id): Path<String>,
    Json(payload): Json<crate::dtos::session::UpdateSessionRequest>,
) -> impl IntoResponse {
    match svc::update(&pool, &id, &payload).await {
        Ok(Some(session)) => ok(session).into_response(),
        Ok(None) => err(StatusCode::NOT_FOUND, errors::SESSION_NOT_FOUND).into_response(),
        Err(e) => err_internal("sessions", e).into_response(),
    }
}

use crate::terminal::TerminalManager;
use axum::Extension;
use std::sync::Arc;

pub async fn get_live_sessions(
    Extension(tm): Extension<Arc<TerminalManager>>,
) -> impl IntoResponse {
    let sessions = tm.get_live_sessions().await;
    ok_list(sessions).into_response()
}

pub async fn delete_session(
    State(pool): State<SqlitePool>,
    Extension(tm): Extension<Arc<TerminalManager>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    tm.kill_session(&id).await;

    match svc::delete(&pool, &id).await {
        Ok(()) => ok(serde_json::json!({ "status": "success", "id": id })).into_response(),
        Err(e) => err_internal("sessions", e).into_response(),
    }
}

pub async fn get_session_git(
    State(pool): State<SqlitePool>,
    Path(session_id): Path<String>,
) -> impl IntoResponse {
    let row = sqlx::query_scalar::<_, String>(
        "SELECT working_directory FROM ai_sessions WHERE id = ?",
    )
    .bind(&session_id)
    .fetch_optional(&pool)
    .await;

    match row {
        Ok(Some(wd)) => ok(crate::git::get_git_info(&wd)).into_response(),
        Ok(None) => err(StatusCode::NOT_FOUND, errors::SESSION_NOT_FOUND).into_response(),
        Err(e) => err_internal("sessions", e).into_response(),
    }
}

pub async fn list_session_memory(
    State(pool): State<SqlitePool>,
    Path(session_id): Path<String>,
) -> impl IntoResponse {
    match crate::repositories::session_memory::find_all(&pool, &session_id).await {
        Ok(rows) => ok_list(rows).into_response(),
        Err(e) => err_internal("session_memory", e).into_response(),
    }
}

pub async fn delete_session_memory(
    State(pool): State<SqlitePool>,
    Path((session_id, key)): Path<(String, String)>,
) -> impl IntoResponse {
    match crate::repositories::session_memory::delete(&pool, &session_id, &key).await {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => err(StatusCode::NOT_FOUND, "Memory key not found").into_response(),
        Err(e) => err_internal("session_memory", e).into_response(),
    }
}

pub async fn list_session_documents(
    State(pool): State<SqlitePool>,
    Path(session_id): Path<String>,
) -> impl IntoResponse {
    match crate::repositories::session_document::find_all(&pool, &session_id, None).await {
        Ok(rows) => ok_list(rows).into_response(),
        Err(e) => err_internal("session_documents", e).into_response(),
    }
}

pub async fn delete_session_document(
    State(pool): State<SqlitePool>,
    Path((_session_id, doc_id)): Path<(String, String)>,
) -> impl IntoResponse {
    match crate::repositories::session_document::delete(&pool, &doc_id).await {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => err(StatusCode::NOT_FOUND, "Document not found").into_response(),
        Err(e) => err_internal("session_documents", e).into_response(),
    }
}

pub async fn list_session_tasks(
    State(pool): State<SqlitePool>,
    Path(session_id): Path<String>,
) -> impl IntoResponse {
    match crate::repositories::task::find_for_session(&pool, &session_id, None).await {
        Ok(rows) => ok_list(rows).into_response(),
        Err(e) => err_internal("session_tasks", e).into_response(),
    }
}

pub async fn list_session_reminders(
    State(pool): State<SqlitePool>,
    Path(session_id): Path<String>,
) -> impl IntoResponse {
    match crate::repositories::reminder::find_for_session(&pool, &session_id, None).await {
        Ok(rows) => ok_list(rows).into_response(),
        Err(e) => err_internal("session_reminders", e).into_response(),
    }
}
