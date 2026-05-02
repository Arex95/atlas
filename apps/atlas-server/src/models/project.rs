#[derive(sqlx::FromRow, Clone, Debug)]
pub struct Project {
    pub id: String,
    pub slug: String,
    pub name: String,
    pub description: Option<String>,
    pub status: String,
    pub root_path: String,
    pub index_path: String,
    pub deadline: Option<String>,
    pub tags: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub last_synced_at: Option<String>,
    pub author: Option<String>,
    pub version: String,
    pub color: Option<String>,
}
