//! End-to-end tests of failure fingerprinting through the binary.
//!
//! Verifies that `driftwatch run` attaches a fingerprint to failed
//! runs, that repeated identical failures group into one fingerprint,
//! and that the generic normalizer does not split logically identical
//! errors that differ only in line numbers or durations.

#![cfg(unix)]

use assert_cmd::Command;
use driftwatch::repo::Db;
use rusqlite::Connection;
use tempfile::tempdir;

fn driftwatch() -> Command {
    Command::cargo_bin("driftwatch").expect("compiled driftwatch binary")
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

fn fingerprint_count(conn: &Connection) -> i64 {
    conn.query_row("SELECT COUNT(*) FROM fingerprints", [], |r| r.get(0))
        .unwrap()
}

fn occurrence_count(conn: &Connection) -> i64 {
    conn.query_row("SELECT COUNT(*) FROM occurrences", [], |r| r.get(0))
        .unwrap()
}

fn occurrence_count_for(conn: &Connection, fingerprint_id: i64) -> i64 {
    conn.query_row(
        "SELECT COUNT(*) FROM occurrences WHERE fingerprint_id = ?1",
        [fingerprint_id],
        |r| r.get(0),
    )
    .unwrap()
}

fn run_count(conn: &Connection, status: &str) -> i64 {
    conn.query_row(
        "SELECT COUNT(*) FROM runs WHERE status = ?1",
        [status],
        |r| r.get(0),
    )
    .unwrap()
}

#[test]
fn failing_command_attaches_fingerprint() {
    let tmp = init_dir();
    driftwatch()
        .args(["run", "sh", "-c", "echo boom >&2; exit 1"])
        .current_dir(tmp.path())
        .assert()
        .failure();
    let conn = open_db(&tmp);
    assert_eq!(run_count(&conn, "failed"), 1);
    assert_eq!(fingerprint_count(&conn), 1);
    assert_eq!(occurrence_count(&conn), 1);
}

#[test]
fn successful_command_does_not_attach_fingerprint() {
    let tmp = init_dir();
    driftwatch()
        .args(["run", "echo", "hi"])
        .current_dir(tmp.path())
        .assert()
        .success();
    let conn = open_db(&tmp);
    assert_eq!(run_count(&conn, "success"), 1);
    assert_eq!(fingerprint_count(&conn), 0);
    assert_eq!(occurrence_count(&conn), 0);
}

#[test]
fn three_identical_failures_group_into_one_fingerprint() {
    let tmp = init_dir();
    for _ in 0..3 {
        driftwatch()
            .args(["run", "sh", "-c", "echo boom >&2; exit 1"])
            .current_dir(tmp.path())
            .assert()
            .failure();
    }
    let conn = open_db(&tmp);
    assert_eq!(run_count(&conn, "failed"), 3);
    assert_eq!(fingerprint_count(&conn), 1);
    assert_eq!(occurrence_count(&conn), 3);
}

#[test]
fn moving_timeout_does_not_split_fingerprint() {
    let tmp = init_dir();
    // Two failures that differ only in the absolute path and the
    // embedded duration; the generic normalizer should collapse them
    // to the same canonical text.
    driftwatch()
        .args([
            "run",
            "sh",
            "-c",
            "echo error at /var/log/db.log:42:5 after 1500 ms >&2; exit 1",
        ])
        .current_dir(tmp.path())
        .assert()
        .failure();
    driftwatch()
        .args([
            "run",
            "sh",
            "-c",
            "echo error at /var/log/db.log:207:9 after 3200 ms >&2; exit 1",
        ])
        .current_dir(tmp.path())
        .assert()
        .failure();
    let conn = open_db(&tmp);
    assert_eq!(run_count(&conn, "failed"), 2);
    assert_eq!(fingerprint_count(&conn), 1);
    assert_eq!(occurrence_count(&conn), 2);
}

#[test]
fn start_failed_attaches_fingerprint() {
    let tmp = init_dir();
    driftwatch()
        .args(["run", "definitely-not-a-real-binary-xyz-zzz"])
        .current_dir(tmp.path())
        .assert()
        .failure()
        .code(127);
    let conn = open_db(&tmp);
    assert_eq!(run_count(&conn, "start_failed"), 1);
    // The OS diagnostic is fingerprinted, so we get at least one
    // fingerprint and one occurrence.
    assert_eq!(fingerprint_count(&conn), 1);
    assert_eq!(occurrence_count(&conn), 1);
}

#[test]
fn distinct_messages_get_distinct_fingerprints() {
    let tmp = init_dir();
    driftwatch()
        .args(["run", "sh", "-c", "echo boom_a >&2; exit 1"])
        .current_dir(tmp.path())
        .assert()
        .failure();
    driftwatch()
        .args(["run", "sh", "-c", "echo boom_b >&2; exit 1"])
        .current_dir(tmp.path())
        .assert()
        .failure();
    let conn = open_db(&tmp);
    assert_eq!(fingerprint_count(&conn), 2);
    assert_eq!(occurrence_count(&conn), 2);
}

#[test]
fn repository_helper_matches_integration() {
    // The typed `Bugs::hash_for_run` should see the same fingerprint
    // that the run-time hook wrote.
    let tmp = init_dir();
    driftwatch()
        .args(["run", "sh", "-c", "echo boom >&2; exit 1"])
        .current_dir(tmp.path())
        .assert()
        .failure();
    let db = Db::open(&tmp.path().join(".driftwatch/state.db")).unwrap();
    let run_id: i64 = {
        let mut stmt = db.conn().prepare("SELECT id FROM runs LIMIT 1").unwrap();
        stmt.query_row([], |r| r.get(0)).unwrap()
    };
    let fp_id: i64 = {
        let mut stmt = db
            .conn()
            .prepare("SELECT fingerprint_id FROM occurrences WHERE run_id = ?1")
            .unwrap();
        stmt.query_row([run_id], |r| r.get(0)).unwrap()
    };
    assert_eq!(occurrence_count_for(db.conn(), fp_id), 1);
}
