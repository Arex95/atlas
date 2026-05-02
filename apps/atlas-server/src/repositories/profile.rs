use crate::models::UserProfile;
use sqlx::SqlitePool;

pub async fn get(pool: &SqlitePool) -> sqlx::Result<UserProfile> {
    sqlx::query_as::<_, UserProfile>(
        "SELECT id as _id, name, title, email, github, website, avatar_color, updated_at FROM user_profile WHERE id = 'default'",
    )
    .fetch_one(pool)
    .await
}

pub async fn update(
    pool: &SqlitePool,
    name: &str,
    title: &str,
    email: &str,
    github: &str,
    website: &str,
    avatar_color: &str,
) -> sqlx::Result<UserProfile> {
    sqlx::query(
        "UPDATE user_profile SET name = ?1, title = ?2, email = ?3, github = ?4, website = ?5, avatar_color = ?6, updated_at = datetime('now') WHERE id = 'default'",
    )
    .bind(name)
    .bind(title)
    .bind(email)
    .bind(github)
    .bind(website)
    .bind(avatar_color)
    .execute(pool)
    .await?;

    get(pool).await
}
