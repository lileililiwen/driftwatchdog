//! End-to-end tests for the per-check gate timeout.
//!
//! Two things have to be true at once and neither implies the other:
//! the resolved deadline must be *visible* on the dry-run plan, and it
//! must be *enforced* when the check runs. A field that renders but
//! does not bound, or that bounds but cannot be seen, is the same
//! invisible timeout this change exists to remove.
//!
//! Unix-only because the bound commands are shell-driven for control
//! over their runtime.

#![cfg(unix)]

use assert_cmd::Command;
use driftwatchdog::gate::dto::GateRunResultsDocument;
use driftwatchdog::gate::types::GateStatus;
use driftwatchdog::repo::{gates::Gates, Db};
use tempfile::TempDir;

fn driftwatch() -> Command {
    Command::cargo_bin("driftwatchdog").expect("compiled driftwatch binary")
}

fn init_dir() -> TempDir {
    let tmp = tempfile::tempdir().unwrap();
    driftwatch()
        .arg("init")
        .current_dir(tmp.path())
        .assert()
        .success();
    tmp
}

fn governance_command(tmp: &TempDir) -> String {
    let path = tmp.path().join("governance.json");
    std::fs::write(
        &path,
        r#"{"findings":[],"summary":{"errors":0,"warnings":0}}"#,
    )
    .unwrap();
    format!("cat {}", path.display())
}

fn open_db(tmp: &TempDir) -> Db {
    Db::open_read_only(&tmp.path().join(".driftwatch/state.db")).expect("open db")
}

/// The effective deadline of every scheduled check must be readable
/// before anything executes. A manifest that declares nothing is the
/// case that matters most here: the 60s default is exactly the value
/// that was invisible for 76 manifests.
#[test]
fn dry_run_prints_the_effective_timeout_of_every_check() {
    let tmp = init_dir();
    std::fs::write(
        tmp.path().join("gate.toml"),
        "profile = \"minimal\"\n\
         [[checks]]\nid = \"slow\"\ncommand = \"sleep 5\"\ntimeout_ms = 900000\n\
         [[checks]]\nid = \"smoke\"\ncommand = \"true\"\n",
    )
    .unwrap();
    let out = driftwatch()
        .arg("gate")
        .arg("--dry-run")
        .current_dir(tmp.path())
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&out.get_output().stdout).to_string();
    assert!(stdout.contains("check timeouts"), "{stdout}");
    // Declared value, and the default for the check that declared none.
    assert!(stdout.contains("slow: 900000ms"), "{stdout}");
    assert!(stdout.contains("smoke: 60000ms"), "{stdout}");
}

/// **Negative control: the declared deadline is honored, not ignored.**
///
/// `timeout_ms = 1000` against `sleep 5`. If the field were ignored the
/// 60s default would apply, the sleep would run to completion, exit 0
/// would be recorded and the gate would report `PASS` — so this test
/// fails exactly when the wiring breaks. The duration bound closes the
/// other door: a run that simply waited out 5s and then reported a
/// timeout for some unrelated reason also fails.
#[test]
fn a_declared_timeout_kills_the_command_before_it_finishes() {
    let tmp = init_dir();
    let governance = governance_command(&tmp);
    std::fs::write(
        tmp.path().join("gate.toml"),
        format!("profile = \"minimal\"\n[[checks]]\nid = \"governance-quality\"\ncommand = \"{governance}\"\n[[checks]]\nid = \"slow\"\ncommand = \"sleep 5\"\ntimeout_ms = 1000\n"),
    )
    .unwrap();

    let started = std::time::Instant::now();
    let assert = driftwatch()
        .arg("gate")
        .current_dir(tmp.path())
        .assert()
        .failure();
    let elapsed = started.elapsed();

    let stderr = String::from_utf8_lossy(&assert.get_output().stderr).to_string();
    assert!(
        elapsed < std::time::Duration::from_secs(4),
        "took {elapsed:?}; the 1000ms deadline did not take effect (stderr: {stderr})"
    );

    // A timeout is REVIEW_REQUIRED and blocks, distinct from the FAIL a
    // non-zero exit produces and from the PASS an ignored field gives.
    let db = open_db(&tmp);
    let run = Gates::new(&db).latest().unwrap().unwrap();
    assert_eq!(run.status, "REVIEW_REQUIRED");
    assert!(run.blocked, "a timeout must block: {stderr}");
    let doc: GateRunResultsDocument = serde_json::from_str(&run.results_json).unwrap();
    let r = doc
        .results
        .iter()
        .find(|r| r.gate_id == "slow")
        .expect("the check reported");
    assert_eq!(r.status, GateStatus::ReviewRequired);
    assert!(
        r.missing_evidence
            .iter()
            .any(|k| k == "project-runtime:timeout"),
        "timeout evidence missing from {:?}",
        r.missing_evidence
    );
}

