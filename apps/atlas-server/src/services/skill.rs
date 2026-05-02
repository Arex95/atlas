use crate::dtos::skill::{CreateSkillRequest, SkillResponse, UpdateSkillRequest};
use crate::repositories::skill as repo;
use crate::terminal::TerminalManager;
use sqlx::SqlitePool;
use std::sync::Arc;

pub async fn list(pool: &SqlitePool, project_id: &str) -> sqlx::Result<Vec<SkillResponse>> {
    repo::find_all(pool, project_id)
        .await
        .map(|rows| rows.into_iter().map(SkillResponse::from).collect())
}

pub async fn get(pool: &SqlitePool, id: &str) -> sqlx::Result<Option<SkillResponse>> {
    repo::find_by_id(pool, id)
        .await
        .map(|opt| opt.map(SkillResponse::from))
}

pub async fn create(pool: &SqlitePool, payload: &CreateSkillRequest) -> sqlx::Result<SkillResponse> {
    let id = ulid::Ulid::new().to_string();
    repo::create(pool, &id, &payload.project_id, &payload.name, &payload.description, &payload.script)
        .await
        .map(SkillResponse::from)
}

pub async fn update(
    pool: &SqlitePool,
    id: &str,
    payload: &UpdateSkillRequest,
    existing: crate::models::AgentSkill,
) -> sqlx::Result<SkillResponse> {
    let name = payload.name.clone().unwrap_or(existing.name);
    let description = payload.description.clone().unwrap_or(existing.description);
    let script = payload.script.clone().unwrap_or(existing.script);

    repo::update(pool, id, &name, &description, &script)
        .await
        .map(SkillResponse::from)
}

pub async fn delete(pool: &SqlitePool, id: &str) -> sqlx::Result<bool> {
    repo::delete(pool, id).await
}

pub async fn run(
    pool: &SqlitePool,
    tm: &Arc<TerminalManager>,
    skill_id: &str,
    target_session: &str,
    caller_project_id: Option<&str>,
) -> Result<String, String> {
    let skill = repo::find_with_global(pool, skill_id)
        .await
        .map_err(|e| e.to_string())?;

    match skill {
        Some(s) => {
            if !s.is_global
                && let Some(my_pid) = caller_project_id
                && s.project_id.as_deref() != Some(my_pid)
            {
                return Err("Access denied: skill belongs to a different project".to_string());
            }

            let script_with_newline = if s.script.ends_with('\n') {
                s.script.clone()
            } else {
                format!("{}\n", s.script)
            };

            tm.force_write_input(target_session, &script_with_newline).await;

            let _ = repo::increment_usage(pool, skill_id, s.is_global).await;

            Ok(format!("Skill '{}' executed in session {}", s.name, target_session))
        }
        None => Err(crate::constants::errors::SKILL_NOT_FOUND.to_string()),
    }
}
