//! End-to-end tests for the `governance-quality` Gate concern.
//!
//! The concern runs a project-bound governance command and reports the
//! code-scoped slice of its findings. What these tests pin is that the
//! status is *earned*: the same binding, pointed at a clean report and at
//! a report carrying one matching `ERROR`, must read `PASS` and `FAIL`
//! respectively. A concern that cannot produce a failure is an
//! assumption, not a check.
//!
//! The unbound case is equally load-bearing. A project that picks a
//! product profile without binding a governance command — which is every
//! repository in the workspace today — records `UNBOUND` and blocks,
//! because the checker lives outside this repository and there is no path
//! to default to.
//!
//! Unix-only: the concern's command runs through `sh -c`.

#![cfg(unix)]

use assert_cmd::Command;
use driftwatchdog::gate::dto::GateRunResultsDocument;
use driftwatchdog::gate::types::{GateSeverity, GateStatus};
use driftwatchdog::repo::{gates::Gates, Db};
use tempfile::TempDir;

const CONCERN: &str = "governance-quality";

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

fn latest_run(tmp: &TempDir) -> driftwatchdog::repo::gates::GateRunRow {
    let db = Db::open_read_only(&tmp.path().join(".driftwatch/state.db")).unwrap();
    Gates::new(&db)
        .latest()
        .unwrap()
        .expect("a gate run was persisted")
}

fn run_doc(tmp: &TempDir) -> GateRunResultsDocument {
    let run = latest_run(tmp);
    serde_json::from_str(&run.results_json).expect("results_json is a run document")
}

fn result_of<'a>(
    doc: &'a GateRunResultsDocument,
    id: &str,
) -> &'a driftwatchdog::gate::types::GateResult {
    doc.results
        .iter()
        .find(|r| r.gate_id == id)
        .unwrap_or_else(|| panic!("result for {id} missing"))
}

fn run_gate(tmp: &TempDir) -> assert_cmd::assert::Assert {
    driftwatch().arg("gate").current_dir(tmp.path()).assert()
}

/// Write a governance report fixture and return a `sh -c` fragment that
/// prints it with the given exit status.
fn report_command(tmp: &TempDir, name: &str, report: &str, exit: i32) -> String {
    let path = tmp.path().join(name);
    std::fs::write(&path, report).unwrap();
    format!("cat {} ; exit {exit}", path.display())
}

fn finding(code: &str, level: &str, message: &str) -> String {
    format!(
        r#"{{"code":"{code}","level":"{level}","message":"{message}","project":"driftwatchdog"}}"#
    )
}

fn report(findings: &[String]) -> String {
    format!(
        r#"{{"findings":[{}],"summary":{{"errors":0,"warnings":0}}}}"#,
        findings.join(",")
    )
}

/// A `product` manifest that binds only the governance concern and
/// relaxes the sibling product concerns, so the run's status is the
/// governance concern's and nothing else.
fn bound_manifest(command: &str) -> String {
    format!(
        "version = 1\nprofile = \"product\"\n\
[[checks]]\nid = \"product-code-boundary\"\nrequired = false\n\
[[checks]]\nid = \"placeholder-threshold\"\nrequired = false\n\
[[checks]]\nid = \"{CONCERN}\"\ncommand = \"{command}\"\n"
    )
}

#[test]
fn a_clean_report_in_scope_passes() {
    let tmp = init_dir();
    let cmd = report_command(&tmp, "governance.json", &report(&[]), 0);
    std::fs::write(tmp.path().join("gate.toml"), bound_manifest(&cmd)).unwrap();

    run_gate(&tmp).success();
    let run = latest_run(&tmp);
    assert_eq!(run.status, "PASS");
    assert!(!run.blocked);
    let doc = run_doc(&tmp);
    let r = result_of(&doc, CONCERN);
    assert_eq!(r.status, GateStatus::Pass);
    assert_eq!(r.severity, GateSeverity::Info);
    assert!(r.findings.is_empty());
}

