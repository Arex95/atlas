use crate::constants::response::{STATUS_ERROR, STATUS_SUCCESS};
use crate::dtos::ApiResponse;
use axum::Json;
use axum::http::StatusCode;
use serde::Serialize;

pub fn validate<T: garde::Validate>(
    payload: &T,
) -> Result<(), (StatusCode, Json<ApiResponse<()>>)>
where
    T::Context: Default,
{
    payload
        .validate()
        .map_err(|e| err(StatusCode::UNPROCESSABLE_ENTITY, &e.to_string()))
}

pub mod document;
pub mod fs;
pub mod mcp;
pub mod message;
pub mod metric;
pub mod notification;
pub mod orchestrator;
pub mod path_guard;
pub mod project;
pub mod profile;
pub mod prompt;
pub mod reminder;
pub mod session;
pub mod skill;
pub mod system;
pub mod global;
pub mod search;
pub mod task;
pub mod webhook;

pub fn ok<T: Serialize>(data: T) -> Json<ApiResponse<T>> {
    Json(ApiResponse {
        status: STATUS_SUCCESS.into(),
        data: Some(data),
        message: None,
        error: None,
        count: None,
    })
}

pub fn ok_list<T: Serialize>(data: Vec<T>) -> Json<ApiResponse<Vec<T>>> {
    let count = data.len();
    Json(ApiResponse {
        status: STATUS_SUCCESS.into(),
        count: Some(count),
        data: Some(data),
        message: None,
        error: None,
    })
}

pub fn err(code: StatusCode, msg: &str) -> (StatusCode, Json<ApiResponse<()>>) {
    (
        code,
        Json(ApiResponse {
            status: STATUS_ERROR.into(),
            data: None,
            message: Some(msg.to_string()),
            error: Some(msg.to_string()),
            count: None,
        }),
    )
}

pub fn err_internal(scope: &str, e: impl std::fmt::Display) -> (StatusCode, Json<ApiResponse<()>>) {
    tracing::error!("[{}] {}", scope, e);
    err(StatusCode::INTERNAL_SERVER_ERROR, crate::constants::errors::INTERNAL)
}
