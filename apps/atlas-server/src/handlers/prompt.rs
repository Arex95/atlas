use crate::constants::errors;
use crate::dtos::prompt::{CreatePromptRequest, UpdatePromptRequest};
use crate::handlers::{err, err_internal, ok, ok_list, validate};
use crate::repositories::prompt as prompt_repo;
use crate::services::prompt as svc;
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
pub struct PromptQuery {
    pub project_id: Option<String>,
    pub session_id: Option<String>,
    pub category: Option<String>,
}

pub async fn list_prompts(
    State(pool): State<SqlitePool>,
    Query(q): Query<PromptQuery>,
) -> impl IntoResponse {
    match svc::list(&pool, q.project_id.as_deref(), q.session_id.as_deref(), q.category.as_deref()).await {
        Ok(prompts) => ok_list(prompts).into_response(),
        Err(e) => err_internal("prompt", e).into_response(),
    }
}

pub async fn get_prompt(
    State(pool): State<SqlitePool>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match svc::get(&pool, &id).await {
        Ok(Some(p)) => ok(p).into_response(),
        Ok(None) => err(StatusCode::NOT_FOUND, errors::PROMPT_NOT_FOUND).into_response(),
        Err(e) => err_internal("prompt", e).into_response(),
    }
}

pub async fn create_prompt(
    State(pool): State<SqlitePool>,
    Json(payload): Json<CreatePromptRequest>,
) -> impl IntoResponse {
    if let Err(e) = validate(&payload) {
        return e.into_response();
    }

    match svc::create(&pool, &payload).await {
        Ok(p) => ok(p).into_response(),
        Err(e) => err_internal("prompt", e).into_response(),
    }
}

pub async fn update_prompt(
    State(pool): State<SqlitePool>,
    Path(id): Path<String>,
    Json(payload): Json<UpdatePromptRequest>,
) -> impl IntoResponse {
    if let Err(e) = validate(&payload) {
        return e.into_response();
    }

    let existing = prompt_repo::find_by_id(&pool, &id).await;
    let prompt = match existing {
        Ok(Some(p)) => p,
        Ok(None) => return err(StatusCode::NOT_FOUND, errors::PROMPT_NOT_FOUND).into_response(),
        Err(e) => return err_internal("prompt", e).into_response(),
    };

    match svc::update(&pool, &id, &payload, prompt).await {
        Ok(p) => ok(p).into_response(),
        Err(e) => err_internal("prompt", e).into_response(),
    }
}

pub async fn delete_prompt(
    State(pool): State<SqlitePool>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match svc::delete(&pool, &id).await {
        Ok(true) => ok(()).into_response(),
        Ok(false) => err(StatusCode::NOT_FOUND, errors::PROMPT_NOT_FOUND).into_response(),
        Err(e) => err_internal("prompt", e).into_response(),
    }
}

