#[derive(sqlx::FromRow, Clone, Debug)]
pub struct ProjectDocument {
    pub id: String,
    pub project_id: String,
    pub title: String,
    pub content: String,
    #[sqlx(rename = "kind")]
    pub kind: String,
    pub tags: String,
    pub created_at: String,
    pub updated_at: String,
}
