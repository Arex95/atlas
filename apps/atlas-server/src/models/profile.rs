#[derive(sqlx::FromRow, Clone, Debug)]
pub struct UserProfile {
    pub _id: String,
    pub name: String,
    pub title: String,
    pub email: String,
    pub github: String,
    pub website: String,
    pub avatar_color: String,
    pub updated_at: String,
}