/// Task 4.6: the same binding, pointed at a fixture carrying one
/// matching `ERROR`, must return `FAIL`. Without this the `PASS` above
/// would be indistinguishable from a concern that can never fail.
#[test]
fn one_matching_error_fails_the_run_and_is_earned_not_assumed() {
    let tmp = init_dir();
    let body = report(&[finding(
        "QUALITY_README_CLAIM_CONTRADICTED",
        "ERROR",
        "README claims a check this repository does not run",
    )]);
    let cmd = report_command(&tmp, "governance.json", &body, 1);
    std::fs::write(tmp.path().join("gate.toml"), bound_manifest(&cmd)).unwrap();

    let assert = run_gate(&tmp);
    assert.failure();
    let run = latest_run(&tmp);
    assert_eq!(run.status, "FAIL");
    assert!(run.blocked, "an ERROR-severity finding must block");
    let doc = run_doc(&tmp);
    let r = result_of(&doc, CONCERN);
    assert_eq!(r.status, GateStatus::Fail);
    assert_eq!(r.severity, GateSeverity::Error);
    assert_eq!(r.findings.len(), 1);
    assert_eq!(
        r.findings[0].rule.as_deref(),
        Some("QUALITY_README_CLAIM_CONTRADICTED")
    );
    assert!(r.findings[0]
        .title
        .contains("claims a check this repository does not run"));
}

#[test]
fn one_matching_warning_is_review_required() {
    let tmp = init_dir();
    let body = report(&[finding("RULES_DRIFT_NEW", "WARNING", "a new clause gap")]);
    let cmd = report_command(&tmp, "governance.json", &body, 0);
    std::fs::write(tmp.path().join("gate.toml"), bound_manifest(&cmd)).unwrap();

    let assert = run_gate(&tmp);
    assert.failure();
    let run = latest_run(&tmp);
    assert_eq!(run.status, "REVIEW_REQUIRED");
    assert!(
        run.blocked,
        "REVIEW_REQUIRED blocks while it is in the blocking list"
    );
    let doc = run_doc(&tmp);
    let r = result_of(&doc, CONCERN);
    assert_eq!(r.status, GateStatus::ReviewRequired);
    assert_eq!(r.severity, GateSeverity::Warning);
    assert_eq!(r.findings[0].severity, GateSeverity::Warning);
}

/// The checker's own exit code is recorded, never mapped: this run exits
/// 1 on a finding the concern does not own and still reads `PASS`.
#[test]
fn an_out_of_scope_backlog_does_not_fail_the_concern() {
    let tmp = init_dir();
    let body = report(&[
        finding("PUBLICATION_UNVERIFIED", "ERROR", "unrelated backlog"),
        finding("LICENSE_MISSING", "ERROR", "another unrelated finding"),
    ]);
    let cmd = report_command(&tmp, "governance.json", &body, 1);
    std::fs::write(tmp.path().join("gate.toml"), bound_manifest(&cmd)).unwrap();

    run_gate(&tmp).success();
    let doc = run_doc(&tmp);
    let r = result_of(&doc, CONCERN);
    assert_eq!(r.status, GateStatus::Pass);
    let diagnostic = r.diagnostic.as_deref().unwrap();
    assert!(diagnostic.contains("0 of 2"), "{diagnostic}");
}

/// Task 4.5: a project that picks the profile and binds no command is
/// `UNBOUND` and blocks. This is the state of the 70 repositories that
/// have not yet opted in, and it is a real result rather than a skip.
#[test]
fn a_product_profile_without_a_governance_command_records_unbound() {
    let tmp = init_dir();
    std::fs::write(
        tmp.path().join("gate.toml"),
        "version = 1\nprofile = \"product\"\n\
[[checks]]\nid = \"product-code-boundary\"\nrequired = false\n\
[[checks]]\nid = \"placeholder-threshold\"\nrequired = false\n",
    )
    .unwrap();

    let assert = run_gate(&tmp);
    assert.failure();
    let run = latest_run(&tmp);
    assert_eq!(run.status, "FAIL");
    assert!(run.blocked);
    let doc = run_doc(&tmp);
    let r = result_of(&doc, CONCERN);
    assert_eq!(r.status, GateStatus::Unbound);
    assert_eq!(r.severity, GateSeverity::Error);
    assert_ne!(r.status, GateStatus::NotApplicable);
    assert_ne!(r.status, GateStatus::Pass);

    // The plan names it as unbound before anything runs, so the debt is
    // visible without a gate run.
    let dry = driftwatch()
        .arg("gate")
        .arg("--dry-run")
        .current_dir(tmp.path())
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&dry.stdout);
    assert!(stdout.contains(CONCERN), "dry run plan: {stdout}");
}

