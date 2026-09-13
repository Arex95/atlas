//! Integration tests spawning real PTY processes — not mocks. Uses
//! a plain shell and predictable, non-interactive commands (`echo`)
//! to stay deterministic in CI: no login-shell prompts, no ANSI
//! noise to parse around.

use std::path::Path;
use std::time::Duration;

/// Every fixture session is created under this owner, and every
/// pool call is now made as its owner — a terminal belongs to one.
const OWNER: &str = "owner-1";

/// A different developer. Everything they try must look like it is
/// not there.
const INTRUDER: &str = "someone-else";

use atlas_sessions::api::{NewSession, SessionStore, SqlitePool, run_migrations};
use atlas_terminal::api::{PtyPool, RestoreOutcome, TerminalError};
use tempfile::TempDir;

async fn fresh_pool() -> (PtyPool, TempDir, atlas_sessions::api::SessionId) {
    let (pool, workspace, id, _sessions) = fresh_pool_with_store().await;
    (pool, workspace, id)
}

async fn fresh_pool_with_store() -> (
    PtyPool,
    TempDir,
    atlas_sessions::api::SessionId,
    SessionStore,
) {
    let workspace = TempDir::new().unwrap();
    std::fs::create_dir_all(workspace.path().join("your-org/your-project")).unwrap();

    let db_pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    run_migrations(&db_pool).await.unwrap();
    let sessions = SessionStore::new(db_pool);

    let session = sessions
        .create(NewSession {
            project: "your-org/your-project".to_owned(),
            owner_id: "owner-1".to_owned(),
            remote_url: "git@github.com:you/your-project.git".to_owned(),
            branch: "main".to_owned(),
            relative_path: "your-org/your-project".to_owned(),
            agent_kind: None,
            title: None,
            resume_command: None,
        })
        .await
        .unwrap();

    let pool = PtyPool::new(
        sessions.clone(),
        workspace.path().to_path_buf(),
        "http://127.0.0.1:4000".to_owned(),
    );
    (pool, workspace, session.id, sessions)
}

