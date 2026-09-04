//! End-to-end tests of `driftwatch top` via the compiled binary.
//!
//! The fingerprinting change has not landed yet, so the database has no
//! `fingerprints` rows. The spec mandates that `top` then show the empty
//! state and exit successfully.

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
fn top_empty_state_after_fresh_init() {
    let tmp = init_dir();
    driftwatch()
        .arg("top")
        .current_dir(tmp.path())
        .assert()
        .success()
        .stdout(predicates::str::contains(
            "No recurring failures in the selected window.",
        ));
}

#[test]
fn top_empty_state_with_days_flag() {
    let tmp = init_dir();
    driftwatch()
        .args(["top", "--days", "1"])
        .current_dir(tmp.path())
        .assert()
        .success()
        .stdout(predicates::str::contains(
            "No recurring failures in the selected window.",
        ));
}

#[test]
fn top_empty_state_even_with_failed_runs() {
    // Failed runs without fingerprint data should still produce the empty
    // state — `top` groups by fingerprint, not by exit code.
    let tmp = init_dir();
    driftwatch()
        .args(["run", "sh", "-c", "exit 1"])
        .current_dir(tmp.path())
        .assert()
        .failure();
    driftwatch()
        .arg("top")
        .current_dir(tmp.path())
        .assert()
        .success()
        .stdout(predicates::str::contains(
            "No recurring failures in the selected window.",
        ));
}
