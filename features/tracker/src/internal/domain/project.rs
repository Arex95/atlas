use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// A tracker-agnostic project pointer of the shape `owner/repo`.
///
/// `owner` may itself contain further `/`-separated segments —
/// GitLab nests groups arbitrarily deep (`acme/sandbox/widgets`);
/// `repo` is always the last segment. Works for GitHub's flat
/// `owner/repo` too, since that is just the one-subgroup case.
/// Adapters resolve to their own internal ids; the domain never
/// carries a vendor id.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize)]
pub struct ProjectRef {
    owner: String,
    repo: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ProjectRefError {
    #[error("project reference must be at least `owner/repo`, got {0:?}")]
    Malformed(String),
    #[error("project reference has an empty segment: {0:?}")]
    EmptySegment(String),
}

impl ProjectRef {
    pub fn new(owner: impl Into<String>, repo: impl Into<String>) -> Result<Self, ProjectRefError> {
        let owner = owner.into();
        let repo = repo.into();
        if owner.is_empty() || repo.is_empty() {
            return Err(ProjectRefError::EmptySegment(format!("{owner}/{repo}")));
        }
        Ok(Self { owner, repo })
    }

    #[must_use]
    pub fn owner(&self) -> &str {
        &self.owner
    }

    #[must_use]
    pub fn repo(&self) -> &str {
        &self.repo
    }

    /// The canonical `owner/repo` rendering — the wire form for
    /// GitLab's `path_with_namespace` (URL-encoded by the adapter).
    #[must_use]
    pub fn path(&self) -> String {
        format!("{}/{}", self.owner, self.repo)
    }
}

impl fmt::Display for ProjectRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.owner, self.repo)
    }
}

impl FromStr for ProjectRef {
    type Err = ProjectRefError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let segments: Vec<&str> = s.split('/').collect();
        if segments.len() < 2 {
            return Err(ProjectRefError::Malformed(s.to_owned()));
        }
        if segments.iter().any(|seg| seg.is_empty()) {
            return Err(ProjectRefError::EmptySegment(s.to_owned()));
        }
        let (repo, namespace) = segments.split_last().expect("checked len >= 2 above");
        Self::new(namespace.join("/"), (*repo).to_owned())
    }
}

impl<'de> Deserialize<'de> for ProjectRef {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_owner_repo() {
        let p: ProjectRef = "your-org/your-project".parse().unwrap();
        assert_eq!(p.owner(), "acme");
        assert_eq!(p.repo(), "atlas");
        assert_eq!(p.path(), "your-org/your-project");
    }

    #[test]
    fn rejects_missing_slash() {
        assert!("noslash".parse::<ProjectRef>().is_err());
    }

    #[test]
    fn parses_nested_subgroup() {
        let p: ProjectRef = "acme/sandbox/widgets".parse().unwrap();
        assert_eq!(p.owner(), "acme/sandbox");
        assert_eq!(p.repo(), "atlas-test");
        assert_eq!(p.path(), "acme/sandbox/widgets");
    }

    #[test]
    fn parses_arbitrary_depth() {
        let p: ProjectRef = "a/b/c/d".parse().unwrap();
        assert_eq!(p.owner(), "a/b/c");
        assert_eq!(p.repo(), "d");
        assert_eq!(p.path(), "a/b/c/d");
    }

    #[test]
    fn rejects_empty_segments() {
        assert!("/repo".parse::<ProjectRef>().is_err());
        assert!("owner/".parse::<ProjectRef>().is_err());
        assert!("a//c".parse::<ProjectRef>().is_err());
    }
}
