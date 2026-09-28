//! End-to-end tests for the deployable Compose/CI Gate profile.

#![cfg(unix)]

use assert_cmd::Command;
use driftwatchdog::repo::{gates::Gates, Db};
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

fn latest_status(tmp: &tempfile::TempDir) -> (String, bool, String) {
    let db = Db::open_read_only(&tmp.path().join(".driftwatch/state.db")).unwrap();
    let run = Gates::new(&db).latest().unwrap().unwrap();
    (run.status, run.blocked, run.results_json)
}

#[test]
fn deployable_profile_requires_both_contract_bindings() {
    let tmp = init_dir();
    std::fs::write(tmp.path().join("gate.toml"), "profile = \"deployable\"\n").unwrap();

    driftwatch()
        .arg("gate")
        .current_dir(tmp.path())
        .assert()
        .failure();

    let (status, blocked, results) = latest_status(&tmp);
    assert_eq!(status, "REVIEW_REQUIRED");
    assert!(blocked);
    assert!(results.contains("compose-contract:command"));
    assert!(results.contains("ci-contract:command"));
}

#[test]
fn deployable_profile_passes_when_both_project_commands_pass() {
    let tmp = init_dir();
    std::fs::write(
        tmp.path().join("gate.toml"),
        r#"profile = "deployable"
[[checks]]
id = "compose-contract"
command = "true"
[[checks]]
id = "ci-contract"
command = "true"
"#,
    )
    .unwrap();

    driftwatch()
        .arg("gate")
        .current_dir(tmp.path())
        .assert()
        .success();

    let (status, blocked, _) = latest_status(&tmp);
    assert_eq!(status, "PASS");
    assert!(!blocked);
}

#[test]
fn deployable_profile_blocks_when_a_contract_command_fails() {
    let tmp = init_dir();
    std::fs::write(
        tmp.path().join("gate.toml"),
        r#"profile = "deployable"
[[checks]]
id = "compose-contract"
command = "false"
[[checks]]
id = "ci-contract"
command = "true"
"#,
    )
    .unwrap();

    driftwatch()
        .arg("gate")
        .current_dir(tmp.path())
        .assert()
        .failure();

    let (status, blocked, results) = latest_status(&tmp);
    assert_eq!(status, "FAIL");
    assert!(blocked);
    assert!(results.contains("compose-contract"));
}
