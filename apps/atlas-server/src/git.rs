use serde::Serialize;
use std::collections::HashMap;
use std::process::Command;
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GitInfo {
    pub branch: String,
    pub has_changes: bool,
    pub insertions: i32,
    pub deletions: i32,
    pub staged: i32,
    pub untracked: i32,
    pub commit_hash: String,
    pub last_commit_message: String,
    pub remote_url: Option<String>,
    pub remote_name: Option<String>,
    pub user_name: Option<String>,
    pub user_email: Option<String>,
    pub ahead: i32,
    pub behind: i32,
    pub stash_count: i32,
}

type GitCacheEntry = (Instant, Option<GitInfo>);

const GIT_CACHE_TTL: Duration = Duration::from_secs(5);
static GIT_CACHE: LazyLock<Mutex<HashMap<String, GitCacheEntry>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

pub fn get_git_info(path: &str) -> Option<GitInfo> {
    if let Ok(cache) = GIT_CACHE.lock()
        && let Some((ts, info)) = cache.get(path)
        && ts.elapsed() < GIT_CACHE_TTL
    {
        return info.clone();
    }

    let info = compute_git_info(path);

    if let Ok(mut cache) = GIT_CACHE.lock() {
        cache.insert(path.to_string(), (Instant::now(), info.clone()));
    }
    info
}

fn compute_git_info(path: &str) -> Option<GitInfo> {
    let status = Command::new("git")
        .arg("-C")
        .arg(path)
        .arg("rev-parse")
        .arg("--is-inside-work-tree")
        .output()
        .ok()?;

    if !status.status.success() {
        return None;
    }

    let branch = Command::new("git")
        .arg("-C")
        .arg(path)
        .arg("rev-parse")
        .arg("--abbrev-ref")
        .arg("HEAD")
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    let status_output = Command::new("git")
        .arg("-C")
        .arg(path)
        .arg("status")
        .arg("--porcelain")
        .output()
        .ok()?;

    let has_changes = !status_output.stdout.is_empty();
    let status_str = String::from_utf8_lossy(&status_output.stdout);
    let mut staged = 0i32;
    let mut untracked = 0i32;
    for line in status_str.lines() {
        if line.starts_with("??") {
            untracked += 1;
        } else if !line.is_empty() && !line.starts_with(' ') {
            staged += 1;
        }
    }

    let diff_output = Command::new("git")
        .arg("-C")
        .arg(path)
        .arg("diff")
        .arg("--numstat")
        .output()
        .ok()?;

    let mut insertions = 0;
    let mut deletions = 0;
    let diff_str = String::from_utf8_lossy(&diff_output.stdout);
    for line in diff_str.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 2 {
            insertions += parts[0].parse::<i32>().unwrap_or(0);
            deletions += parts[1].parse::<i32>().unwrap_or(0);
        }
    }

    let log_output = Command::new("git")
        .arg("-C")
        .arg(path)
        .arg("log")
        .arg("-1")
        .arg("--format=%h|%s")
        .output()
        .ok()?;

    let log_str = String::from_utf8_lossy(&log_output.stdout);
    let log_parts: Vec<&str> = log_str.trim().splitn(2, '|').collect();
    let commit_hash = log_parts.first().unwrap_or(&"").to_string();
    let last_commit_message = log_parts.get(1).unwrap_or(&"").to_string();

    let remote_url = Command::new("git")
        .arg("-C")
        .arg(path)
        .arg("remote")
        .arg("get-url")
        .arg("origin")
        .output()
        .ok()
        .and_then(|o| if o.status.success() { Some(String::from_utf8_lossy(&o.stdout).trim().to_string()) } else { None })
        .filter(|s| !s.is_empty());

    let remote_name = remote_url.as_ref().map(|_| "origin".to_string());

    let user_name = Command::new("git")
        .arg("-C")
        .arg(path)
        .arg("config")
        .arg("user.name")
        .output()
        .ok()
        .and_then(|o| if o.status.success() { Some(String::from_utf8_lossy(&o.stdout).trim().to_string()) } else { None })
        .filter(|s| !s.is_empty());

    let user_email = Command::new("git")
        .arg("-C")
        .arg(path)
        .arg("config")
        .arg("user.email")
        .output()
        .ok()
        .and_then(|o| if o.status.success() { Some(String::from_utf8_lossy(&o.stdout).trim().to_string()) } else { None })
        .filter(|s| !s.is_empty());

    let ahead = Command::new("git")
        .arg("-C")
        .arg(path)
        .arg("rev-list")
        .arg("--count")
        .arg("@{u}..HEAD")
        .output()
        .ok()
        .and_then(|o| if o.status.success() { String::from_utf8_lossy(&o.stdout).trim().parse::<i32>().ok() } else { None })
        .unwrap_or(0);

    let behind = Command::new("git")
        .arg("-C")
        .arg(path)
        .arg("rev-list")
        .arg("--count")
        .arg("HEAD..@{u}")
        .output()
        .ok()
        .and_then(|o| if o.status.success() { String::from_utf8_lossy(&o.stdout).trim().parse::<i32>().ok() } else { None })
        .unwrap_or(0);

    let stash_count = Command::new("git")
        .arg("-C")
        .arg(path)
        .arg("stash")
        .arg("list")
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).lines().count() as i32)
        .unwrap_or(0);

    Some(GitInfo {
        branch,
        has_changes,
        insertions,
        deletions,
        staged,
        untracked,
        commit_hash,
        last_commit_message,
        remote_url,
        remote_name,
        user_name,
        user_email,
        ahead,
        behind,
        stash_count,
    })
}
