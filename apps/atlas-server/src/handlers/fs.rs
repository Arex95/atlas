use crate::constants::errors;
use crate::handlers::{
    err, ok,
    path_guard::{PathGuardError, validate_path_in_projects},
};
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

#[derive(Deserialize)]
pub struct ListFilesQuery {
    pub path: String,
}

#[derive(Serialize)]
pub struct FileItem {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub size: u64,
}

fn map_guard_error(e: PathGuardError) -> (StatusCode, axum::Json<crate::dtos::ApiResponse<()>>) {
    match e {
        PathGuardError::DoesNotExist => err(StatusCode::NOT_FOUND, errors::PATH_DOES_NOT_EXIST),
        PathGuardError::OutsideProjectRoots => {
            err(StatusCode::FORBIDDEN, errors::PATH_OUTSIDE_ROOT)
        }
        PathGuardError::Db(msg) => err(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("Path validation failed: {}", msg),
        ),
    }
}

const MAX_DIR_ENTRIES: usize = 1_000;
const MAX_FILE_READ_BYTES: u64 = 5 * 1024 * 1024;

pub async fn list_files(
    State(pool): State<SqlitePool>,
    Query(query): Query<ListFilesQuery>,
) -> impl IntoResponse {
    let safe_path = match validate_path_in_projects(&pool, &query.path).await {
        Ok(p) => p,
        Err(e) => return map_guard_error(e).into_response(),
    };

    if !safe_path.is_dir() {
        return err(StatusCode::BAD_REQUEST, errors::PATH_NOT_DIRECTORY).into_response();
    }

    let items = tokio::task::spawn_blocking(move || {
        let mut items = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&safe_path) {
            for entry in entries.flatten().take(MAX_DIR_ENTRIES) {
                let path = entry.path();
                let metadata = entry.metadata().ok();
                items.push(FileItem {
                    name: entry.file_name().to_string_lossy().to_string(),
                    path: path.to_string_lossy().to_string(),
                    is_dir: path.is_dir(),
                    size: metadata.map(|m| m.len()).unwrap_or(0),
                });
            }
        }
        items.sort_by(|a, b| {
            if a.is_dir != b.is_dir {
                b.is_dir.cmp(&a.is_dir)
            } else {
                a.name.to_lowercase().cmp(&b.name.to_lowercase())
            }
        });
        items
    })
    .await
    .unwrap_or_default();

    ok(items).into_response()
}

#[derive(Deserialize)]
pub struct ReadFileQuery {
    pub path: String,
}

pub async fn read_file(
    State(pool): State<SqlitePool>,
    Query(query): Query<ReadFileQuery>,
) -> impl IntoResponse {
    let safe_path = match validate_path_in_projects(&pool, &query.path).await {
        Ok(p) => p,
        Err(e) => return map_guard_error(e).into_response(),
    };

    if safe_path.is_dir() {
        return err(StatusCode::BAD_REQUEST, errors::CANNOT_READ_DIRECTORY).into_response();
    }

    match tokio::fs::metadata(&safe_path).await {
        Ok(meta) if meta.len() > MAX_FILE_READ_BYTES => {
            return err(StatusCode::PAYLOAD_TOO_LARGE, errors::FILE_TOO_LARGE).into_response();
        }
        Err(_) => {
            return err(StatusCode::NOT_FOUND, errors::PATH_DOES_NOT_EXIST).into_response();
        }
        Ok(_) => {}
    }

    match tokio::fs::read_to_string(&safe_path).await {
        Ok(content) => ok(content).into_response(),
        Err(e) => {
            tracing::error!("[fs] read_file({:?}) failed: {}", safe_path, e);
            err(StatusCode::INTERNAL_SERVER_ERROR, errors::READ_FILE_FAILED).into_response()
        }
    }
}
