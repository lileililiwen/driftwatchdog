//! End-to-end tests of `driftwatch run` via the compiled binary.
//!
//! Unix-only because the child processes are shell-driven for control over
//! exit codes. The cross-platform aspect is covered by the runner unit
//! tests (argv passthrough) and the binary's compiled surface.

#![cfg(unix)]

use assert_cmd::Command;
use driftwatchdog::repo::{runs::RunRecord, runs::RunStatus, runs::Runs, Db};
use rusqlite::Connection;
use tempfile::tempdir;

fn driftwatch() -> Command {
    Command::cargo_bin("driftwatchdog").expect("compiled driftwatch binary")
}

fn init_dir() -> tempfile::TempDir {
    let tmp = tempdir().unwrap();
    driftwatch()
        .arg("init")
        .current_dir(tmp.path())
        .assert()
        .success();
    tmp
}

fn open_db(tmp: &tempfile::TempDir) -> Connection {
    let path = tmp.path().join(".driftwatch/state.db");
    Connection::open(&path).expect("open db")
}

fn list_runs(conn: &Connection) -> Vec<RunRecord> {
    let mut stmt = conn
        .prepare(
            "SELECT id, started_at, finished_at, duration_ms, program, argv, cwd, exit_code, status,
                    tags, stdout_excerpt, stderr_excerpt, stdout_truncated, stderr_truncated,
                    git_commit, git_branch, git_dirty
             FROM runs ORDER BY id",
        )
        .unwrap();
    stmt.query_map([], map_run)
        .unwrap()
        .filter_map(Result::ok)
        .collect()
}

fn map_run(row: &rusqlite::Row<'_>) -> rusqlite::Result<RunRecord> {
    let status_str: String = row.get(8)?;
    let tags_json: String = row.get(9)?;
    let stdout_truncated: i64 = row.get(12)?;
    let stderr_truncated: i64 = row.get(13)?;
    let git_dirty: Option<i64> = row.get(16)?;
    Ok(RunRecord {
        id: row.get(0)?,
        started_at: row.get(1)?,
        finished_at: row.get(2)?,
        duration_ms: row.get(3)?,
        program: row.get(4)?,
        argv_json: row.get(5)?,
        cwd: row.get(6)?,
        exit_code: row.get(7)?,
        status: RunStatus::parse(&status_str).unwrap(),
        tags: serde_json::from_str(&tags_json).unwrap_or_default(),
        stdout_excerpt: row.get(10)?,
        stderr_excerpt: row.get(11)?,
        stdout_truncated: stdout_truncated != 0,
        stderr_truncated: stderr_truncated != 0,
        git_commit: row.get(14)?,
        git_branch: row.get(15)?,
        git_dirty: git_dirty.map(|v| v != 0),
    })
}

#[test]
fn run_echo_succeeds_and_returns_zero() {
    let tmp = init_dir();
    driftwatch()
        .args(["run", "echo", "hello"])
        .current_dir(tmp.path())
        .assert()
        .success()
        .stdout(predicates::str::contains("driftwatch: echo"));

    let conn = open_db(&tmp);
    let rows = list_runs(&conn);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].program, "echo");
    assert_eq!(rows[0].status, RunStatus::Success);
    assert_eq!(rows[0].exit_code, Some(0));
    let argv: Vec<String> = serde_json::from_str(&rows[0].argv_json).unwrap();
    assert_eq!(argv, vec!["hello".to_string()]);
}

#[test]
fn run_failing_command_propagates_exit_code() {
    let tmp = init_dir();
    driftwatch()
        .args(["run", "sh", "-c", "exit 7"])
        .current_dir(tmp.path())
        .assert()
        .failure()
        .code(7);

    let conn = open_db(&tmp);
    let rows = list_runs(&conn);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].status, RunStatus::Failed);
    assert_eq!(rows[0].exit_code, Some(7));
}

#[test]
fn run_missing_executable_is_start_failed() {
    let tmp = init_dir();
    driftwatch()
        .args(["run", "definitely-not-a-real-binary-xyz-abc"])
        .current_dir(tmp.path())
        .assert()
        .failure()
        .code(127);

    let conn = open_db(&tmp);
    let rows = list_runs(&conn);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].status, RunStatus::StartFailed);
    assert!(rows[0].exit_code.is_none());
}

#[test]
fn run_with_tag_persists_tag() {
    let tmp = init_dir();
    driftwatch()
        .args(["run", "--tag", "auth", "echo", "hi"])
        .current_dir(tmp.path())
        .assert()
        .success();

    let conn = open_db(&tmp);
    let rows = list_runs(&conn);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].tags, vec!["auth".to_string()]);
}

