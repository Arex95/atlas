use crate::dtos::session::{AiSessionResponse, CreateSessionRequest, ConversationTurnResponse};
use crate::repositories::session as repo;
use sqlx::SqlitePool;

pub async fn create(
    pool: &SqlitePool,
    slug: &str,
    payload: &CreateSessionRequest,
    default_author: &str,
) -> sqlx::Result<Option<AiSessionResponse>> {
    let project = repo::find_project_by_slug(pool, slug).await?;
    let project_id = match project {
        Some(p) => p.id,
        None => return Ok(None),
    };

    let id = ulid::Ulid::new().to_string();

    repo::create(
        pool,
        &id,
        &project_id,
        &payload.provider,
        &payload.model,
        &payload.mode,
        &payload.working_directory,
        Some(payload.title.as_str()),
        default_author,
    )
    .await
    .map(|s| Some(AiSessionResponse::from(s).with_git()))
}

pub async fn save(
    pool: &SqlitePool,
    id: &str,
    custom_name: Option<&str>,
    custom_description: Option<&str>,
    color: Option<&str>,
    icon: Option<&str>,
) -> sqlx::Result<AiSessionResponse> {
    repo::save(pool, id, custom_name, custom_description, color, icon)
        .await
        .map(|s| AiSessionResponse::from(s).with_git())
}

pub async fn get_history(pool: &SqlitePool, session_id: &str) -> sqlx::Result<Vec<ConversationTurnResponse>> {
    repo::find_messages(pool, session_id)
        .await
        .map(|msgs| msgs.into_iter().map(ConversationTurnResponse::from).collect())
}

pub async fn create_message(
    pool: &SqlitePool,
    session_id: &str,
    role: &str,
    content: &str,
) -> sqlx::Result<ConversationTurnResponse> {
    let id = ulid::Ulid::new().to_string();
    repo::create_message(pool, &id, session_id, role, content)
        .await
        .map(ConversationTurnResponse::from)
}

pub async fn list_for_project(
    pool: &SqlitePool,
    slug: &str,
) -> sqlx::Result<Option<Vec<AiSessionResponse>>> {
    let project = repo::find_project_by_slug(pool, slug).await?;
    let project_id = match project {
        Some(p) => p.id,
        None => return Ok(None),
    };

    repo::find_by_project_id(pool, &project_id)
        .await
        .map(|sessions| {
            Some(
                sessions
                    .into_iter()
                    .map(|s| AiSessionResponse::from(s).with_git())
                    .collect(),
            )
        })
}

pub async fn list_saved(pool: &SqlitePool) -> sqlx::Result<Vec<AiSessionResponse>> {
    repo::find_saved(pool)
        .await
        .map(|sessions| sessions.into_iter().map(|s| AiSessionResponse::from(s).with_git()).collect())
}

pub async fn update(
    pool: &SqlitePool,
    id: &str,
    payload: &crate::dtos::session::UpdateSessionRequest,
) -> sqlx::Result<Option<AiSessionResponse>> {
    repo::update(
        pool,
        id,
        payload.custom_name.as_deref(),
        payload.custom_description.as_deref(),
        payload.color.as_deref(),
        payload.icon.as_deref(),
        payload.linked_task_id.as_deref(),
    )
    .await
    .map(|opt| opt.map(|s| AiSessionResponse::from(s).with_git()))
}

pub async fn delete(pool: &SqlitePool, id: &str) -> sqlx::Result<()> {
    repo::delete(pool, id).await
}

pub async fn delete_message(pool: &SqlitePool, message_id: &str) -> sqlx::Result<bool> {
    repo::delete_message(pool, message_id).await
}