/// The concern is not exempable: an exemption removes a required check
/// from the gate, and here that would remove the report of a
/// contradicted claim.
#[test]
fn the_concern_may_not_be_exempted() {
    let tmp = init_dir();
    let cmd = report_command(&tmp, "governance.json", &report(&[]), 0);
    std::fs::write(
        tmp.path().join("gate.toml"),
        format!(
            "version = 1\nprofile = \"minimal\"\n\
[[checks]]\nid = \"c0\"\ncommand = \"true\"\n\
[[checks]]\nid = \"c1\"\ncommand = \"true\"\n\
[[checks]]\nid = \"c2\"\ncommand = \"true\"\n\
[[checks]]\nid = \"c3\"\ncommand = \"true\"\n\
[[checks]]\nid = \"{CONCERN}\"\n\
[not_applicable]\n{CONCERN} = \"we review this by hand\"\n"
        ),
    )
    .unwrap();
    let _ = cmd;

    run_gate(&tmp).failure();
    let run = latest_run(&tmp);
    assert_eq!(run.status, "FAIL");
    assert!(run.blocked);
    let doc = run_doc(&tmp);
    let r = result_of(&doc, CONCERN);
    assert_eq!(r.status, GateStatus::Fail);
    assert!(r.findings[0].title.contains("may never be exempted"));
    let budget = result_of(&doc, "exemption-budget");
    assert_eq!(budget.status, GateStatus::Fail);
    assert!(budget.diagnostic.as_deref().unwrap().contains(CONCERN));
}

/// Scope is a declaration of what the concern looks at, and it may
/// only shrink.
#[test]
fn a_narrowed_scope_ignores_the_codes_it_drops() {
    let tmp = init_dir();
    let body = report(&[
        finding("RULES_DRIFT_NEW", "ERROR", "outside the narrowed scope"),
        finding(
            "QUALITY_README_CLAIM_CONTRADICTED",
            "ERROR",
            "inside the narrowed scope",
        ),
    ]);
    let cmd = report_command(&tmp, "governance.json", &body, 1);
    std::fs::write(
        tmp.path().join("gate.toml"),
        format!(
            "{}[governance_quality]\ncodes = [\"QUALITY_README_CLAIM_CONTRADICTED\"]\n",
            bound_manifest(&cmd)
        ),
    )
    .unwrap();

    run_gate(&tmp).failure();
    let doc = run_doc(&tmp);
    let r = result_of(&doc, CONCERN);
    assert_eq!(r.status, GateStatus::Fail);
    assert_eq!(r.findings.len(), 1);
    assert_eq!(
        r.findings[0].rule.as_deref(),
        Some("QUALITY_README_CLAIM_CONTRADICTED")
    );
}

#[test]
fn a_widened_scope_is_refused_by_name() {
    let tmp = init_dir();
    let cmd = report_command(&tmp, "governance.json", &report(&[]), 0);
    std::fs::write(
        tmp.path().join("gate.toml"),
        format!(
            "{}[governance_quality]\ncodes = [\"LICENSE_MISSING\"]\n",
            bound_manifest(&cmd)
        ),
    )
    .unwrap();

    run_gate(&tmp).failure();
    let doc = run_doc(&tmp);
    let r = result_of(&doc, CONCERN);
    assert_eq!(r.status, GateStatus::ReviewRequired);
    let diagnostic = r.diagnostic.as_deref().unwrap();
    assert!(diagnostic.contains("LICENSE_MISSING"), "{diagnostic}");
    assert!(diagnostic.contains("does not declare"), "{diagnostic}");
}

#[test]
fn an_empty_scope_is_review_required_and_never_a_pass() {
    let tmp = init_dir();
    let cmd = report_command(&tmp, "governance.json", &report(&[]), 0);
    std::fs::write(
        tmp.path().join("gate.toml"),
        format!("{}[governance_quality]\ncodes = []\n", bound_manifest(&cmd)),
    )
    .unwrap();

    run_gate(&tmp).failure();
    let doc = run_doc(&tmp);
    let r = result_of(&doc, CONCERN);
    assert_eq!(r.status, GateStatus::ReviewRequired);
    assert_ne!(r.status, GateStatus::Pass);
    assert!(r
        .diagnostic
        .as_deref()
        .unwrap()
        .contains("empty code scope"));
}