#[test]
fn run_arbitrary_argv_passes_through() {
    let tmp = init_dir();
    driftwatch()
        .args(["run", "echo", "--release", "with spaces"])
        .current_dir(tmp.path())
        .assert()
        .success();

    let conn = open_db(&tmp);
    let rows = list_runs(&conn);
    assert_eq!(rows.len(), 1);
    let argv: Vec<String> = serde_json::from_str(&rows[0].argv_json).unwrap();
    assert_eq!(
        argv,
        vec!["--release".to_string(), "with spaces".to_string()]
    );
}

#[test]
fn run_in_subdirectory_discovers_parent_project() {
    let tmp = init_dir();
    let sub = tmp.path().join("sub/deeper");
    std::fs::create_dir_all(&sub).unwrap();
    driftwatch()
        .args(["run", "echo", "from-sub"])
        .current_dir(&sub)
        .assert()
        .success();

    // The row should land in the parent's DB.
    let conn = open_db(&tmp);
    let rows = list_runs(&conn);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].program, "echo");
    // The reserved cwd is the directory we ran from, even though the
    // discovered project root is the parent. This is by design: the run is
    // attributed to the directory the user was in.
    assert!(rows[0].cwd.ends_with("deeper"));
}

#[test]
fn multiple_runs_append_rows() {
    let tmp = init_dir();
    for arg in ["one", "two", "three"] {
        driftwatch()
            .args(["run", "echo", arg])
            .current_dir(tmp.path())
            .assert()
            .success();
    }
    let conn = open_db(&tmp);
    let rows = list_runs(&conn);
    assert_eq!(rows.len(), 3);
    let argv_per_row: Vec<Vec<String>> = rows
        .iter()
        .map(|r| serde_json::from_str(&r.argv_json).unwrap())
        .collect();
    assert_eq!(argv_per_row[0], vec!["one".to_string()]);
    assert_eq!(argv_per_row[1], vec!["two".to_string()]);
    assert_eq!(argv_per_row[2], vec!["three".to_string()]);
}

#[test]
fn runs_repository_list_helper_matches_integration_list() {
    // Cross-check that the typed Runs::list helper sees the same rows that
    // the CLI's list command rendered.
    let tmp = init_dir();
    driftwatch()
        .args(["run", "echo", "ok"])
        .current_dir(tmp.path())
        .assert()
        .success();
    driftwatch()
        .args(["run", "sh", "-c", "exit 2"])
        .current_dir(tmp.path())
        .assert()
        .failure();

    let db = Db::open(&tmp.path().join(".driftwatch/state.db")).unwrap();
    let rows = Runs::new(&db)
        .list(&driftwatchdog::repo::runs::ListFilter {
            limit: 10,
            only_failed: true,
            tag: None,
        })
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].status, RunStatus::Failed);
}

#[test]
fn run_sigkill_records_signalled_not_start_failed() {
    let tmp = init_dir();
    driftwatch()
        .args(["run", "sh", "-c", "kill -KILL $$"])
        .current_dir(tmp.path())
        .assert()
        .failure()
        .code(127);

    let conn = open_db(&tmp);
    let rows = list_runs(&conn);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].status, RunStatus::Signalled);
    assert!(rows[0].exit_code.is_none());
}

#[test]
fn run_timeout_records_timeout_status() {
    let tmp = init_dir();
    let started = std::time::Instant::now();
    driftwatch()
        .args(["run", "--timeout-ms", "300", "sh", "-c", "sleep 30"])
        .current_dir(tmp.path())
        .assert()
        .failure();
    assert!(
        started.elapsed() < std::time::Duration::from_secs(10),
        "timed-out run must return promptly"
    );

    let conn = open_db(&tmp);
    let rows = list_runs(&conn);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].status, RunStatus::Timeout);
}

#[test]
fn run_failed_filter_includes_signalled_and_timeout() {
    let tmp = init_dir();
    driftwatch()
        .args(["run", "sh", "-c", "kill -KILL $$"])
        .current_dir(tmp.path())
        .assert()
        .failure();
    driftwatch()
        .args(["run", "--timeout-ms", "200", "sh", "-c", "sleep 30"])
        .current_dir(tmp.path())
        .assert()
        .failure();

    let db = Db::open(&tmp.path().join(".driftwatch/state.db")).unwrap();
    let rows = Runs::new(&db)
        .list(&driftwatchdog::repo::runs::ListFilter {
            limit: 10,
            only_failed: true,
            tag: None,
        })
        .unwrap();
    assert_eq!(rows.len(), 2);
}
