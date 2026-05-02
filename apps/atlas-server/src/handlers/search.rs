use crate::handlers::ok;
use axum::{
    extract::{Query, State},
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

#[derive(Deserialize)]
pub struct SearchQuery {
    pub q: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    pub kind: String,
    pub id: String,
    pub title: String,
    pub subtitle: Option<String>,
    pub url: Option<String>,
}

pub async fn global_search(
    State(pool): State<SqlitePool>,
    Query(q): Query<SearchQuery>,
) -> impl IntoResponse {
    if q.q.trim().is_empty() {
        return ok(Vec::<SearchResult>::new()).into_response();
    }

    let pattern = format!("%{}%", q.q.to_lowercase());
    let mut results: Vec<SearchResult> = Vec::new();

    #[derive(sqlx::FromRow)]
    struct ProjectRow { id: String, name: String, slug: String, description: Option<String> }
    if let Ok(rows) = sqlx::query_as::<_, ProjectRow>(
        "SELECT id, name, slug, description FROM projects WHERE lower(name) LIKE ? OR lower(slug) LIKE ? LIMIT 5",
    )
    .bind(&pattern)
    .bind(&pattern)
    .fetch_all(&pool)
    .await
    {
        for r in rows {
            results.push(SearchResult {
                kind: "project".into(),
                id: r.id,
                title: r.name,
                subtitle: r.description,
                url: Some(format!("/projects/{}", r.slug)),
            });
        }
    }

    #[derive(sqlx::FromRow)]
    struct TaskRow { id: String, title: String, status: String }
    if let Ok(rows) = sqlx::query_as::<_, TaskRow>(
        "SELECT id, title, status FROM tasks WHERE lower(title) LIKE ? OR lower(description) LIKE ? LIMIT 5",
    )
    .bind(&pattern)
    .bind(&pattern)
    .fetch_all(&pool)
    .await
    {
        for r in rows {
            results.push(SearchResult {
                kind: "task".into(),
                id: r.id.clone(),
                title: r.title,
                subtitle: Some(r.status),
                url: None,
            });
        }
    }

    #[derive(sqlx::FromRow)]
    struct SessionRow { id: String, title: Option<String>, custom_name: Option<String>, status: String }
    if let Ok(rows) = sqlx::query_as::<_, SessionRow>(
        "SELECT id, title, custom_name, status FROM ai_sessions WHERE lower(coalesce(custom_name, title, '')) LIKE ? LIMIT 5",
    )
    .bind(&pattern)
    .fetch_all(&pool)
    .await
    {
        for r in rows {
            let display = r.custom_name.or(r.title).unwrap_or_else(|| r.id.clone());
            results.push(SearchResult {
                kind: "session".into(),
                id: r.id,
                title: display,
                subtitle: Some(r.status),
                url: None,
            });
        }
    }

    #[derive(sqlx::FromRow)]
    struct DocRow { id: String, title: String, kind: String }
    if let Ok(rows) = sqlx::query_as::<_, DocRow>(
        "SELECT id, title, kind FROM project_documents WHERE lower(title) LIKE ? OR lower(content) LIKE ? LIMIT 5",
    )
    .bind(&pattern)
    .bind(&pattern)
    .fetch_all(&pool)
    .await
    {
        for r in rows {
            results.push(SearchResult {
                kind: "document".into(),
                id: r.id,
                title: r.title,
                subtitle: Some(r.kind),
                url: None,
            });
        }
    }

    ok(results).into_response()
}
