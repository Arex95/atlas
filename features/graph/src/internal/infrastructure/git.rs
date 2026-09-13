//! Reading what has changed, from git.
//!
//! Shelling out to `git` rather than linking a library. The binary is
//! already present wherever this runs — a developer machine with a
//! repository on it — and it is the one implementation guaranteed to
//! agree with what the developer sees when they type the same command.
//! A library that disagrees with `git status` about a submodule, a
//! sparse checkout or a worktree produces an impact report the reader
//! cannot reconcile with their own terminal.
//!
//! Paths are read with `-z` so that a filename containing a space,
//! a quote or a newline is handled rather than silently truncated.

use std::path::Path;
use std::process::Command;

use crate::internal::domain::GraphError;

/// What happened to one file.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeKind {
    Added,
    Modified,
    Deleted,
    Renamed,
    /// On disk, never committed. Reported because a new file nothing
    /// imports yet is exactly the kind of thing worth noticing before
    /// it becomes permanent.
    Untracked,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct ChangedFile {
    /// Relative to the repository root, matching graph node identity.
    pub path: String,
    pub kind: ChangeKind,
    /// Where a rename came from. `None` for everything else.
    pub renamed_from: Option<String>,
}

/// Checks that `root` is a working tree this process may read.
///
/// Returns git's own refusal rather than a paraphrase of it. The first
/// version of this reported every failure as "not a working tree",
/// which was wrong the moment it met a repository owned by another
/// user: git refuses that with `detected dubious ownership` and tells
/// you exactly which command fixes it, and flattening that into "not a
/// repository" sent the reader to check a path that was fine.
///
/// **The ownership refusal is deliberately not worked around.** Atlas
/// could pass `-c safe.directory`, and it would make a mounted
/// repository work immediately — but that protection exists because a
/// repository owned by someone else carries configuration that can run
/// commands (`core.fsmonitor`, hooks), and `git status` honours it.
/// Bypassing a check on the user's behalf, silently, is not Atlas's
/// call to make.
///
/// # Errors
/// `Walk` if git is missing, the path is not a repository, or git
/// refuses to read it.
fn require_work_tree(root: &Path) -> Result<(), GraphError> {
    let out = Command::new("git")
        .args(["rev-parse", "--is-inside-work-tree"])
        .current_dir(root)
        .output()
        .map_err(|e| missing_git(&e))?;

    if out.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&out.stderr);
    let stderr = stderr.trim();
    if stderr.is_empty() || stderr.contains("not a git repository") {
        return Err(GraphError::Walk(format!(
            "{} is not inside a git working tree, so there is nothing to compare",
            root.display()
        )));
    }
    Err(GraphError::Walk(stderr.to_owned()))
}

/// Files changed in the working tree, or between `against` and the
/// working tree when a ref is given.
///
/// The default is everything not yet committed — staged, unstaged and
/// untracked — because that is what a developer is about to be judged
/// on. Passing a ref (`main`, a tag, a SHA) answers the other useful
/// question: what does this branch change.
///
/// # Errors
/// `Walk` if git cannot be run or read this repository, or the ref
/// does not exist.
pub fn changed_files(root: &Path, against: Option<&str>) -> Result<Vec<ChangedFile>, GraphError> {
    require_work_tree(root)?;

    match against {
        Some(reference) => diff_against(root, reference),
        None => working_tree(root),
    }
}

/// Turns the raw OS error into something actionable.
///
/// A bare "No such file or directory (os error 2)" from a tool that
/// was handed a directory sends the reader to check that directory,
/// which is not the problem. This says which file is missing.
fn missing_git(e: &std::io::Error) -> GraphError {
    if e.kind() == std::io::ErrorKind::NotFound {
        return GraphError::Walk(
            "git is not installed or not on PATH, and this reads a working tree by running it"
                .to_owned(),
        );
    }
    GraphError::Walk(format!("could not run git: {e}"))
}

fn working_tree(root: &Path) -> Result<Vec<ChangedFile>, GraphError> {
    let out = run(
        root,
        &["status", "--porcelain", "-z", "--untracked-files=all"],
    )?;
    Ok(parse_status(&out))
}

fn diff_against(root: &Path, reference: &str) -> Result<Vec<ChangedFile>, GraphError> {
    // `--` separates the ref from paths, so a ref that happens to
    // share a name with a file cannot be reinterpreted as one.
    let out = run(
        root,
        &[
            "diff",
            "--name-status",
            "-z",
            "--find-renames",
            reference,
            "--",
        ],
    )?;
    Ok(parse_name_status(&out))
}

