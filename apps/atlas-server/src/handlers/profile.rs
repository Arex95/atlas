use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use sqlx::SqlitePool;

use crate::dtos::profile::{ProfileResponse, UpdateProfileRequest};
use crate::handlers::{err_internal, ok, validate};
use crate::services::profile as svc;

pub async fn get_profile(
    State(pool): State<SqlitePool>,
) -> Result<Json<crate::dtos::ApiResponse<ProfileResponse>>, (StatusCode, Json<crate::dtos::ApiResponse<()>>)> {
    svc::get(&pool)
        .await
        .map(ok)
        .map_err(|e| err_internal("get_profile", e))
}

pub async fn update_profile(
    State(pool): State<SqlitePool>,
    Json(body): Json<UpdateProfileRequest>,
) -> Result<Json<crate::dtos::ApiResponse<ProfileResponse>>, (StatusCode, Json<crate::dtos::ApiResponse<()>>)> {
    validate(&body)?;
    svc::update(&pool, &body)
        .await
        .map(ok)
        .map_err(|e| err_internal("update_profile", e))
}
