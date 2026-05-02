use crate::dtos::profile::{ProfileResponse, UpdateProfileRequest};
use crate::repositories::profile as repo;
use sqlx::SqlitePool;

pub async fn get(pool: &SqlitePool) -> sqlx::Result<ProfileResponse> {
    repo::get(pool).await.map(ProfileResponse::from)
}

pub async fn update(pool: &SqlitePool, body: &UpdateProfileRequest) -> sqlx::Result<ProfileResponse> {
    repo::update(
        pool,
        &body.name,
        &body.title,
        &body.email,
        &body.github,
        &body.website,
        &body.avatar_color,
    )
    .await
    .map(ProfileResponse::from)
}
