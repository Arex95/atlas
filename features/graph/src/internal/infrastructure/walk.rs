//! Gitignore-aware discovery of what is worth indexing.
//!
//! Exclusion is layered, and the order matters:
//!
//! 1. `.gitignore` at every level, `.git/info/exclude`, and `.ignore`
//!    — handled by the `ignore` crate, the same walker ripgrep uses.
//! 2. A hardcoded blacklist, applied **regardless** of what any
//!    ignore file says. A repository that forgets to ignore
//!    `node_modules` should not produce a graph that is mostly
//!    `node_modules`.
//! 3. A size cap: a file past it is skipped rather than read.
//! 4. Binary detection by null byte, so a `.bin` that slipped through
//!    is not indexed as text.
//!
//! Symlinks are never followed — that avoids both loops and a link
//! that escapes the project root entirely.

use std::path::{Path, PathBuf};

use ignore::WalkBuilder;

use crate::internal::domain::GraphError;

/// Always skipped, whatever `.gitignore` says.
const BLACKLIST_DIRS: &[&str] = &[
    ".git",
    "target",
    "node_modules",
    "dist",
    "build",
    ".venv",
    "venv",
    "__pycache__",
    ".next",
    ".nuxt",
    ".cache",
    "vendor",
    "coverage",
];

/// Lockfiles and minified bundles: large, generated, and nothing an
/// agent asks a question about.
const BLACKLIST_FILES: &[&str] = &[
    "Cargo.lock",
    "package-lock.json",
    "pnpm-lock.yaml",
    "yarn.lock",
    "poetry.lock",
    "composer.lock",
    "Gemfile.lock",
];

/// Past this, a file is generated, vendored or data — not something
/// worth a node.
const MAX_FILE_BYTES: u64 = 1_000_000;

/// How much of a file to sniff for a null byte before deciding it is
/// binary.
const BINARY_SNIFF_BYTES: usize = 8_000;

pub struct DiscoveredFile {
    pub absolute_path: PathBuf,
    /// Relative to the project root, always `/`-separated so the same
    /// file has the same identity on every platform.
    pub relative_path: String,
    pub extension: String,
}

/// Walks `root`, returning the files worth indexing.
///
/// # Errors
/// `RootNotADirectory` if `root` is not one, `Walk` on an unreadable
/// tree.
pub fn discover(root: &Path) -> Result<Vec<DiscoveredFile>, GraphError> {
    if !root.is_dir() {
        return Err(GraphError::RootNotADirectory(root.display().to_string()));
    }

    let mut found = Vec::new();

    let walker = WalkBuilder::new(root)
        .hidden(false) // dotfiles are content; .gitignore still applies
        .follow_links(false)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        // Honour .gitignore even when there is no .git directory. The
        // crate's default is to require one, which would silently
        // index everything in a project that is not a repository — or
        // in a subdirectory indexed on its own. A .gitignore is a
        // statement of intent regardless of what is around it.
        .require_git(false)
        .filter_entry(|entry| {
            entry
                .file_name()
                .to_str()
                .is_none_or(|name| !BLACKLIST_DIRS.contains(&name))
        })
        .build();

    for entry in walker {
        let entry = entry.map_err(|e| GraphError::Walk(e.to_string()))?;
        if !entry.file_type().is_some_and(|t| t.is_file()) {
            continue;
        }
        let path = entry.path();

        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if BLACKLIST_FILES.contains(&name) {
            continue;
        }

        match path.metadata() {
            Ok(meta) if meta.len() > MAX_FILE_BYTES => continue,
            Ok(_) => {}
            Err(_) => continue,
        }

        if is_binary(path) {
            continue;
        }

        let Ok(relative) = path.strip_prefix(root) else {
            continue;
        };
        let relative_path = relative
            .components()
            .filter_map(|c| c.as_os_str().to_str())
            .collect::<Vec<_>>()
            .join("/");
        if relative_path.is_empty() {
            continue;
        }

        found.push(DiscoveredFile {
            absolute_path: path.to_path_buf(),
            relative_path,
            extension: path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or_default()
                .to_owned(),
        });
    }

    found.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
    Ok(found)
}

/// Whether a path lies under something the walker never indexes.
///
/// The watcher needs the same answer the walker gives, and needs it
/// per-event rather than per-tree: a `cargo build` writes thousands of
/// files under `target/`, and a watcher that does not filter them
/// reindexes in a loop for as long as the build runs.
///
/// Only the directory blacklist, not the full ignore-file machinery —
/// this runs on every filesystem event, and the blacklist is what
/// covers the churn (`target`, `node_modules`, `.git`). A file that
/// only `.gitignore` excludes triggers a reindex that then correctly
/// omits it: wasteful, not wrong.
#[must_use]
pub fn is_ignored_path(path: &Path) -> bool {
    path.components()
        .filter_map(|c| c.as_os_str().to_str())
        .any(|segment| BLACKLIST_DIRS.contains(&segment))
}

/// A null byte in the first few kilobytes. Crude, and the same test
/// git itself uses — a text file that trips it is rare enough to be
/// worth the simplicity.
fn is_binary(path: &Path) -> bool {
    use std::io::Read as _;

    let Ok(mut file) = std::fs::File::open(path) else {
        return true;
    };
    let mut buffer = vec![0u8; BINARY_SNIFF_BYTES];
    let Ok(read) = file.read(&mut buffer) else {
        return true;
    };
    buffer[..read].contains(&0)
}
