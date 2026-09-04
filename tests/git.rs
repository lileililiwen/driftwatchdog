//! Git metadata collection: the spec mandates that Git is never required for
//! initialization or for command execution. This file asserts the
//! "non-fatal Git failure" behavior.

use assert_cmd::Command;
use driftwatchdog::project::git;
use tempfile::tempdir;

#[test]
fn init_outside_git_succeeds() {
    let tmp = tempdir().unwrap();
    // The fresh tempdir is not a Git worktree.
    Command::cargo_bin("driftwatchdog")
        .expect("binary")
        .arg("init")
        .current_dir(tmp.path())
        .assert()
        .success();
    assert!(tmp.path().join(".driftwatch/state.db").is_file());
}

#[test]
fn git_capture_in_non_git_dir_returns_diagnostic() {
    let tmp = tempdir().unwrap();
    let ctx = git::capture(tmp.path());
    // We are explicitly NOT in a Git worktree, so commit/branch should be
    // None and the diagnostic should be populated.
    assert!(ctx.commit.is_none());
    assert!(ctx.branch.is_none());
    assert!(ctx.diagnostic.is_some());
}
