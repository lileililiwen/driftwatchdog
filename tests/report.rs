//! End-to-end tests of `driftwatch report`.

#![cfg(unix)]

use assert_cmd::Command;
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

fn run_failing(tmp: &tempfile::TempDir, msg: &str) {
    driftwatch()
        .args(["run", "sh", "-c", &format!("echo {msg} >&2; exit 1")])
        .current_dir(tmp.path())
        .assert()
        .failure();
}

#[test]
fn report_renders_markdown_sections_on_empty_db() {
    let tmp = init_dir();
    let out = driftwatch()
        .arg("report")
        .current_dir(tmp.path())
        .assert()
        .success();
    let stdout = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    assert!(stdout.contains("# Driftwatch report"));
    assert!(stdout.contains("No recurring failures in the selected window."));
}

#[test]
fn report_renders_top_table_and_trend() {
    let tmp = init_dir();
    for _ in 0..3 {
        run_failing(&tmp, "report_boom");
    }
    let out = driftwatch()
        .arg("report")
        .current_dir(tmp.path())
        .assert()
        .success();
    let stdout = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    assert!(stdout.contains("# Driftwatch report"));
    assert!(stdout.contains("## Top recurring failures"));
    assert!(stdout.contains("## Bug trend (last 7 days)"));
    assert!(stdout.contains("| Bug | Today | -1d | -2d | -3d | -4d | -5d | -6d |"));
    assert!(stdout.contains("report_boom"));
    // Count column should show 3.
    assert!(stdout.contains("| 3 |") || stdout.contains(" 3 "));
}

#[test]
fn report_respects_days_filter() {
    let tmp = init_dir();
    run_failing(&tmp, "recent_boom");
    // Insert a synthetic old fingerprint via the DB.
    let db_path = tmp.path().join(".driftwatch/state.db");
    let conn = rusqlite::Connection::open(&db_path).unwrap();
    conn.execute(
        "INSERT INTO fingerprints (hash, canonical, summary, first_seen_at, last_seen_at, occurrence_count)
         VALUES ('1111111111111111111111111111111111111111111111111111111111111111', 'old boom', 'old boom', '2020-01-01T00:00:00Z', '2020-01-01T00:00:00Z', 1)",
        [],
    )
    .unwrap();
    let out = driftwatch()
        .arg("report")
        .current_dir(tmp.path())
        .assert()
        .success();
    let stdout = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    // The old fingerprint is filtered out by the default 30-day window.
    assert!(!stdout.contains("old boom"));
    assert!(stdout.contains("recent_boom"));
}
