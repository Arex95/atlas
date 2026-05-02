use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use serde::Deserialize;
use sqlx::SqlitePool;

use crate::dtos::task::{CreateTaskRequest, TaskResponse, UpdateTaskRequest};
use crate::handlers::{err, err_internal, ok, ok_list, validate};
use crate::services::task as svc;

#[derive(Deserialize)]
pub struct ListTasksQuery {
    pub project_id: Option<String>,
    pub status: Option<String>,
    pub parent_id: Option<String>,
}

pub async fn list_tasks(
    State(pool): State<SqlitePool>,
    Query(q): Query<ListTasksQuery>,
) -> Result<Json<crate::dtos::ApiResponse<Vec<TaskResponse>>>, (StatusCode, Json<crate::dtos::ApiResponse<()>>)> {
    svc::list(&pool, q.project_id.as_deref(), q.status.as_deref(), q.parent_id.as_deref())
        .await
        .map(ok_list)
        .map_err(|e| err_internal("list_tasks", e))
}

pub async fn create_task(
    State(pool): State<SqlitePool>,
    Json(body): Json<CreateTaskRequest>,
) -> Result<Json<crate::dtos::ApiResponse<TaskResponse>>, (StatusCode, Json<crate::dtos::ApiResponse<()>>)> {
    validate(&body)?;
    svc::create(&pool, &body)
        .await
        .map(ok)
        .map_err(|e| err_internal("create_task", e))
}

pub async fn update_task(
    State(pool): State<SqlitePool>,
    Path(id): Path<String>,
    Json(body): Json<UpdateTaskRequest>,
) -> Result<Json<crate::dtos::ApiResponse<TaskResponse>>, (StatusCode, Json<crate::dtos::ApiResponse<()>>)> {
    validate(&body)?;
    match svc::update(&pool, &id, &body).await.map_err(|e| err_internal("update_task", e))? {
        Some(t) => Ok(ok(t)),
        None => Err((StatusCode::NOT_FOUND, Json(crate::dtos::ApiResponse {
            status: "error".to_string(),
            data: None,
            message: Some("Task not found".to_string()),
            error: None,
            count: None,
        }))),
    }
}

pub async fn delete_task(
    State(pool): State<SqlitePool>,
    Path(id): Path<String>,
) -> Result<Json<crate::dtos::ApiResponse<()>>, (StatusCode, Json<crate::dtos::ApiResponse<()>>)> {
    match svc::delete(&pool, &id).await.map_err(|e| err_internal("delete_task", e))? {
        true => Ok(Json(crate::dtos::ApiResponse {
            status: "success".to_string(),
            data: None,
            message: Some("Task deleted".to_string()),
            error: None,
            count: None,
        })),
        false => Err(err(StatusCode::NOT_FOUND, "Task not found")),
    }
}
