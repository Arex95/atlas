use crate::repositories::global::{self as repo, GlobalMemoryRow, GlobalPromptRow, GlobalSkillRow};
use sqlx::SqlitePool;

pub async fn list_memory(pool: &SqlitePool) -> sqlx::Result<Vec<GlobalMemoryRow>> {
    repo::list_memory(pool).await
}

pub async fn upsert_memory(
    pool: &SqlitePool,
    key: &str,
    value: &str,
    description: &str,
) -> sqlx::Result<GlobalMemoryRow> {
    let id = ulid::Ulid::new().to_string();
    repo::upsert_memory(pool, &id, key, value, description).await
}

pub async fn delete_memory(pool: &SqlitePool, key: &str) -> sqlx::Result<bool> {
    repo::delete_memory(pool, key).await
}

pub async fn list_skills(pool: &SqlitePool) -> sqlx::Result<Vec<GlobalSkillRow>> {
    repo::list_skills(pool).await
}

pub async fn create_skill(
    pool: &SqlitePool,
    name: &str,
    description: &str,
    trigger: Option<&str>,
    script: &str,
) -> sqlx::Result<GlobalSkillRow> {
    let id = ulid::Ulid::new().to_string();
    repo::create_skill(pool, &id, name, description, trigger, script).await
}

pub async fn update_skill(
    pool: &SqlitePool,
    id: &str,
    name: Option<&str>,
    description: Option<&str>,
    trigger: Option<&str>,
    script: Option<&str>,
) -> sqlx::Result<Option<GlobalSkillRow>> {
    repo::update_skill(pool, id, name, description, trigger, script).await
}

pub async fn delete_skill(pool: &SqlitePool, id: &str) -> sqlx::Result<bool> {
    repo::delete_skill(pool, id).await
}

pub async fn list_prompts(pool: &SqlitePool) -> sqlx::Result<Vec<GlobalPromptRow>> {
    repo::list_prompts(pool).await
}

pub async fn create_prompt(
    pool: &SqlitePool,
    title: &str,
    content: &str,
) -> sqlx::Result<GlobalPromptRow> {
    let id = ulid::Ulid::new().to_string();
    repo::create_prompt(pool, &id, title, content).await
}

pub async fn update_prompt(
    pool: &SqlitePool,
    id: &str,
    title: Option<&str>,
    content: Option<&str>,
) -> sqlx::Result<Option<GlobalPromptRow>> {
    repo::update_prompt(pool, id, title, content).await
}

pub async fn delete_prompt(pool: &SqlitePool, id: &str) -> sqlx::Result<bool> {
    repo::delete_prompt(pool, id).await
}
