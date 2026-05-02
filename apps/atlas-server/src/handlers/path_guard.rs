use crate::constants::errors;
use sqlx::SqlitePool;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub enum PathGuardError {
    DoesNotExist,
    OutsideProjectRoots,
    Db(String),
}

impl std::fmt::Display for PathGuardError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PathGuardError::DoesNotExist => write!(f, "{}", errors::PATH_DOES_NOT_EXIST),
            PathGuardError::OutsideProjectRoots => write!(f, "{}", errors::PATH_OUTSIDE_ROOT),
            PathGuardError::Db(e) => write!(f, "DB error while validating path: {}", e),
        }
    }
}

pub async fn validate_path_in_projects(
    pool: &SqlitePool,
    requested: &str,
) -> Result<PathBuf, PathGuardError> {
    let requested_path = Path::new(requested);
    let canonical = requested_path
        .canonicalize()
        .map_err(|_| PathGuardError::DoesNotExist)?;

    let roots: Vec<(String,)> = sqlx::query_as("SELECT root_path FROM projects")
        .fetch_all(pool)
        .await
        .map_err(|e| PathGuardError::Db(e.to_string()))?;

    for (root,) in roots {
        let root_canonical = match Path::new(&root).canonicalize() {
            Ok(p) => p,
            Err(_) => continue,
        };
        if canonical.starts_with(&root_canonical) {
            return Ok(canonical);
        }
    }

    Err(PathGuardError::OutsideProjectRoots)
}