/// A checker that did not start is `UNBOUND`: nothing ran, so nothing
/// can be claimed.
#[test]
fn a_bound_command_that_cannot_start_is_unbound() {
    let tmp = init_dir();
    let missing = tmp.path().join("no-such-governance-checker.py");
    std::fs::write(&missing, "#!/bin/sh\nexit 0\n").unwrap();
    std::fs::remove_file(&missing).unwrap();
    std::fs::write(
        tmp.path().join("gate.toml"),
        bound_manifest(missing.to_str().unwrap()),
    )
    .unwrap();

    run_gate(&tmp).failure();
    let doc = run_doc(&tmp);
    let r = result_of(&doc, CONCERN);
    assert_eq!(r.status, GateStatus::Unbound);
    let diagnostic = r.diagnostic.as_deref().unwrap();
    assert!(
        diagnostic.contains("no-such-governance-checker.py"),
        "the reason names the command that could not start: {diagnostic}"
    );
}

/// The `.ai-gate` form carries the same concern, including its scope.
#[test]
fn the_aigate_form_binds_and_scopes_the_concern() {
    let tmp = init_dir();
    let body = report(&[finding("RULES_DRIFT_NEW", "ERROR", "in scope")]);
    let cmd = report_command(&tmp, "governance.json", &body, 1);
    std::fs::create_dir_all(tmp.path().join(".ai-gate")).unwrap();
    std::fs::write(
        tmp.path().join(".ai-gate/gate.yaml"),
        format!(
            "version: 1\nruntime: driftwatchdog\nprofile: product\n\
checks:\n  {CONCERN}: true\n\
commands:\n  {CONCERN}: \"{cmd}\"\n\
governance_quality:\n  codes: [RULES_DRIFT_NEW]\n\
blocking:\n  - FAIL\n  - REVIEW_REQUIRED\n"
        ),
    )
    .unwrap();

    run_gate(&tmp).failure();
    let doc = run_doc(&tmp);
    let r = result_of(&doc, CONCERN);
    assert_eq!(r.status, GateStatus::Fail);
    assert_eq!(r.findings.len(), 1);
    assert_eq!(r.findings[0].rule.as_deref(), Some("RULES_DRIFT_NEW"));
}

/// `SPEC_PURPOSE_PLACEHOLDER` is declared ahead of the producing checker
/// shipping it, and this is the end-to-end proof that the declaration
/// took: a bound project, the default scope, a checker that reports the
/// code at ERROR and exits 1. Before the code was declared the same run
/// read `PASS` with zero findings and exit 0 — an ERROR finding gating
/// nothing. It now blocks.
#[test]
fn a_spec_purpose_placeholder_finding_blocks_a_bound_project() {
    let tmp = init_dir();
    let body = report(&[finding(
        "SPEC_PURPOSE_PLACEHOLDER",
        "ERROR",
        "spec purpose is the placeholder",
    )]);
    let cmd = report_command(&tmp, "governance.json", &body, 1);
    std::fs::write(tmp.path().join("gate.toml"), bound_manifest(&cmd)).unwrap();

    run_gate(&tmp).failure();
    let run = latest_run(&tmp);
    assert_eq!(run.status, "FAIL");
    assert!(run.blocked);
    let doc = run_doc(&tmp);
    let r = result_of(&doc, CONCERN);
    assert_eq!(r.status, GateStatus::Fail);
    assert_eq!(r.severity, GateSeverity::Error);
    assert_eq!(r.findings.len(), 1);
    assert_eq!(
        r.findings[0].rule.as_deref(),
        Some("SPEC_PURPOSE_PLACEHOLDER")
    );
    let diagnostic = r.diagnostic.as_deref().unwrap();
    assert!(diagnostic.starts_with("1 of 1"), "{diagnostic}");
}

/// The same binding against a report that does not carry the code must
/// still pass. Without this, the test above would be satisfied by a
/// concern that fails on anything, and the declaration would be
/// indistinguishable from a blanket failure.
#[test]
fn a_report_without_the_code_still_passes() {
    let tmp = init_dir();
    let cmd = report_command(&tmp, "governance.json", &report(&[]), 0);
    std::fs::write(tmp.path().join("gate.toml"), bound_manifest(&cmd)).unwrap();

    run_gate(&tmp).success();
    let doc = run_doc(&tmp);
    let r = result_of(&doc, CONCERN);
    assert_eq!(r.status, GateStatus::Pass);
    assert!(r.findings.is_empty());
    // A declared code with no finding earns no mention: the concern
    // reports observed findings, and an unemitted code is not one.
    assert!(!r
        .diagnostic
        .as_deref()
        .unwrap()
        .contains("SPEC_PURPOSE_PLACEHOLDER"));
}
