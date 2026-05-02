use garde::Validate;
use serde::{Deserialize, Serialize};

use crate::models::UserProfile;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileResponse {
    pub name: String,
    pub title: String,
    pub email: String,
    pub github: String,
    pub website: String,
    pub avatar_color: String,
    pub updated_at: String,
}

impl From<UserProfile> for ProfileResponse {
    fn from(p: UserProfile) -> Self {
        Self {
            name: p.name,
            title: p.title,
            email: p.email,
            github: p.github,
            website: p.website,
            avatar_color: p.avatar_color,
            updated_at: p.updated_at,
        }
    }
}

#[derive(Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProfileRequest {
    #[garde(length(min = 1, max = 100))]
    pub name: String,
    #[garde(length(max = 100))]
    pub title: String,
    #[garde(length(max = 200))]
    pub email: String,
    #[garde(length(max = 100))]
    pub github: String,
    #[garde(length(max = 500))]
    pub website: String,
    #[garde(length(max = 20))]
    pub avatar_color: String,
}
