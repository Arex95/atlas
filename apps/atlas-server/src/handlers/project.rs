use crate::constants::{errors, response};
use crate::dtos::project::{CreateProjectRequest, UpdateProjectRequest};
use crate::handlers::{err, err_internal, ok, ok_list, validate};
use crate::services::project as svc;
use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};
use sqlx::SqlitePool;

pub async fn get_projects(State(pool): State<SqlitePool>) -> impl IntoResponse {
    match svc::list(&pool).await {
        Ok(projects) => ok_list(projects).into_response(),
        Err(e) => err_internal("projects", e).into_response(),
    }
}

pub async fn create_project(
    State(pool): State<SqlitePool>,
    Json(payload): Json<CreateProjectRequest>,
) -> impl IntoResponse {
    if let Err(rejection) = validate(&payload) {
        return rejection.into_response();
    }

    match svc::create(&pool, &payload).await {
        Ok(project) => ok(project).into_response(),
        Err(e) => {
            tracing::error!("[projects] create failed: {}", e);
            err(StatusCode::INTERNAL_SERVER_ERROR, errors::PROJECT_CREATE_FAILED).into_response()
        }
    }
}

pub async fn update_project(
    State(pool): State<SqlitePool>,
    axum::extract::Path(slug): axum::extract::Path<String>,
    Json(payload): Json<UpdateProjectRequest>,
) -> impl IntoResponse {
    match svc::update(&pool, &slug, &payload).await {
        Ok(Some(updated)) => ok(updated).into_response(),
        Ok(None) => err(StatusCode::NOT_FOUND, errors::PROJECT_NOT_FOUND).into_response(),
        Err(e) => err_internal("projects", e).into_response(),
    }
}

pub async fn delete_project(
    State(pool): State<SqlitePool>,
    axum::extract::Path(slug): axum::extract::Path<String>,
) -> impl IntoResponse {
    match svc::delete(&pool, &slug).await {
        Ok(true) => axum::http::StatusCode::NO_CONTENT.into_response(),
        Ok(false) => err(StatusCode::NOT_FOUND, errors::PROJECT_NOT_FOUND).into_response(),
        Err(e) => err_internal("projects", e).into_response(),
    }
}

pub async fn index_project(
    State(pool): State<SqlitePool>,
    axum::extract::Path(slug): axum::extract::Path<String>,
) -> impl IntoResponse {
    match svc::index(&pool, &slug).await {
        Ok(Some(Ok(()))) => ok(response::PROJECT_INDEXED).into_response(),
        Ok(Some(Err(e))) => err(StatusCode::INTERNAL_SERVER_ERROR, &e).into_response(),
        Ok(None) => err(StatusCode::NOT_FOUND, errors::PROJECT_NOT_FOUND).into_response(),
        Err(e) => err_internal("projects", e).into_response(),
    }
}
