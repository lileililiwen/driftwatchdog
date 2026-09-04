//! End-to-end tests of `driftwatch doctor`.

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

#[test]
fn doctor_exits_zero_on_fresh_init() {
    let tmp = init_dir();
    let out = driftwatch()
        .arg("doctor")
        .current_dir(tmp.path())
        .assert()
        .success();
    let stdout = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    // The summary line must be present.
    assert!(
        stdout.contains("All ") && stdout.contains("checks passed"),
        "missing summary: {stdout}"
    );
    // Each well-known check id appears.
    for id in ["db.open", "config.parse", "dir.state", "dir.project"] {
        assert!(stdout.contains(id), "missing {id} in: {stdout}");
    }
}

#[test]
fn doctor_warns_on_missing_git_context() {
    // The temp dir is not a Git worktree. The git check must be a WARN
    // (not a FAIL) so the overall exit code is still 0.
    let tmp = init_dir();
    let out = driftwatch()
        .arg("doctor")
        .current_dir(tmp.path())
        .assert()
        .success();
    let stdout = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    assert!(
        stdout.contains("[WARN] Git context unavailable"),
        "expected git warn line: {stdout}"
    );
}

#[test]
fn doctor_fails_on_malformed_config() {
    let tmp = init_dir();
    // Overwrite the config with invalid TOML.
    std::fs::write(
        tmp.path().join("driftwatch.toml"),
        "this is not valid TOML = = =",
    )
    .unwrap();
    let out = driftwatch()
        .arg("doctor")
        .current_dir(tmp.path())
        .assert()
        .failure()
        .code(2);
    let stdout = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    assert!(
        stdout.contains("[FAIL] driftwatch.toml is invalid"),
        "expected config fail line: {stdout}"
    );
    // A remediation line is provided.
    assert!(stdout.contains("remediation:"));
}

#[test]
fn doctor_warns_on_missing_optional_checker() {
    let tmp = init_dir();
    std::fs::write(
        tmp.path().join("driftwatch.toml"),
        r#"
[[checkers]]
name = "missing-tool"
command = "definitely-not-installed-tool-xyz"
"#,
    )
    .unwrap();
    let out = driftwatch()
        .arg("doctor")
        .current_dir(tmp.path())
        .assert()
        .success();
    let stdout = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    // A missing optional checker is a WARN, not a FAIL.
    assert!(
        stdout.contains("[WARN] Checker `missing-tool` is not on PATH"),
        "expected missing-checker warn: {stdout}"
    );
}

#[test]
fn doctor_help_works() {
    driftwatch()
        .arg("doctor")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicates::str::contains("Diagnose"));
}

#[test]
fn doctor_remediation_lines_never_include_env_values() {
    // A regression check: the doctor must not include any environment
    // variable values in the report.
    let tmp = init_dir();
    let out = driftwatch()
        .arg("doctor")
        .env("SECRET_TEST_VALUE_DO_NOT_LEAK", "hunter2")
        .current_dir(tmp.path())
        .assert()
        .success();
    let stdout = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    assert!(
        !stdout.contains("hunter2"),
        "doctor leaked an env value: {stdout}"
    );
    assert!(
        !stdout.contains("SECRET_TEST_VALUE_DO_NOT_LEAK"),
        "doctor leaked env name: {stdout}"
    );
}
