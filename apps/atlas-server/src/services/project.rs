use crate::dtos::project::{CreateProjectRequest, ProjectResponse, UpdateProjectRequest};
use crate::repositories::project as repo;
use sqlx::SqlitePool;

pub async fn list(pool: &SqlitePool) -> sqlx::Result<Vec<ProjectResponse>> {
    repo::find_all(pool)
        .await
        .map(|rows| rows.into_iter().map(|p| ProjectResponse::from(p).with_git()).collect())
}

pub async fn create(pool: &SqlitePool, payload: &CreateProjectRequest) -> sqlx::Result<ProjectResponse> {
    let id = ulid::Ulid::new().to_string();
    let desc = payload.description.clone().unwrap_or_default();

    repo::upsert(
        pool,
        &id,
        &payload.slug,
        &payload.name,
        &desc,
        &payload.root_path,
        &payload.index_path,
        payload.color.as_deref(),
    )
    .await
    .map(|p| ProjectResponse::from(p).with_git())
}

pub async fn update(
    pool: &SqlitePool,
    slug: &str,
    payload: &UpdateProjectRequest,
) -> sqlx::Result<Option<ProjectResponse>> {
    let current = repo::find_by_slug(pool, slug).await?;
    let project = match current {
        Some(p) => p,
        None => return Ok(None),
    };

    let name = payload.name.clone().unwrap_or(project.name);
    let description = payload.description.clone().or(project.description);
    let color = payload.color.clone().or(project.color);
    let root_path = payload.root_path.clone().unwrap_or(project.root_path);
    let index_path = payload.index_path.clone().unwrap_or(project.index_path);
    let version = payload.version.clone().unwrap_or(project.version);
    let author = payload.author.clone().or(project.author);

    repo::update(
        pool,
        &project.id,
        &name,
        description.as_deref(),
        color.as_deref(),
        &root_path,
        &index_path,
        &version,
        author.as_deref(),
    )
    .await
    .map(|p| Some(ProjectResponse::from(p).with_git()))
}

pub async fn delete(pool: &SqlitePool, slug: &str) -> sqlx::Result<bool> {
    repo::delete(pool, slug).await
}

pub async fn index(pool: &SqlitePool, slug: &str) -> sqlx::Result<Option<Result<(), String>>> {
    let project = repo::find_by_slug(pool, slug).await?;
    let project = match project {
        Some(p) => p,
        None => return Ok(None),
    };

    let indexer = crate::indexer::ProjectIndexer::new(&project.root_path);
    match indexer.run().await {
        Ok(_) => {
            repo::touch_synced(pool, &project.id).await?;
            Ok(Some(Ok(())))
        }
        Err(e) => Ok(Some(Err(e))),
    }
}
