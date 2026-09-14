//! End-to-end tests of `driftwatch check`.
//!
//! All tests are Unix-only because the checkers are launched via
//! `sh -c`. The unit tests in `src/checker/` cover the cross-platform
//! pieces of the runner and protocol parser.

#![cfg(unix)]

use assert_cmd::Command;
use driftwatchdog::repo::{alerts::Alerts, Db};
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
    Connection::open(tmp.path().join(".driftwatch/state.db")).expect("open db")
}

fn write_config(tmp: &tempfile::TempDir, body: &str) {
    std::fs::write(tmp.path().join("driftwatch.toml"), body).unwrap();
}

fn snapshot_count(conn: &Connection) -> i64 {
    conn.query_row("SELECT COUNT(*) FROM check_snapshots", [], |r| r.get(0))
        .unwrap()
}

fn alert_count(conn: &Connection) -> i64 {
    conn.query_row("SELECT COUNT(*) FROM drift_alerts", [], |r| r.get(0))
        .unwrap()
}

#[test]
fn check_with_no_checkers_exits_zero_and_prints_message() {
    let tmp = init_dir();
    let out = driftwatch()
        .arg("check")
        .current_dir(tmp.path())
        .assert()
        .success();
    let stdout = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    assert!(
        stdout.contains("no checkers configured"),
        "expected helpful message; got: {stdout}"
    );
    let conn = open_db(&tmp);
    assert_eq!(snapshot_count(&conn), 0);
}

#[test]
fn check_with_one_successful_checker_persists_snapshot_and_alerts() {
    let tmp = init_dir();
    write_config(
        &tmp,
        r#"
[[checkers]]
name = "spec"
command = "sh"
args = ["-c", "echo '{\"alerts\":[{\"severity\":\"warning\",\"message\":\"m\",\"source\":\"s\",\"symbol\":\"S\"}]}'"]
"#,
    );
    let out = driftwatch()
        .arg("check")
        .current_dir(tmp.path())
        .assert()
        .success();
    let stdout = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    assert!(stdout.contains("spec"), "missing checker name: {stdout}");
    assert!(stdout.contains("success"), "missing status: {stdout}");
    assert!(
        stdout.contains("Total alerts: 1"),
        "missing total: {stdout}"
    );

    let conn = open_db(&tmp);
    assert_eq!(snapshot_count(&conn), 1);
    assert_eq!(alert_count(&conn), 1);
}

