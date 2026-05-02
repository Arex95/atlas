use axum::extract::Multipart;
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::IntoResponse;
use axum::Json;

use crate::constants::env;
use crate::dtos::ApiResponse;

const SQLITE_MAGIC: &[u8] = b"SQLite format 3\0";

fn db_file_path() -> String {
    let url = std::env::var(env::DATABASE_URL)
        .unwrap_or_else(|_| "sqlite:./atlas-data/atlas.db".to_string());

    if let Some(rest) = url.strip_prefix("sqlite:///") {
        // sqlite:///absolute/path  =>  /absolute/path
        format!("/{rest}")
    } else if let Some(rest) = url.strip_prefix("sqlite://") {
        rest.to_string()
    } else if let Some(rest) = url.strip_prefix("sqlite:") {
        rest.to_string()
    } else {
        url
    }
}

pub async fn export_db() -> impl IntoResponse {
    let path = db_file_path();
    match tokio::fs::read(&path).await {
        Ok(data) => {
            let mut headers = HeaderMap::new();
            headers.insert(header::CONTENT_TYPE, HeaderValue::from_static("application/octet-stream"));
            headers.insert(
                header::CONTENT_DISPOSITION,
                HeaderValue::from_static("attachment; filename=\"atlas.db\""),
            );
            (headers, data).into_response()
        }
        Err(e) => {
            tracing::error!("[system] export_db read '{}': {}", path, e);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

pub async fn import_db(mut multipart: Multipart) -> impl IntoResponse {
    let path = db_file_path();

    if let Ok(Some(field)) = multipart.next_field().await {
        let data = match field.bytes().await {
            Ok(b) => b,
            Err(e) => {
                tracing::error!("[system] import_db read field: {}", e);
                return (
                    StatusCode::BAD_REQUEST,
                    Json(ApiResponse::<()> {
                        status: "error".into(),
                        data: None,
                        message: Some("Failed to read uploaded file".into()),
                        error: Some("Failed to read uploaded file".into()),
                        count: None,
                    }),
                )
                    .into_response();
            }
        };

        if !data.starts_with(SQLITE_MAGIC) {
            return (
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::<()> {
                    status: "error".into(),
                    data: None,
                    message: Some("File is not a valid SQLite database".into()),
                    error: Some("Invalid SQLite file".into()),
                    count: None,
                }),
            )
                .into_response();
        }

        let backup_path = format!("{path}.bak");
        if let Err(e) = tokio::fs::copy(&path, &backup_path).await {
            tracing::warn!("[system] import_db backup failed: {}", e);
        }

        if let Err(e) = tokio::fs::write(&path, &data).await {
            tracing::error!("[system] import_db write '{}': {}", path, e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::<()> {
                    status: "error".into(),
                    data: None,
                    message: Some("Failed to write database file".into()),
                    error: Some("Write failed".into()),
                    count: None,
                }),
            )
                .into_response();
        }

        tracing::info!("[system] import_db: wrote {} bytes to {}", data.len(), path);
        return (
            StatusCode::OK,
            Json(ApiResponse::<String> {
                status: "success".into(),
                data: Some("Database imported. Restart the server to apply changes.".into()),
                message: Some("Database imported. Restart the server to apply changes.".into()),
                error: None,
                count: None,
            }),
        )
            .into_response();
    }

    (
        StatusCode::BAD_REQUEST,
        Json(ApiResponse::<()> {
            status: "error".into(),
            data: None,
            message: Some("No file received".into()),
            error: Some("No file".into()),
            count: None,
        }),
    )
        .into_response()
}
