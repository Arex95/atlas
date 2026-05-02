use crate::constants::errors;
use crate::dtos::reminder::{CreateReminderRequest, UpdateReminderRequest};
use crate::handlers::{err, err_internal, ok, ok_list, validate};
use crate::repositories::reminder as reminder_repo;
use crate::services::reminder as svc;
use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde::Deserialize;
use sqlx::SqlitePool;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReminderQuery {
    pub project_id: Option<String>,
    pub status: Option<String>,
}

pub async fn list_reminders(
    State(pool): State<SqlitePool>,
    Query(q): Query<ReminderQuery>,
) -> impl IntoResponse {
    match svc::list(&pool, q.project_id.as_deref(), q.status.as_deref()).await {
        Ok(reminders) => ok_list(reminders).into_response(),
        Err(e) => err_internal("reminder", e).into_response(),
    }
}

pub async fn create_reminder(
    State(pool): State<SqlitePool>,
    Json(payload): Json<CreateReminderRequest>,
) -> impl IntoResponse {
    if let Err(e) = validate(&payload) {
        return e.into_response();
    }

    match svc::create(&pool, &payload).await {
        Ok(r) => ok(r).into_response(),
        Err(e) => err_internal("reminder", e).into_response(),
    }
}

pub async fn update_reminder(
    State(pool): State<SqlitePool>,
    Path(id): Path<String>,
    Json(payload): Json<UpdateReminderRequest>,
) -> impl IntoResponse {
    if let Err(e) = validate(&payload) {
        return e.into_response();
    }

    let existing = reminder_repo::find_by_id(&pool, &id).await;
    let reminder = match existing {
        Ok(Some(r)) => r,
        Ok(None) => return err(StatusCode::NOT_FOUND, errors::REMINDER_NOT_FOUND).into_response(),
        Err(e) => return err_internal("reminder", e).into_response(),
    };

    match svc::update(&pool, &id, &payload, reminder).await {
        Ok(r) => ok(r).into_response(),
        Err(e) => err_internal("reminder", e).into_response(),
    }
}

pub async fn delete_reminder(
    State(pool): State<SqlitePool>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match svc::delete(&pool, &id).await {
        Ok(true) => ok(()).into_response(),
        Ok(false) => err(StatusCode::NOT_FOUND, errors::REMINDER_NOT_FOUND).into_response(),
        Err(e) => err_internal("reminder", e).into_response(),
    }
}