#[test]
fn check_with_empty_alerts_persists_empty_snapshot() {
    let tmp = init_dir();
    write_config(
        &tmp,
        r#"
[[checkers]]
name = "spec"
command = "sh"
args = ["-c", "echo '{\"alerts\":[]}'"]
"#,
    );
    driftwatch()
        .arg("check")
        .current_dir(tmp.path())
        .assert()
        .success();
    let conn = open_db(&tmp);
    assert_eq!(snapshot_count(&conn), 1);
    assert_eq!(alert_count(&conn), 0);
    let status: String = conn
        .query_row("SELECT status FROM check_snapshots LIMIT 1", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(status, "empty");
}

#[test]
fn check_isolates_failures_between_two_checkers() {
    let tmp = init_dir();
    write_config(
        &tmp,
        r#"
[[checkers]]
name = "broken"
command = "sh"
args = ["-c", "exit 1"]

[[checkers]]
name = "ok"
command = "sh"
args = ["-c", "echo '{\"alerts\":[{\"severity\":\"warning\",\"message\":\"m\",\"source\":\"s\",\"symbol\":\"S\"}]}'"]
"#,
    );
    // The first checker fails, the second succeeds. The aggregate exit
    // code is 1 (partial failure) and both snapshots are persisted.
    let out = driftwatch()
        .arg("check")
        .current_dir(tmp.path())
        .assert()
        .failure()
        .code(1);
    let stdout = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    assert!(stdout.contains("broken"), "missing broken row: {stdout}");
    assert!(stdout.contains("ok"), "missing ok row: {stdout}");

    let conn = open_db(&tmp);
    assert_eq!(snapshot_count(&conn), 2);
    // One snapshot failed, one succeeded; one alert in the successful one.
    assert_eq!(alert_count(&conn), 1);
}

#[test]
fn check_all_checkers_failing_exits_two() {
    let tmp = init_dir();
    write_config(
        &tmp,
        r#"
[[checkers]]
name = "a"
command = "sh"
args = ["-c", "exit 1"]

[[checkers]]
name = "b"
command = "sh"
args = ["-c", "exit 2"]
"#,
    );
    driftwatch()
        .arg("check")
        .current_dir(tmp.path())
        .assert()
        .failure()
        .code(2);
    let conn = open_db(&tmp);
    assert_eq!(snapshot_count(&conn), 2);
    assert_eq!(alert_count(&conn), 0);
}

#[test]
fn check_only_filter_runs_named_checker() {
    let tmp = init_dir();
    write_config(
        &tmp,
        r#"
[[checkers]]
name = "first"
command = "sh"
args = ["-c", "exit 1"]

[[checkers]]
name = "second"
command = "sh"
args = ["-c", "echo '{\"alerts\":[]}'"]
"#,
    );
    driftwatch()
        .args(["check", "--only", "second"])
        .current_dir(tmp.path())
        .assert()
        .success();
    let conn = open_db(&tmp);
    assert_eq!(snapshot_count(&conn), 1);
    let name: String = conn
        .query_row(
            "SELECT checker_name FROM check_snapshots LIMIT 1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(name, "second");
}

#[test]
fn check_only_unknown_name_is_an_error() {
    let tmp = init_dir();
    write_config(
        &tmp,
        r#"
[[checkers]]
name = "first"
command = "sh"
args = ["-c", "echo '{\"alerts\":[]}'"]
"#,
    );
    driftwatch()
        .args(["check", "--only", "does_not_exist"])
        .current_dir(tmp.path())
        .assert()
        .failure();
    let conn = open_db(&tmp);
    assert_eq!(snapshot_count(&conn), 0);
}

#[test]
fn check_dry_run_does_not_persist() {
    let tmp = init_dir();
    write_config(
        &tmp,
        r#"
[[checkers]]
name = "spec"
command = "sh"
args = ["-c", "echo '{\"alerts\":[{\"severity\":\"warning\",\"message\":\"m\",\"source\":\"s\",\"symbol\":\"S\"}]}'"]
"#,
    );
    let out = driftwatch()
        .args(["check", "--dry-run"])
        .current_dir(tmp.path())
        .assert()
        .success();
    let stdout = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    assert!(
        stdout.contains("dry-run"),
        "missing dry-run banner: {stdout}"
    );

    let conn = open_db(&tmp);
    assert_eq!(snapshot_count(&conn), 0);
    assert_eq!(alert_count(&conn), 0);
}

#[test]
fn check_malformed_json_records_bad_json_status() {
    let tmp = init_dir();
    write_config(
        &tmp,
        r#"
[[checkers]]
name = "spec"
command = "sh"
args = ["-c", "echo 'not-json'"]
"#,
    );
    // Single checker with a failure → exit 2 (all-failed).
    let out = driftwatch()
        .arg("check")
        .current_dir(tmp.path())
        .assert()
        .failure()
        .code(2);
    let stdout = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    assert!(
        stdout.contains("bad_json"),
        "missing bad_json status: {stdout}"
    );

    let conn = open_db(&tmp);
    assert_eq!(snapshot_count(&conn), 1);
    let status: String = conn
        .query_row("SELECT status FROM check_snapshots LIMIT 1", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(status, "bad_json");
}

#[test]
fn check_missing_executable_records_start_failed() {
    let tmp = init_dir();
    write_config(
        &tmp,
        r#"
[[checkers]]
name = "ghost"
command = "definitely-not-installed-xyz-12345"
"#,
    );
    let out = driftwatch()
        .arg("check")
        .current_dir(tmp.path())
        .assert()
        .failure()
        .code(2);
    let stdout = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    assert!(
        stdout.contains("start_failed"),
        "missing start_failed status: {stdout}"
    );

    let conn = open_db(&tmp);
    assert_eq!(snapshot_count(&conn), 1);
    let status: String = conn
        .query_row("SELECT status FROM check_snapshots LIMIT 1", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(status, "start_failed");
}

#[test]
fn check_export_after_run_includes_snapshot_and_alert() {
    let tmp = init_dir();
    write_config(
        &tmp,
        r#"
[[checkers]]
name = "spec"
command = "sh"
args = ["-c", "echo '{\"alerts\":[{\"severity\":\"warning\",\"message\":\"m\",\"source\":\"s\",\"symbol\":\"S\"}]}'"]
"#,
    );
    driftwatch()
        .arg("check")
        .current_dir(tmp.path())
        .assert()
        .success();
    let out = driftwatch()
        .args(["export", "json"])
        .current_dir(tmp.path())
        .assert()
        .success();
    let stdout = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["snapshots"].as_array().unwrap().len(), 1);
    assert_eq!(v["alerts"].as_array().unwrap().len(), 1);
    assert_eq!(v["snapshots"][0]["checker_name"], "spec");
    assert_eq!(v["snapshots"][0]["status"], "success");
    assert_eq!(v["alerts"][0]["severity"], "warning");
    assert_eq!(v["alerts"][0]["symbol"], "S");
}

#[test]
fn check_help_works() {
    driftwatch()
        .args(["check", "--help"])
        .assert()
        .success()
        .stdout(predicates::str::contains(
            "Run configured external checkers",
        ));
}

#[test]
fn check_timeout_records_timeout_status() {
    let tmp = init_dir();
    // A 5-second sleep with a 100ms timeout triggers the timeout path.
    write_config(
        &tmp,
        r#"
[[checkers]]
name = "slow"
command = "sh"
args = ["-c", "sleep 5"]
timeout_ms = 100
"#,
    );
    let out = driftwatch()
        .arg("check")
        .current_dir(tmp.path())
        .assert()
        .failure()
        .code(2);
    let stdout = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    assert!(
        stdout.contains("timeout"),
        "missing timeout status: {stdout}"
    );
    let conn = open_db(&tmp);
    let status: String = conn
        .query_row("SELECT status FROM check_snapshots LIMIT 1", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(status, "timeout");
}

#[test]
fn doctor_warns_after_a_checker_has_failed() {
    let tmp = init_dir();
    write_config(
        &tmp,
        r#"
[[checkers]]
name = "broken"
command = "sh"
args = ["-c", "exit 1"]
"#,
    );
    driftwatch()
        .arg("check")
        .current_dir(tmp.path())
        .assert()
        .failure();
    let out = driftwatch()
        .arg("doctor")
        .current_dir(tmp.path())
        .assert()
        .success();
    let stdout = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    assert!(
        stdout.contains("Checker `broken` last run failed"),
        "expected recent-run warn line: {stdout}"
    );
}

// Sanity: the public Alerts helper can read what the check command wrote.
#[test]
fn alerts_repo_reads_what_check_wrote() {
    let tmp = init_dir();
    write_config(
        &tmp,
        r#"
[[checkers]]
name = "spec"
command = "sh"
args = ["-c", "echo '{\"alerts\":[{\"severity\":\"warning\",\"message\":\"m\",\"source\":\"s\",\"symbol\":\"S\"}]}'"]
"#,
    );
    driftwatch()
        .arg("check")
        .current_dir(tmp.path())
        .assert()
        .success();
    let db = Db::open(&tmp.path().join(".driftwatch/state.db")).unwrap();
    let snaps = Alerts::new(&db).list_snapshots().unwrap();
    assert_eq!(snaps.len(), 1);
    assert_eq!(snaps[0].checker_name, "spec");
    let alerts = Alerts::new(&db).list_alerts().unwrap();
    assert_eq!(alerts.len(), 1);
    assert_eq!(alerts[0].symbol.as_deref(), Some("S"));
}

#[test]
fn check_signal_killed_records_unknown_not_failed() {
    let tmp = init_dir();
    write_config(
        &tmp,
        r#"
[[checkers]]
name = "sig"
command = "sh"
args = ["-c", "kill -KILL $$"]
"#,
    );
    let out = driftwatch()
        .arg("check")
        .current_dir(tmp.path())
        .assert()
        .failure()
        .code(2);
    let stdout = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    assert!(
        stdout.contains("unknown"),
        "missing unknown status: {stdout}"
    );

    let conn = open_db(&tmp);
    let status: String = conn
        .query_row("SELECT status FROM check_snapshots LIMIT 1", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(status, "unknown");
    // Isolation: a signalled checker must not block a healthy one.
    assert_eq!(snapshot_count(&conn), 1);
}

#[test]
fn check_cjk_stderr_does_not_panic() {
    let tmp = init_dir();
    write_config(
        &tmp,
        r#"
[[checkers]]
name = "cjk"
command = "sh"
args = ["-c", "printf '汉%.0s' $(seq 1 300) >&2; exit 1"]
"#,
    );
    let out = driftwatch()
        .arg("check")
        .current_dir(tmp.path())
        .assert()
        .failure()
        .code(2);
    let stdout = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    assert!(stdout.contains("cjk"), "missing checker row: {stdout}");
    assert!(stdout.contains("failed"), "missing failed status: {stdout}");

    let conn = open_db(&tmp);
    assert_eq!(snapshot_count(&conn), 1);
    let diag: Option<String> = conn
        .query_row("SELECT diagnostic FROM check_snapshots LIMIT 1", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert!(diag.is_some());
}

#[test]
fn check_grandchild_holding_pipe_times_out() {
    let tmp = init_dir();
    write_config(
        &tmp,
        r#"
[[checkers]]
name = "grandchild"
command = "sh"
args = ["-c", "sleep 30 & exec sleep 30"]
timeout_ms = 200
"#,
    );
    let out = driftwatch()
        .arg("check")
        .current_dir(tmp.path())
        .assert()
        .failure()
        .code(2);
    let stdout = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    assert!(stdout.contains("timeout"), "missing timeout: {stdout}");

    let conn = open_db(&tmp);
    let status: String = conn
        .query_row("SELECT status FROM check_snapshots LIMIT 1", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(status, "timeout");
}

#[test]
fn check_missing_alerts_key_is_protocol_error_not_success() {
    let tmp = init_dir();
    write_config(
        &tmp,
        r#"
[[checkers]]
name = "emptyobj"
command = "sh"
args = ["-c", "echo '{}'"]
"#,
    );
    let out = driftwatch()
        .arg("check")
        .current_dir(tmp.path())
        .assert()
        .failure()
        .code(2);
    let stdout = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    assert!(stdout.contains("bad_json"), "missing bad_json: {stdout}");
    assert!(stdout.contains("missing"), "missing diagnostic: {stdout}");
    let conn = open_db(&tmp);
    let (status, diag): (String, Option<String>) = conn
        .query_row(
            "SELECT status, diagnostic FROM check_snapshots LIMIT 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(status, "bad_json");
    assert!(
        diag.unwrap_or_default().contains("missing"),
        "diagnostic should mention missing alerts"
    );
}

#[test]
fn check_unknown_top_level_field_is_forward_compatible() {
    let tmp = init_dir();
    write_config(
        &tmp,
        r#"
[[checkers]]
name = "future"
command = "sh"
args = ["-c", "echo '{\"alerts\":[],\"newField\":1}'"]
"#,
    );
    driftwatch()
        .arg("check")
        .current_dir(tmp.path())
        .assert()
        .success();
    let conn = open_db(&tmp);
    let status: String = conn
        .query_row("SELECT status FROM check_snapshots LIMIT 1", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(status, "empty");
}

#[test]
fn check_malformed_checker_does_not_block_healthy_checker() {
    let tmp = init_dir();
    write_config(
        &tmp,
        r#"
[[checkers]]
name = "malformed"
command = "sh"
args = ["-c", "echo '{}'"]

[[checkers]]
name = "healthy"
command = "sh"
args = ["-c", "echo '{\"alerts\":[{\"severity\":\"warning\",\"message\":\"m\",\"source\":\"s\",\"symbol\":\"S\"}]}'"]
"#,
    );
    let out = driftwatch()
        .arg("check")
        .current_dir(tmp.path())
        .assert()
        .failure()
        .code(1);
    let stdout = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    assert!(
        stdout.contains("malformed"),
        "missing malformed row: {stdout}"
    );
    assert!(stdout.contains("healthy"), "missing healthy row: {stdout}");
    let conn = open_db(&tmp);
    assert_eq!(snapshot_count(&conn), 2);
    assert_eq!(alert_count(&conn), 1);
}

#[test]
fn check_duplicate_checker_name_fails_fast() {
    let tmp = init_dir();
    write_config(
        &tmp,
        r#"
[[checkers]]
name = "dup"
command = "sh"
args = ["-c", "echo '{\"alerts\":[]}'"]

[[checkers]]
name = "dup"
command = "sh"
args = ["-c", "echo '{\"alerts\":[]}'"]
"#,
    );
    let out = driftwatch()
        .arg("check")
        .current_dir(tmp.path())
        .assert()
        .failure();
    let stderr = String::from_utf8(out.get_output().stderr.clone()).unwrap();
    assert!(
        stderr.contains("duplicate checker name"),
        "got stderr: {stderr}"
    );
    let conn = open_db(&tmp);
    assert_eq!(snapshot_count(&conn), 0);
}

#[test]
fn check_working_dir_escape_fails_fast() {
    let tmp = init_dir();
    write_config(
        &tmp,
        r#"
[[checkers]]
name = "esc"
command = "sh"
args = ["-c", "echo '{\"alerts\":[]}'"]
working_dir = "../../etc"
"#,
    );
    let out = driftwatch()
        .arg("check")
        .current_dir(tmp.path())
        .assert()
        .failure();
    let stderr = String::from_utf8(out.get_output().stderr.clone()).unwrap();
    assert!(
        stderr.contains("working_dir escapes"),
        "got stderr: {stderr}"
    );
}

#[test]
fn check_dry_run_reports_parsed_counts_and_persists_nothing() {
    let tmp = init_dir();
    write_config(
        &tmp,
        r#"
[[checkers]]
name = "spec"
command = "sh"
args = ["-c", "echo '{\"alerts\":[{\"severity\":\"warning\",\"message\":\"m\",\"source\":\"s\",\"symbol\":\"S\"}]}'"]
"#,
    );
    let out = driftwatch()
        .args(["check", "--dry-run"])
        .current_dir(tmp.path())
        .assert()
        .success();
    let stdout = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    assert!(
        stdout.contains("dry-run"),
        "missing dry-run banner: {stdout}"
    );
    assert!(
        stdout.contains("parsed 1 alert"),
        "missing parsed count: {stdout}"
    );
    let conn = open_db(&tmp);
    assert_eq!(snapshot_count(&conn), 0);
}