/// The counter-case that keeps the control honest: a check that
/// declares no timeout and finishes inside the default still passes.
/// Without this, a gate that failed everything would also satisfy the
/// test above.
#[test]
fn a_quick_check_under_the_default_still_passes() {
    let tmp = init_dir();
    let governance = governance_command(&tmp);
    std::fs::write(
        tmp.path().join("gate.toml"),
        format!("profile = \"minimal\"\n[[checks]]\nid = \"governance-quality\"\ncommand = \"{governance}\"\n[[checks]]\nid = \"smoke\"\ncommand = \"true\"\n"),
    )
    .unwrap();
    driftwatch()
        .arg("gate")
        .current_dir(tmp.path())
        .assert()
        .success();
    let db = open_db(&tmp);
    let run = Gates::new(&db).latest().unwrap().unwrap();
    assert_eq!(run.status, "PASS");
    assert!(!run.blocked);
}

/// The business YAML form is bounded the same way, through the same
/// resolved value. The deadline named in `timeouts:` must reach the
/// adapter, not just the plan.
#[test]
fn the_business_yaml_form_honors_a_declared_timeout() {
    let tmp = init_dir();
    let governance = governance_command(&tmp);
    std::fs::create_dir_all(tmp.path().join(".ai-gate")).unwrap();
    std::fs::write(
        tmp.path().join(".ai-gate/gate.yaml"),
        format!("version: 1\nruntime: driftwatchdog\nprofile: minimal\nchecks:\n  slow: true\n  governance-quality: true\ncommands:\n  slow: \"sleep 5\"\n  governance-quality: \"{governance}\"\ntimeouts:\n  slow: 1000\n"),
    )
    .unwrap();

    let started = std::time::Instant::now();
    driftwatch()
        .arg("gate")
        .current_dir(tmp.path())
        .assert()
        .failure();
    let elapsed = started.elapsed();
    assert!(
        elapsed < std::time::Duration::from_secs(4),
        "took {elapsed:?}; the YAML-declared deadline did not take effect"
    );

    let db = open_db(&tmp);
    let run = Gates::new(&db).latest().unwrap().unwrap();
    assert_eq!(run.status, "REVIEW_REQUIRED");
}

/// An unusable deadline is refused at parse time, before any check
/// runs, with the cap named so the fix is a number the operator can
/// read.
#[test]
fn an_unusable_timeout_is_refused_before_anything_executes() {
    for (declared, must_name) in [("0", "omit the key"), ("3600001", "3600000")] {
        let tmp = init_dir();
        let governance = governance_command(&tmp);
        std::fs::write(
            tmp.path().join("gate.toml"),
            format!(
                "profile = \"minimal\"\n[[checks]]\nid = \"governance-quality\"\ncommand = \"{governance}\"\n[[checks]]\nid = \"bad\"\ncommand = \"true\"\ntimeout_ms = {declared}\n"
            ),
        )
        .unwrap();
        let assert = driftwatch()
            .arg("gate")
            .current_dir(tmp.path())
            .assert()
            .failure();
        let stderr = String::from_utf8_lossy(&assert.get_output().stderr).to_string();
        assert!(stderr.contains("bad"), "declared={declared}: {stderr}");
        assert!(stderr.contains(must_name), "declared={declared}: {stderr}");
        // Refused before execution: no run was recorded.
        let db = open_db(&tmp);
        assert!(
            Gates::new(&db).latest().unwrap().is_none(),
            "declared={declared}: a refused manifest still executed and persisted"
        );
    }
}
