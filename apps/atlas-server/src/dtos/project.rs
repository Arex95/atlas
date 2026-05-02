use crate::models::Project;
use garde::Validate;
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeadlineInfo {
    pub date: String,
    pub days_remaining: i64,
    pub is_breached: bool,
    pub urgency: &'static str,
}

impl DeadlineInfo {
    pub fn from_date_str(date: &str) -> Self {
        let days_remaining = Self::calc_days(date);
        let is_breached = days_remaining < 0;
        let urgency = if is_breached || days_remaining <= 3 {
            "critical"
        } else if days_remaining <= 7 {
            "warning"
        } else {
            "normal"
        };
        Self { date: date.to_string(), days_remaining, is_breached, urgency }
    }

    fn calc_days(date_str: &str) -> i64 {
        // Parse YYYY-MM-DD, compute diff from today in days
        let parts: Vec<&str> = date_str.splitn(3, '-').collect();
        if parts.len() < 3 { return 0; }
        let Ok(y) = parts[0].parse::<i32>() else { return 0 };
        let Ok(m) = parts[1].parse::<u32>() else { return 0 };
        let Ok(d) = parts[2].parse::<u32>() else { return 0 };
        // days since epoch for deadline
        let target = days_from_ymd(y, m, d);
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() / 86400)
            .unwrap_or(0) as i64;
        target - now
    }
}

fn days_from_ymd(y: i32, m: u32, d: u32) -> i64 {
    // Gregorian days since Unix epoch (1970-01-01)
    let y = y as i64;
    let m = m as i64;
    let d = d as i64;
    let a = (14 - m) / 12;
    let yr = y + 4800 - a;
    let mo = m + 12 * a - 3;
    let jdn = d + (153 * mo + 2) / 5 + 365 * yr + yr / 4 - yr / 100 + yr / 400 - 32045;
    jdn - 2440588 // subtract Julian day of 1970-01-01
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectResponse {
    pub id: String,
    pub slug: String,
    pub name: String,
    pub description: Option<String>,
    pub status: String,
    pub root_path: String,
    pub index_path: String,
    pub deadline: Option<DeadlineInfo>,
    pub tags: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
    pub last_synced_at: Option<String>,
    pub git: Option<crate::git::GitInfo>,
    pub author: Option<String>,
    pub version: String,
    pub color: Option<String>,
}

impl From<Project> for ProjectResponse {
    fn from(p: Project) -> Self {
        let tags: Vec<String> = p
            .tags
            .as_deref()
            .and_then(|t| serde_json::from_str(t).ok())
            .unwrap_or_default();
        Self {
            id: p.id,
            slug: p.slug,
            name: p.name,
            description: p.description,
            status: p.status,
            root_path: p.root_path,
            index_path: p.index_path,
            deadline: p.deadline.as_deref().map(DeadlineInfo::from_date_str),
            tags,
            created_at: p.created_at,
            updated_at: p.updated_at,
            last_synced_at: p.last_synced_at,
            git: None,
            author: p.author,
            version: p.version,
            color: p.color,
        }
    }
}

impl ProjectResponse {
    pub fn with_git(mut self) -> Self {
        self.git = crate::git::get_git_info(&self.root_path);
        self
    }
}

#[derive(Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct CreateProjectRequest {
    #[garde(length(min = 1, max = 200))]
    pub name: String,
    #[garde(length(min = 1, max = 100), pattern(r"^[a-zA-Z0-9_-]+$"))]
    pub slug: String,
    #[garde(inner(length(max = 2000)))]
    pub description: Option<String>,
    #[garde(length(min = 1, max = 500))]
    pub root_path: String,
    #[garde(length(max = 500))]
    pub index_path: String,
    #[garde(skip)]
    pub color: Option<String>,
}

#[derive(Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProjectRequest {
    #[garde(inner(length(min = 1, max = 200)))]
    pub name: Option<String>,
    #[garde(inner(length(max = 2000)))]
    pub description: Option<String>,
    #[garde(skip)]
    pub color: Option<String>,
    #[garde(inner(length(min = 1, max = 500)))]
    pub root_path: Option<String>,
    #[garde(inner(length(max = 500)))]
    pub index_path: Option<String>,
    #[garde(skip)]
    pub version: Option<String>,
    #[garde(skip)]
    pub author: Option<String>,
}
