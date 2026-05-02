use crate::handlers::{err, err_internal, ok, ok_list};
use crate::services::global as svc;
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde::Deserialize;
use sqlx::SqlitePool;

pub async fn list_global_memory(State(pool): State<SqlitePool>) -> impl IntoResponse {
    match svc::list_memory(&pool).await {
        Ok(rows) => ok_list(rows).into_response(),
        Err(e) => err_internal("global_memory", e).into_response(),
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpsertMemoryRequest {
    pub key: String,
    pub value: String,
    pub description: Option<String>,
}

pub async fn upsert_global_memory(
    State(pool): State<SqlitePool>,
    Json(payload): Json<UpsertMemoryRequest>,
) -> impl IntoResponse {
    if payload.key.trim().is_empty() {
        return err(StatusCode::UNPROCESSABLE_ENTITY, "Key is required").into_response();
    }
    let description = payload.description.as_deref().unwrap_or("");
    match svc::upsert_memory(&pool, &payload.key, &payload.value, description).await {
        Ok(r) => ok(r).into_response(),
        Err(e) => err_internal("global_memory", e).into_response(),
    }
}

pub async fn delete_global_memory(
    State(pool): State<SqlitePool>,
    Path(key): Path<String>,
) -> impl IntoResponse {
    match svc::delete_memory(&pool, &key).await {
        Ok(true) => ok(()).into_response(),
        Ok(false) => err(StatusCode::NOT_FOUND, "Key not found").into_response(),
        Err(e) => err_internal("global_memory", e).into_response(),
    }
}

pub async fn list_global_skills(State(pool): State<SqlitePool>) -> impl IntoResponse {
    match svc::list_skills(&pool).await {
        Ok(rows) => ok_list(rows).into_response(),
        Err(e) => err_internal("global_skills", e).into_response(),
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSkillRequest {
    pub name: String,
    pub description: Option<String>,
    pub trigger: Option<String>,
    pub script: Option<String>,
}

pub async fn create_global_skill(
    State(pool): State<SqlitePool>,
    Json(payload): Json<CreateSkillRequest>,
) -> impl IntoResponse {
    if payload.name.trim().is_empty() {
        return err(StatusCode::UNPROCESSABLE_ENTITY, "Name is required").into_response();
    }
    match svc::create_skill(
        &pool,
        &payload.name,
        payload.description.as_deref().unwrap_or(""),
        payload.trigger.as_deref(),
        payload.script.as_deref().unwrap_or(""),
    )
    .await
    {
        Ok(r) => ok(r).into_response(),
        Err(e) => err_internal("global_skills", e).into_response(),
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSkillRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub trigger: Option<String>,
    pub script: Option<String>,
}

pub async fn update_global_skill(
    State(pool): State<SqlitePool>,
    Path(id): Path<String>,
    Json(payload): Json<UpdateSkillRequest>,
) -> impl IntoResponse {
    match svc::update_skill(
        &pool,
        &id,
        payload.name.as_deref(),
        payload.description.as_deref(),
        payload.trigger.as_deref(),
        payload.script.as_deref(),
    )
    .await
    {
        Ok(Some(r)) => ok(r).into_response(),
        Ok(None) => err(StatusCode::NOT_FOUND, "Global skill not found").into_response(),
        Err(e) => err_internal("global_skills", e).into_response(),
    }
}

pub async fn delete_global_skill(
    State(pool): State<SqlitePool>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match svc::delete_skill(&pool, &id).await {
        Ok(true) => ok(()).into_response(),
        Ok(false) => err(StatusCode::NOT_FOUND, "Global skill not found").into_response(),
        Err(e) => err_internal("global_skills", e).into_response(),
    }
}

pub async fn list_global_prompts(State(pool): State<SqlitePool>) -> impl IntoResponse {
    match svc::list_prompts(&pool).await {
        Ok(rows) => ok_list(rows).into_response(),
        Err(e) => err_internal("global_prompts", e).into_response(),
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatePromptRequest {
    pub title: String,
    pub content: Option<String>,
}

pub async fn create_global_prompt(
    State(pool): State<SqlitePool>,
    Json(payload): Json<CreatePromptRequest>,
) -> impl IntoResponse {
    if payload.title.trim().is_empty() {
        return err(StatusCode::UNPROCESSABLE_ENTITY, "Title is required").into_response();
    }
    match svc::create_prompt(&pool, &payload.title, payload.content.as_deref().unwrap_or("")).await {
        Ok(r) => ok(r).into_response(),
        Err(e) => err_internal("global_prompts", e).into_response(),
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdatePromptRequest {
    pub title: Option<String>,
    pub content: Option<String>,
}

pub async fn update_global_prompt(
    State(pool): State<SqlitePool>,
    Path(id): Path<String>,
    Json(payload): Json<UpdatePromptRequest>,
) -> impl IntoResponse {
    match svc::update_prompt(&pool, &id, payload.title.as_deref(), payload.content.as_deref()).await {
        Ok(Some(r)) => ok(r).into_response(),
        Ok(None) => err(StatusCode::NOT_FOUND, "Global prompt not found").into_response(),
        Err(e) => err_internal("global_prompts", e).into_response(),
    }
}

pub async fn delete_global_prompt(
    State(pool): State<SqlitePool>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match svc::delete_prompt(&pool, &id).await {
        Ok(true) => ok(()).into_response(),
        Ok(false) => err(StatusCode::NOT_FOUND, "Global prompt not found").into_response(),
        Err(e) => err_internal("global_prompts", e).into_response(),
    }
}
