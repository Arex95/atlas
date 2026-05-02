use crate::constants::errors;
use crate::dtos::document::{CreateDocumentRequest, UpdateDocumentRequest};
use crate::handlers::{err, err_internal, ok, ok_list, validate};
use crate::repositories::document as doc_repo;
use crate::services::document as svc;
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
pub struct DocumentQuery {
    pub project_id: String,
    pub kind: Option<String>,
}

pub async fn list_documents(
    State(pool): State<SqlitePool>,
    Query(q): Query<DocumentQuery>,
) -> impl IntoResponse {
    match svc::list(&pool, &q.project_id, q.kind.as_deref()).await {
        Ok(docs) => ok_list(docs).into_response(),
        Err(e) => err_internal("document", e).into_response(),
    }
}

pub async fn get_document(
    State(pool): State<SqlitePool>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match svc::get(&pool, &id).await {
        Ok(Some(doc)) => ok(doc).into_response(),
        Ok(None) => err(StatusCode::NOT_FOUND, errors::DOCUMENT_NOT_FOUND).into_response(),
        Err(e) => err_internal("document", e).into_response(),
    }
}

pub async fn create_document(
    State(pool): State<SqlitePool>,
    Json(payload): Json<CreateDocumentRequest>,
) -> impl IntoResponse {
    if let Err(e) = validate(&payload) {
        return e.into_response();
    }

    match svc::create(&pool, &payload).await {
        Ok(doc) => ok(doc).into_response(),
        Err(e) => err_internal("document", e).into_response(),
    }
}

pub async fn update_document(
    State(pool): State<SqlitePool>,
    Path(id): Path<String>,
    Json(payload): Json<UpdateDocumentRequest>,
) -> impl IntoResponse {
    if let Err(e) = validate(&payload) {
        return e.into_response();
    }

    let existing = doc_repo::find_by_id(&pool, &id).await;
    let doc = match existing {
        Ok(Some(d)) => d,
        Ok(None) => return err(StatusCode::NOT_FOUND, errors::DOCUMENT_NOT_FOUND).into_response(),
        Err(e) => return err_internal("document", e).into_response(),
    };

    match svc::update(&pool, &id, &payload, doc).await {
        Ok(updated) => ok(updated).into_response(),
        Err(e) => err_internal("document", e).into_response(),
    }
}

pub async fn delete_document(
    State(pool): State<SqlitePool>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match svc::delete(&pool, &id).await {
        Ok(true) => ok(()).into_response(),
        Ok(false) => err(StatusCode::NOT_FOUND, errors::DOCUMENT_NOT_FOUND).into_response(),
        Err(e) => err_internal("document", e).into_response(),
    }
}