fn run(root: &Path, args: &[&str]) -> Result<String, GraphError> {
    let out = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .map_err(|e| missing_git(&e))?;

    if !out.status.success() {
        // git's own message, not a paraphrase: "unknown revision" tells
        // the caller what to fix, "git failed" does not.
        return Err(GraphError::Walk(
            String::from_utf8_lossy(&out.stderr).trim().to_owned(),
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Parses `git status --porcelain -z`.
///
/// Each record is `XY <path>\0`, where `X` is the index status and `Y`
/// the working-tree status. A rename carries a second NUL-terminated
/// field with the old path.
fn parse_status(out: &str) -> Vec<ChangedFile> {
    let mut fields = out.split('\0').filter(|f| !f.is_empty());
    let mut changes = Vec::new();

    while let Some(record) = fields.next() {
        // "XY path" — the status is two columns, then one space.
        if record.len() < 4 {
            continue;
        }
        let (status, path) = record.split_at(2);
        let path = path.trim_start().to_owned();
        let mut bytes = status.bytes();
        let (index, worktree) = (bytes.next(), bytes.next());

        // Untracked is "??" in both columns and has no old path.
        if status == "??" {
            changes.push(ChangedFile {
                path,
                kind: ChangeKind::Untracked,
                renamed_from: None,
            });
            continue;
        }

        // A rename consumes the following field, which is the old path.
        // Skipping that consumption would read the old path as its own
        // record and report a file that has not changed.
        let renamed_from = if index == Some(b'R') || worktree == Some(b'R') {
            fields.next().map(std::borrow::ToOwned::to_owned)
        } else {
            None
        };

        // The index column wins when both are set: a file staged as
        // added and then modified is, to a reader, a new file.
        let kind = match (index, worktree) {
            (Some(b'R'), _) | (_, Some(b'R')) => ChangeKind::Renamed,
            (Some(b'A'), _) | (_, Some(b'A')) => ChangeKind::Added,
            (Some(b'D'), _) | (_, Some(b'D')) => ChangeKind::Deleted,
            _ => ChangeKind::Modified,
        };

        changes.push(ChangedFile {
            path,
            kind,
            renamed_from,
        });
    }

    changes
}

/// Parses `git diff --name-status -z`.
///
/// Unlike `status`, the status and the path are separate NUL-terminated
/// fields, and a rename is followed by two paths rather than one.
fn parse_name_status(out: &str) -> Vec<ChangedFile> {
    let mut fields = out.split('\0').filter(|f| !f.is_empty());
    let mut changes = Vec::new();

    while let Some(status) = fields.next() {
        let letter = status.as_bytes().first().copied();
        // R and C carry a similarity score (`R100`) and two paths.
        if letter == Some(b'R') || letter == Some(b'C') {
            let (Some(from), Some(to)) = (fields.next(), fields.next()) else {
                break;
            };
            changes.push(ChangedFile {
                path: to.to_owned(),
                kind: ChangeKind::Renamed,
                renamed_from: Some(from.to_owned()),
            });
            continue;
        }

        let Some(path) = fields.next() else { break };
        changes.push(ChangedFile {
            path: path.to_owned(),
            kind: match letter {
                Some(b'A') => ChangeKind::Added,
                Some(b'D') => ChangeKind::Deleted,
                _ => ChangeKind::Modified,
            },
            renamed_from: None,
        });
    }

    changes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_modified_and_an_added_file_are_told_apart() {
        let out = " M src/a.rs\0A  src/b.rs\0";
        let changes = parse_status(out);
        assert_eq!(changes.len(), 2);
        assert_eq!(changes[0].path, "src/a.rs");
        assert_eq!(changes[0].kind, ChangeKind::Modified);
        assert_eq!(changes[1].path, "src/b.rs");
        assert_eq!(changes[1].kind, ChangeKind::Added);
    }

    #[test]
    fn an_untracked_file_is_reported_as_such() {
        let changes = parse_status("?? src/new.rs\0");
        assert_eq!(changes[0].kind, ChangeKind::Untracked);
        assert_eq!(changes[0].path, "src/new.rs");
    }

    #[test]
    fn a_rename_consumes_its_old_path_rather_than_reporting_it() {
        // The old path is a separate field. Reading it as a record
        // would report `src/old.rs` as a file that changed on its own.
        let changes = parse_status("R  src/new.rs\0src/old.rs\0 M src/other.rs\0");
        assert_eq!(changes.len(), 2, "{changes:?}");
        assert_eq!(changes[0].kind, ChangeKind::Renamed);
        assert_eq!(changes[0].path, "src/new.rs");
        assert_eq!(changes[0].renamed_from.as_deref(), Some("src/old.rs"));
        assert_eq!(changes[1].path, "src/other.rs");
    }

    #[test]
    fn a_path_containing_a_space_survives() {
        let changes = parse_status(" M src/a file.rs\0");
        assert_eq!(changes[0].path, "src/a file.rs");
    }

    #[test]
    fn a_deletion_is_recognised_from_either_column() {
        assert_eq!(parse_status("D  a.rs\0")[0].kind, ChangeKind::Deleted);
        assert_eq!(parse_status(" D a.rs\0")[0].kind, ChangeKind::Deleted);
    }

    #[test]
    fn name_status_pairs_a_status_with_the_path_that_follows_it() {
        let changes = parse_name_status("M\0src/a.rs\0A\0src/b.rs\0D\0src/c.rs\0");
        assert_eq!(changes.len(), 3);
        assert_eq!(changes[0].kind, ChangeKind::Modified);
        assert_eq!(changes[1].kind, ChangeKind::Added);
        assert_eq!(changes[2].kind, ChangeKind::Deleted);
        assert_eq!(changes[2].path, "src/c.rs");
    }

    #[test]
    fn name_status_rename_takes_two_paths_and_reports_the_new_one() {
        let changes = parse_name_status("R100\0src/old.rs\0src/new.rs\0M\0src/z.rs\0");
        assert_eq!(changes.len(), 2, "{changes:?}");
        assert_eq!(changes[0].path, "src/new.rs");
        assert_eq!(changes[0].renamed_from.as_deref(), Some("src/old.rs"));
        assert_eq!(changes[1].path, "src/z.rs");
    }

    #[test]
    fn a_truncated_record_is_dropped_rather_than_panicking() {
        // Whatever produced this, guessing at half a record is worse
        // than ignoring it.
        assert!(parse_status("M\0").is_empty());
        assert!(parse_name_status("R100\0only-one-path\0").is_empty());
    }
}
