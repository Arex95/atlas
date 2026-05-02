use crate::constants::errors;
use crate::dtos::skill::{CreateSkillRequest, UpdateSkillRequest};
use crate::handlers::{err, err_internal, ok, ok_list, validate};
use crate::repositories::skill as skill_repo;
use crate::services::skill as svc;
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
pub struct SkillQuery {
    pub project_id: String,
}

pub async fn list_skills(
    State(pool): State<SqlitePool>,
    Query(q): Query<SkillQuery>,
) -> impl IntoResponse {
    match svc::list(&pool, &q.project_id).await {
        Ok(skills) => ok_list(skills).into_response(),
        Err(e) => err_internal("skill", e).into_response(),
    }
}

pub async fn get_skill(
    State(pool): State<SqlitePool>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match svc::get(&pool, &id).await {
        Ok(Some(s)) => ok(s).into_response(),
        Ok(None) => err(StatusCode::NOT_FOUND, errors::SKILL_NOT_FOUND).into_response(),
        Err(e) => err_internal("skill", e).into_response(),
    }
}

pub async fn create_skill(
    State(pool): State<SqlitePool>,
    Json(payload): Json<CreateSkillRequest>,
) -> impl IntoResponse {
    if let Err(e) = validate(&payload) {
        return e.into_response();
    }

    match svc::create(&pool, &payload).await {
        Ok(s) => ok(s).into_response(),
        Err(e) => err_internal("skill", e).into_response(),
    }
}

pub async fn update_skill(
    State(pool): State<SqlitePool>,
    Path(id): Path<String>,
    Json(payload): Json<UpdateSkillRequest>,
) -> impl IntoResponse {
    if let Err(e) = validate(&payload) {
        return e.into_response();
    }

    let existing = skill_repo::find_by_id(&pool, &id).await;
    let skill = match existing {
        Ok(Some(s)) => s,
        Ok(None) => return err(StatusCode::NOT_FOUND, errors::SKILL_NOT_FOUND).into_response(),
        Err(e) => return err_internal("skill", e).into_response(),
    };

    match svc::update(&pool, &id, &payload, skill).await {
        Ok(s) => ok(s).into_response(),
        Err(e) => err_internal("skill", e).into_response(),
    }
}

pub async fn delete_skill(
    State(pool): State<SqlitePool>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match svc::delete(&pool, &id).await {
        Ok(true) => ok(()).into_response(),
        Ok(false) => err(StatusCode::NOT_FOUND, errors::SKILL_NOT_FOUND).into_response(),
        Err(e) => err_internal("skill", e).into_response(),
    }
}

