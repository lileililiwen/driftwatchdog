//! End-to-end tests of `driftwatch list` via the compiled binary.

#![cfg(unix)]

use assert_cmd::Command;
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

#[test]
fn list_empty_filter_after_fresh_init() {
    let tmp = init_dir();
    driftwatch()
        .arg("list")
        .current_dir(tmp.path())
        .assert()
        .success()
        .stdout(predicates::str::contains(
            "No runs match the current filter.",
        ));
}

#[test]
fn list_after_runs_shows_rows_newest_first() {
    let tmp = init_dir();
    for arg in ["one", "two", "three"] {
        driftwatch()
            .args(["run", "echo", arg])
            .current_dir(tmp.path())
            .assert()
            .success();
    }
    let out = driftwatch()
        .arg("list")
        .current_dir(tmp.path())
        .assert()
        .success();
    let stdout = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    // newest first: "three" appears before "one"
    let pos_three = stdout.find("echo three").expect("echo three present");
    let pos_one = stdout.find("echo one").expect("echo one present");
    assert!(
        pos_three < pos_one,
        "expected three before one, got:\n{stdout}"
    );
}

#[test]
fn list_filter_failed_excludes_successful() {
    let tmp = init_dir();
    driftwatch()
        .args(["run", "echo", "ok"])
        .current_dir(tmp.path())
        .assert()
        .success();
    driftwatch()
        .args(["run", "sh", "-c", "exit 3"])
        .current_dir(tmp.path())
        .assert()
        .failure();

    let out = driftwatch()
        .args(["list", "--failed"])
        .current_dir(tmp.path())
        .assert()
        .success();
    let stdout = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    assert!(stdout.contains("sh -c exit 3"), "got:\n{stdout}");
    assert!(
        !stdout.contains("echo ok"),
        "should not contain successful run"
    );
}

#[test]
fn list_filter_tag_excludes_untagged() {
    let tmp = init_dir();
    driftwatch()
        .args(["run", "echo", "no-tag"])
        .current_dir(tmp.path())
        .assert()
        .success();
    driftwatch()
        .args(["run", "--tag", "auth", "echo", "tagged"])
        .current_dir(tmp.path())
        .assert()
        .success();

    let out = driftwatch()
        .args(["list", "--tag", "auth"])
        .current_dir(tmp.path())
        .assert()
        .success();
    let stdout = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    assert!(stdout.contains("echo tagged"));
    assert!(!stdout.contains("echo no-tag"));
}

#[test]
fn list_limit_caps_rows() {
    let tmp = init_dir();
    for i in 0..5 {
        driftwatch()
            .args(["run", "echo", &format!("arg{i}")])
            .current_dir(tmp.path())
            .assert()
            .success();
    }
    let out = driftwatch()
        .args(["list", "--limit", "2"])
        .current_dir(tmp.path())
        .assert()
        .success();
    let stdout = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    // We expect 2 data rows: the first 3 should not appear.
    assert!(stdout.contains("arg4"));
    assert!(stdout.contains("arg3"));
    assert!(!stdout.contains("arg0"), "should be capped; got:\n{stdout}");
}

#[test]
fn list_emoji_command_does_not_panic() {
    let tmp = init_dir();
    // Long emoji argv straddles the 79-byte truncation cut point.
    let big = format!("{}{}", "🎉".repeat(40), "tail");
    driftwatch()
        .args(["run", "echo", &big])
        .current_dir(tmp.path())
        .assert()
        .success();
    let out = driftwatch()
        .arg("list")
        .current_dir(tmp.path())
        .assert()
        .success();
    let stdout = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    assert!(stdout.contains("echo"), "got:\n{stdout}");
}