/// Poll `read_output` until `predicate` matches or the deadline
/// passes — the reader thread fills the buffer asynchronously, so a
/// single immediate read can race it.
async fn read_until(
    pool: &PtyPool,
    session_id: &atlas_sessions::api::SessionId,
    predicate: impl Fn(&[u8]) -> bool,
) -> Vec<u8> {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    let mut collected = Vec::new();
    let mut offset = 0;
    while tokio::time::Instant::now() < deadline {
        let output = pool.read_output(session_id, OWNER, offset).unwrap();
        collected.extend_from_slice(&output.data);
        offset = output.next_offset;
        if predicate(&collected) {
            return collected;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    collected
}

/// Poll a file until it has at least `want` lines, or the deadline
/// passes.
///
/// The same shape as [`read_until`], and for the same reason: a PTY
/// spawns a shell and the shell runs a command, both asynchronously,
/// so any fixed sleep is a guess about how loaded the machine is. A
/// guess that is usually right is a test that fails for reasons that
/// have nothing to do with the code.
async fn lines_until(path: &str, want: usize) -> usize {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        let n = std::fs::read_to_string(path).map_or(0, |t| t.lines().count());
        if n >= want || tokio::time::Instant::now() >= deadline {
            return n;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

#[tokio::test]
async fn spawn_write_read_close_round_trip() {
    let (pool, _workspace, session_id) = fresh_pool().await;

    let spawned = pool.spawn(&session_id, OWNER).await.unwrap();
    assert_eq!(spawned.session_id, session_id);
    assert!(spawned.pid > 0);

    pool.write(&session_id, OWNER, b"echo atlas-terminal-marker\n")
        .unwrap();

    let output = read_until(&pool, &session_id, |buf| {
        String::from_utf8_lossy(buf).contains("atlas-terminal-marker")
    })
    .await;
    assert!(
        String::from_utf8_lossy(&output).contains("atlas-terminal-marker"),
        "expected the echoed marker in PTY output, got: {:?}",
        String::from_utf8_lossy(&output)
    );

    pool.close(&session_id, OWNER).unwrap();

    let err = pool.write(&session_id, OWNER, b"echo late\n").unwrap_err();
    assert!(matches!(err, TerminalError::NotRunning));

    let err = pool.read_output(&session_id, OWNER, 0).unwrap_err();
    assert!(matches!(err, TerminalError::NotRunning));
}

#[tokio::test]
async fn spawn_is_idempotent() {
    let (pool, _workspace, session_id) = fresh_pool().await;

    let first = pool.spawn(&session_id, OWNER).await.unwrap();
    let second = pool.spawn(&session_id, OWNER).await.unwrap();
    assert_eq!(first.pid, second.pid);

    pool.close(&session_id, OWNER).unwrap();
}

#[tokio::test]
async fn spawn_unknown_session_is_session_not_found() {
    let (pool, _workspace, _session_id) = fresh_pool().await;
    let bogus = atlas_sessions::api::SessionId("nonexistent".to_owned());

    let err = pool.spawn(&bogus, OWNER).await.unwrap_err();
    assert!(matches!(err, TerminalError::SessionNotFound));
}

#[tokio::test]
async fn read_output_since_offset_beyond_buffer_is_empty_not_error() {
    let (pool, _workspace, session_id) = fresh_pool().await;
    pool.spawn(&session_id, OWNER).await.unwrap();

    let output = pool.read_output(&session_id, OWNER, 1_000_000).unwrap();
    assert_eq!(output.data, Vec::<u8>::new());

    pool.close(&session_id, OWNER).unwrap();
}

#[tokio::test]
async fn write_to_not_running_session_is_not_running() {
    let (pool, _workspace, session_id) = fresh_pool().await;
    let err = pool.write(&session_id, OWNER, b"echo x\n").unwrap_err();
    assert!(matches!(err, TerminalError::NotRunning));
}

#[tokio::test]
async fn spawn_missing_directory_is_path_not_found() {
    let workspace = TempDir::new().unwrap();
    // Deliberately do not create the directory this session's
    // relative_path points to.
    let db_pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    run_migrations(&db_pool).await.unwrap();
    let sessions = SessionStore::new(db_pool);

    let session = sessions
        .create(NewSession {
            project: "your-org/your-project".to_owned(),
            owner_id: "owner-1".to_owned(),
            remote_url: "git@github.com:you/your-project.git".to_owned(),
            branch: "main".to_owned(),
            relative_path: "does/not/exist".to_owned(),
            agent_kind: None,
            title: None,
            resume_command: None,
        })
        .await
        .unwrap();

    let pool = PtyPool::new(
        sessions,
        workspace.path().to_path_buf(),
        "http://127.0.0.1:4000".to_owned(),
    );
    let err = pool.spawn(&session.id, OWNER).await.unwrap_err();
    assert!(matches!(err, TerminalError::PathNotFound(_)));
}

/// A real local git repository with one commit on `branch`, servable
/// via a `file://` remote URL — no network, no mocked git.
fn create_local_git_remote(dir: &Path, branch: &str) {
    let run = |args: &[&str]| {
        let status = std::process::Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(args)
            .status()
            .unwrap();
        assert!(status.success(), "git {args:?} failed");
    };

    std::fs::create_dir_all(dir).unwrap();
    run(&["init", "--quiet"]);
    run(&["symbolic-ref", "HEAD", &format!("refs/heads/{branch}")]);
    std::fs::write(dir.join("README.md"), "restored\n").unwrap();
    run(&["add", "-A"]);
    run(&[
        "-c",
        "user.email=test@example.com",
        "-c",
        "user.name=test",
        "commit",
        "--quiet",
        "-m",
        "init",
    ]);
}

async fn pool_with_remote_session(
    remote_dir: &Path,
    branch: &str,
    relative_path: &str,
) -> (PtyPool, TempDir, atlas_sessions::api::SessionId) {
    let workspace = TempDir::new().unwrap();
    let db_pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    run_migrations(&db_pool).await.unwrap();
    let sessions = SessionStore::new(db_pool);

    let session = sessions
        .create(NewSession {
            project: "your-org/your-project".to_owned(),
            owner_id: "owner-1".to_owned(),
            remote_url: format!("file://{}", remote_dir.display()),
            branch: branch.to_owned(),
            relative_path: relative_path.to_owned(),
            agent_kind: None,
            title: None,
            resume_command: None,
        })
        .await
        .unwrap();

    let pool = PtyPool::new(
        sessions,
        workspace.path().to_path_buf(),
        "http://127.0.0.1:4000".to_owned(),
    );
    (pool, workspace, session.id)
}

#[tokio::test]
async fn restore_is_a_noop_when_the_path_already_exists() {
    let (pool, _workspace, session_id) = fresh_pool().await;
    let outcome = pool.restore(&session_id, OWNER).await.unwrap();
    assert_eq!(outcome, RestoreOutcome::AlreadyPresent);
}

#[tokio::test]
async fn restore_clones_a_missing_path_and_then_spawn_succeeds() {
    let remote = TempDir::new().unwrap();
    create_local_git_remote(remote.path(), "main");

    let (pool, workspace, session_id) =
        pool_with_remote_session(remote.path(), "main", "restored-workspace").await;
    let resolved = workspace.path().join("restored-workspace");
    assert!(!resolved.exists());

    let outcome = pool.restore(&session_id, OWNER).await.unwrap();
    assert_eq!(outcome, RestoreOutcome::Cloned);
    assert!(resolved.join("README.md").is_file());

    // Composition: spawn now succeeds against the just-restored path.
    let spawned = pool.spawn(&session_id, OWNER).await.unwrap();
    assert_eq!(spawned.resolved_path, resolved);
    pool.close(&session_id, OWNER).unwrap();
}

#[tokio::test]
async fn restore_unknown_session_is_session_not_found() {
    let (pool, _workspace, _session_id) = fresh_pool().await;
    let bogus = atlas_sessions::api::SessionId("nonexistent".to_owned());

    let err = pool.restore(&bogus, OWNER).await.unwrap_err();
    assert!(matches!(err, TerminalError::SessionNotFound));
}

#[tokio::test]
async fn restore_from_an_unreachable_remote_is_clone_failed() {
    let unreachable = TempDir::new().unwrap().path().join("does-not-exist.git");
    let (pool, _workspace, session_id) =
        pool_with_remote_session(&unreachable, "main", "somewhere").await;

    let err = pool.restore(&session_id, OWNER).await.unwrap_err();
    assert!(matches!(err, TerminalError::CloneFailed(_)));
}

/// The worst of the caller-asserted-input defects.
///
/// Every one of these took a session id and acted on its PTY without
/// checking who was asking. Verified against a real container before
/// the fix: one session wrote `whoami > /tmp/owned; echo PWNED` into
/// another's shell and it executed, then read the output back.
#[tokio::test]
async fn a_terminal_is_unreachable_to_anyone_but_its_owner() {
    let (pool, _workspace, session_id) = fresh_pool().await;

    pool.spawn(&session_id, OWNER).await.unwrap();

    // Spawn: somebody else's session must look like one that does not
    // exist, or ids can be probed for.
    let err = pool.spawn(&session_id, INTRUDER).await.unwrap_err();
    assert!(
        matches!(err, TerminalError::SessionNotFound),
        "spawn: {err:?}"
    );

    // Write: this one is arbitrary command execution in another
    // developer's shell.
    let err = pool.write(&session_id, INTRUDER, b"id\n").unwrap_err();
    assert!(matches!(err, TerminalError::NotRunning), "write: {err:?}");

    let err = pool.read_output(&session_id, INTRUDER, 0).unwrap_err();
    assert!(matches!(err, TerminalError::NotRunning), "read: {err:?}");

    let err = pool.close(&session_id, INTRUDER).unwrap_err();
    assert!(matches!(err, TerminalError::NotRunning), "close: {err:?}");

    // The refused close must not have taken the terminal down anyway.
    pool.write(&session_id, OWNER, b"echo alive\n").unwrap();
    pool.close(&session_id, OWNER).unwrap();
}

#[tokio::test]
async fn an_owner_may_drive_their_own_other_sessions() {
    // Scoped by owner, not by session: Atlas exists to let one
    // developer's agents drive one another, so an orchestrator holding
    // one session's credential must still reach that developer's other
    // sessions.
    let (pool, _workspace, first, sessions) = fresh_pool_with_store().await;
    let second = sessions
        .create(NewSession {
            project: "your-org/your-project".to_owned(),
            owner_id: OWNER.to_owned(),
            remote_url: "git@github.com:you/your-project.git".to_owned(),
            branch: "main".to_owned(),
            relative_path: "your-org/your-project".to_owned(),
            agent_kind: None,
            title: None,
            resume_command: None,
        })
        .await
        .unwrap();

    pool.spawn(&first, OWNER).await.unwrap();
    pool.spawn(&second.id, OWNER).await.unwrap();
    pool.write(&second.id, OWNER, b"echo hello\n").unwrap();
}

/// A session whose terminal should come back running something.
async fn pool_with_resume(command: &str) -> (PtyPool, TempDir, atlas_sessions::api::SessionId) {
    let workspace = TempDir::new().unwrap();
    std::fs::create_dir_all(workspace.path().join("your-org/your-project")).unwrap();

    let db_pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    run_migrations(&db_pool).await.unwrap();
    let sessions = SessionStore::new(db_pool);

    let session = sessions
        .create(NewSession {
            project: "your-org/your-project".to_owned(),
            owner_id: OWNER.to_owned(),
            remote_url: "git@github.com:you/your-project.git".to_owned(),
            branch: "main".to_owned(),
            relative_path: "your-org/your-project".to_owned(),
            agent_kind: None,
            title: None,
            resume_command: Some(command.to_owned()),
        })
        .await
        .unwrap();

    let pool = PtyPool::new(
        sessions,
        workspace.path().to_path_buf(),
        "http://127.0.0.1:4000".to_owned(),
    );
    (pool, workspace, session.id)
}

#[tokio::test]
async fn a_resume_command_runs_when_the_terminal_spawns() {
    // A real shell, really running it — the whole point is that the
    // agent CLI gets to rehydrate itself, and a mocked write would
    // prove only that a string was passed along.
    let (pool, _workspace, session_id) = pool_with_resume("echo rehydrated").await;
    pool.spawn(&session_id, OWNER).await.unwrap();

    let output = read_until(&pool, &session_id, |s| {
        String::from_utf8_lossy(s).contains("rehydrated")
    })
    .await;
    let text = String::from_utf8_lossy(&output);
    assert!(text.contains("rehydrated"), "{text}");
}

#[tokio::test]
async fn a_repeated_spawn_does_not_run_it_again() {
    // `terminal.spawn` is idempotent and callers poll it. Re-running
    // the command every time would restart the agent under them.
    //
    // Counted by appending to a file rather than by matching terminal
    // text: a PTY echoes the input line, redraws a prompt and emits
    // ANSI around both, so counting occurrences in the buffer measures
    // the terminal as much as the command.
    let marker = tempfile::NamedTempFile::new().unwrap();
    let path = marker.path().to_str().unwrap().to_owned();
    let (pool, _workspace, session_id) = pool_with_resume(&format!("echo ran >> {path}")).await;

    pool.spawn(&session_id, OWNER).await.unwrap();
    pool.spawn(&session_id, OWNER).await.unwrap();
    pool.spawn(&session_id, OWNER).await.unwrap();

    // Two waits, and the split is the point. First wait for the
    // command to have run *at all*, because a fixed sleep here fails
    // on a loaded machine and says "it ran 0 times" when it simply had
    // not run yet.
    assert_eq!(
        lines_until(&path, 1).await,
        1,
        "the resume command did not run within five seconds"
    );

    // Then give the other two spawns room to have run, had they.
    tokio::time::sleep(Duration::from_millis(500)).await;
    let runs = std::fs::read_to_string(&path).unwrap();
    assert_eq!(
        runs.lines().count(),
        1,
        "the resume command ran {} times across three spawns",
        runs.lines().count()
    );
}

#[tokio::test]
async fn a_session_without_one_comes_up_as_a_bare_shell() {
    let (pool, _workspace, session_id) = fresh_pool().await;
    pool.spawn(&session_id, OWNER).await.unwrap();

    // Wait for the shell to have said something rather than sleeping a
    // fixed amount: asserting an *absence* after a guess passes for
    // free whenever the guess was short, which is the same as not
    // running the test.
    let output = read_until(&pool, &session_id, |b| !b.is_empty()).await;
    let text = String::from_utf8_lossy(&output);
    assert!(
        !text.contains("command not found"),
        "something was run in a session that asked for nothing: {text}"
    );
}

/// The loop closing: an agent working in a terminal can call Atlas
/// back, and its calls are attributable to this session rather than to
/// a shared secret.
///
/// Asked of the shell and captured in a file rather than read out of
/// the terminal buffer — a PTY echoes the command before running it,
/// so matching on the buffer matches the question as readily as the
/// answer.
#[tokio::test]
async fn a_terminal_is_given_its_own_session_credential() {
    let (pool, _workspace, session_id) = fresh_pool().await;
    let out = tempfile::NamedTempFile::new().unwrap();
    let path = out.path().to_str().unwrap().to_owned();

    pool.spawn(&session_id, OWNER).await.unwrap();
    pool.write(
        &session_id,
        OWNER,
        format!("echo \"$ATLAS_SESSION_ID|$ATLAS_URL|${{#ATLAS_SESSION_TOKEN}}\" > {path}\n")
            .as_bytes(),
    )
    .unwrap();

    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    let mut written = String::new();
    while tokio::time::Instant::now() < deadline {
        written = std::fs::read_to_string(&path).unwrap_or_default();
        if written.contains('|') {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    let fields: Vec<&str> = written.trim().split('|').collect();
    assert_eq!(
        fields.len(),
        3,
        "the shell printed nothing usable: {written:?}"
    );
    assert_eq!(fields[0], session_id.0);
    assert_eq!(fields[1], "http://127.0.0.1:4000");
    // 32 random bytes, base64url without padding.
    assert_eq!(fields[2], "43", "the terminal got no usable token");
}

#[tokio::test]
async fn each_terminal_gets_a_token_that_resolves_to_its_session() {
    let (pool, _workspace, session_id, sessions) = fresh_pool_with_store().await;
    let out = tempfile::NamedTempFile::new().unwrap();
    let path = out.path().to_str().unwrap().to_owned();

    pool.spawn(&session_id, OWNER).await.unwrap();
    pool.write(
        &session_id,
        OWNER,
        format!("printf %s \"$ATLAS_SESSION_TOKEN\" > {path}\n").as_bytes(),
    )
    .unwrap();

    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    let mut token = String::new();
    while tokio::time::Instant::now() < deadline {
        token = std::fs::read_to_string(&path).unwrap_or_default();
        if !token.trim().is_empty() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    // The token the terminal holds is a real credential for this
    // session — not merely a string of the right shape.
    let caller = sessions
        .resolve_token(token.trim())
        .await
        .unwrap()
        .expect("the terminal's token resolved to nothing");
    assert_eq!(caller.session_id(), Some(session_id.0.as_str()));
    assert_eq!(caller.owner_id(), OWNER);
}
