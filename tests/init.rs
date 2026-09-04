//! End-to-end test of the `driftwatch init` CLI via the compiled binary.
//!
//! Asserts the spec scenarios: "Initialize a repository" and idempotent
//! re-run with config preservation.

use assert_cmd::Command;
use tempfile::tempdir;

fn driftwatch() -> Command {
    Command::cargo_bin("driftwatch").expect("compiled driftwatch binary")
}

#[test]
fn init_creates_state_and_config() {
    let tmp = tempdir().unwrap();
    driftwatch()
        .arg("init")
        .current_dir(tmp.path())
        .assert()
        .success()
        .stdout(predicates::str::contains("initialized project at"));

    assert!(tmp.path().join(".driftwatch").is_dir());
    assert!(tmp.path().join(".driftwatch/state.db").is_file());
    assert!(tmp.path().join("driftwatch.toml").is_file());
}

#[test]
fn init_is_idempotent() {
    let tmp = tempdir().unwrap();
    driftwatch()
        .arg("init")
        .current_dir(tmp.path())
        .assert()
        .success();
    let second = driftwatch()
        .arg("init")
        .current_dir(tmp.path())
        .assert()
        .success();

    // On re-run, the toml file is not regenerated — the message should reflect
    // that it was preserved rather than created.
    let stdout = String::from_utf8(second.get_output().stdout.clone()).unwrap();
    assert!(
        !stdout.contains("created .../driftwatch.toml"),
        "second init should not recreate the config; got: {stdout}"
    );
    assert!(stdout.contains("preserved existing"));
}

#[test]
fn init_preserves_user_edited_config() {
    let tmp = tempdir().unwrap();
    let cfg = tmp.path().join("driftwatch.toml");
    std::fs::write(
        &cfg,
        "[project]\nname = \"keep-me\"\n\n[storage]\nmax_stdout_bytes = 0\n",
    )
    .unwrap();

    driftwatch()
        .arg("init")
        .current_dir(tmp.path())
        .assert()
        .success();

    let contents = std::fs::read_to_string(&cfg).unwrap();
    assert!(contents.contains("name = \"keep-me\""));
    assert!(contents.contains("max_stdout_bytes = 0"));
}

#[test]
fn init_no_config_skips_toml() {
    let tmp = tempdir().unwrap();
    driftwatch()
        .args(["init", "--no-config"])
        .current_dir(tmp.path())
        .assert()
        .success();

    assert!(tmp.path().join(".driftwatch").is_dir());
    assert!(!tmp.path().join("driftwatch.toml").exists());
}

#[test]
fn init_help_works_without_existing_project() {
    // The CLI must be usable before init; --help is the canonical case.
    driftwatch()
        .args(["init", "--help"])
        .assert()
        .success()
        .stdout(predicates::str::contains("Initialize"));
}
