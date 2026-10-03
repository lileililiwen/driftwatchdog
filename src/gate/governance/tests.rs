//! Unit tests for the `governance-quality` adapter.
//!
//! Kept in a submodule file so the adapter's production code and its
//! test surface stay separately readable, and so neither grows past the
//! repository's own 1,000-line source-size limit.

use super::*;
use std::collections::BTreeMap;

/// A scope of the full declared set, which is what a project that
/// configures nothing gets.
fn declared_scope() -> Vec<String> {
    GOVERNANCE_QUALITY_DECLARED_CODES
        .iter()
        .map(|c| (*c).to_string())
        .collect()
}

fn gq_input(program: &str, args: &[&str]) -> AdapterInput {
    AdapterInput {
        program: program.to_string(),
        args: args.iter().map(|s| s.to_string()).collect(),
        working_dir: std::env::temp_dir(),
        env: BTreeMap::new(),
        timeout_ms: 10_000,
        max_output_bytes: 1024 * 1024,
        evidence: vec![],
    }
}

/// Write a governance report and return a `sh -c` line that prints
/// it with the given exit status.
fn report_exit(dir: &std::path::Path, report: &str, exit: i32) -> String {
    let path = dir.join("report.json");
    std::fs::write(&path, report).unwrap();
    format!("cat {} ; exit {}", path.display(), exit)
}

fn finding(code: &str, level: &str, message: &str, project: &str) -> String {
    format!(r#"{{"code":"{code}","level":"{level}","message":"{message}","project":"{project}"}}"#)
}

fn report(findings: &[String]) -> String {
    format!(
        r#"{{"findings":[{}],"summary":{{"errors":0,"warnings":0}}}}"#,
        findings.join(",")
    )
}

fn run(script: &str, codes: &[String]) -> GateResult {
    run_governance_quality_adapter(
        "project-runtime",
        Some(GOVERNANCE_QUALITY),
        &gq_input("sh", &["-c", script]),
        codes,
    )
}

// --- Envelope mapping, one test per cell -------------------------

#[test]
fn no_matching_findings_passes_with_exit_zero() {
    let tmp = tempfile::tempdir().unwrap();
    let body = report(&[finding(
        "PUBLICATION_UNVERIFIED",
        "ERROR",
        "an unrelated audit finding",
        "argoset",
    )]);
    let script = report_exit(tmp.path(), &body, 1);
    let result = run(&script, &declared_scope());
    assert_eq!(result.status, GateStatus::Pass);
    assert_eq!(expected_exit_for_status(result.status), Some(0));
    assert!(result.findings.is_empty(), "no finding may be reported");
    // The checker's own exit was 1; it must not have set the status.
    assert!(result.diagnostic.as_deref().unwrap().contains("0 of 1"));
}

#[test]
fn matching_warning_is_review_required_with_exit_two() {
    let tmp = tempfile::tempdir().unwrap();
    let body = report(&[finding(
        "QUALITY_README_CLAIM_UNSCOPED",
        "WARNING",
        "claim is unscoped",
        "opendockify",
    )]);
    let script = report_exit(tmp.path(), &body, 0);
    let result = run(&script, &declared_scope());
    assert_eq!(result.status, GateStatus::ReviewRequired);
    assert_eq!(expected_exit_for_status(result.status), Some(2));
    assert_eq!(result.findings.len(), 1);
    assert_eq!(result.findings[0].severity, GateSeverity::Warning);
    assert_eq!(
        result.findings[0].rule.as_deref(),
        Some("QUALITY_README_CLAIM_UNSCOPED")
    );
}

#[test]
fn matching_error_fails_with_exit_one() {
    let tmp = tempfile::tempdir().unwrap();
    let body = report(&[finding(
        "RULES_DRIFT_NEW",
        "ERROR",
        "a new clause gap",
        "driftwatchdog",
    )]);
    let script = report_exit(tmp.path(), &body, 1);
    let result = run(&script, &declared_scope());
    assert_eq!(result.status, GateStatus::Fail);
    assert_eq!(expected_exit_for_status(result.status), Some(1));
    assert_eq!(result.findings.len(), 1);
    assert_eq!(result.findings[0].severity, GateSeverity::Error);
}

#[test]
fn error_dominates_a_mixed_run() {
    let tmp = tempfile::tempdir().unwrap();
    let body = report(&[
        finding("RULES_DRIFT_NEW", "WARNING", "a warning finding", "one"),
        finding(
            "QUALITY_README_CLAIM_CONTRADICTED",
            "ERROR",
            "a contradicting claim",
            "two",
        ),
    ]);
    let script = report_exit(tmp.path(), &body, 1);
    let result = run(&script, &declared_scope());
    assert_eq!(result.status, GateStatus::Fail);
    assert_eq!(expected_exit_for_status(result.status), Some(1));
    assert_eq!(result.findings.len(), 2, "both matching findings report");
}

// --- Failure path, one test per row -----------------------------

#[test]
fn missing_checker_is_unbound_with_exit_one() {
    let missing = std::env::temp_dir().join("driftwatchdog-no-such-governance-checker");
    std::fs::write(&missing, "#!/bin/sh\nexit 0\n").unwrap();
    std::fs::remove_file(&missing).unwrap();
    let result = run_governance_quality_adapter(
        "project-runtime",
        Some(GOVERNANCE_QUALITY),
        &gq_input(missing.to_str().unwrap(), &["--json"]),
        &declared_scope(),
    );
    assert_eq!(result.status, GateStatus::Unbound);
    assert_eq!(expected_exit_for_status(result.status), Some(1));
    // The reason names the command that could not be started.
    let diagnostic = result.diagnostic.as_deref().unwrap();
    assert!(
        diagnostic.contains("driftwatchdog-no-such-governance-checker"),
        "{diagnostic}"
    );
}

/// The manifest binds a command *string* and the runner invokes it
/// as `sh -c`, so a missing checker file arrives here as the shell's
/// not-found status. It is still "nothing ran", so it stays
/// `UNBOUND` rather than collapsing into a review condition.
#[test]
fn a_shell_that_cannot_execute_the_command_is_unbound_with_exit_one() {
    let tmp = tempfile::tempdir().unwrap();
    let missing = tmp.path().join("no-such-governance-checker.py");
    std::fs::write(&missing, "#!/bin/sh\nexit 0\n").unwrap();
    std::fs::remove_file(&missing).unwrap();
    let script = missing.to_str().unwrap();
    let result = run(script, &declared_scope());
    assert_eq!(result.status, GateStatus::Unbound);
    assert_eq!(expected_exit_for_status(result.status), Some(1));
    let diagnostic = result.diagnostic.as_deref().unwrap();
    assert!(
        diagnostic.contains("no-such-governance-checker.py"),
        "{diagnostic}"
    );
    assert!(diagnostic.contains("nothing ran"), "{diagnostic}");
}

/// 127 is the one non-expected exit that means "never started"; the
/// other non-expected statuses are a checker that ran and misbehaved,
/// and must not be reported as unbound.
#[test]
fn a_near_miss_exit_status_is_still_a_review_condition() {
    let tmp = tempfile::tempdir().unwrap();
    for code in [2, 3, 126, 128] {
        let script = report_exit(tmp.path(), &report(&[]), code);
        let result = run(&script, &declared_scope());
        assert_eq!(
            result.status,
            GateStatus::ReviewRequired,
            "exit {code} is a misbehaved checker, not a missing one"
        );
        assert_eq!(expected_exit_for_status(result.status), Some(2));
    }
}

#[test]
fn unexpected_exit_is_review_required_with_exit_two() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("report.json");
    std::fs::write(&path, report(&[])).unwrap();
    let script = format!("cat {} ; echo boom >&2 ; exit 7", path.display());
    let result = run(&script, &declared_scope());
    assert_eq!(result.status, GateStatus::ReviewRequired);
    assert_eq!(expected_exit_for_status(result.status), Some(2));
    let diagnostic = result.diagnostic.as_deref().unwrap();
    assert!(diagnostic.contains("exit 7"), "{diagnostic}");
    assert!(
        diagnostic.contains("boom"),
        "stderr is the evidence: {diagnostic}"
    );
}

#[test]
fn unparseable_report_is_review_required_with_exit_two() {
    let tmp = tempfile::tempdir().unwrap();
    let truncated = r#"{"findings":[{"code":"RULES_DRIFT_NEW","level":"ERR"#;
    let script = report_exit(tmp.path(), truncated, 0);
    let result = run(&script, &declared_scope());
    assert_eq!(result.status, GateStatus::ReviewRequired);
    assert_eq!(expected_exit_for_status(result.status), Some(2));
    assert!(result
        .diagnostic
        .as_deref()
        .unwrap()
        .contains("unparseable governance report"));
}

#[test]
fn report_without_a_findings_array_is_review_required() {
    let tmp = tempfile::tempdir().unwrap();
    let script = report_exit(tmp.path(), r#"{"summary":{"errors":0}}"#, 0);
    let result = run(&script, &declared_scope());
    assert_eq!(result.status, GateStatus::ReviewRequired);
    assert_eq!(expected_exit_for_status(result.status), Some(2));
}

#[test]
fn timeout_is_review_required_with_exit_two() {
    let mut input = gq_input("sh", &["-c", "sleep 5 ; echo {}"]);
    input.timeout_ms = 150;
    let result = run_governance_quality_adapter(
        "project-runtime",
        Some(GOVERNANCE_QUALITY),
        &input,
        &declared_scope(),
    );
    assert_eq!(result.status, GateStatus::ReviewRequired);
    assert_eq!(expected_exit_for_status(result.status), Some(2));
    assert!(result
        .missing_evidence
        .contains(&format!("{GOVERNANCE_QUALITY}:timeout")));
}

#[test]
fn unrecognised_matching_level_is_review_required_with_exit_two() {
    let tmp = tempfile::tempdir().unwrap();
    let body = report(&[finding(
        "RULES_DRIFT_BASELINE_STALE",
        "INFO",
        "a third level moved in",
        "one",
    )]);
    let script = report_exit(tmp.path(), &body, 0);
    let result = run(&script, &declared_scope());
    assert_eq!(result.status, GateStatus::ReviewRequired);
    assert_eq!(expected_exit_for_status(result.status), Some(2));
    let diagnostic = result.diagnostic.as_deref().unwrap();
    assert!(
        diagnostic.contains("unrecognised level `INFO`"),
        "{diagnostic}"
    );
}

/// The property the failure-path table exists to protect: not one
/// cannot-run case may report a pass.
#[test]
fn no_cannot_run_case_reports_pass() {
    let tmp = tempfile::tempdir().unwrap();
    let clean = report(&[]);
    let cases: Vec<(&str, GateStatus)> = vec![
        (
            "unexpected exit",
            run(
                &format!("cat {} ; exit 3", tmp.path().join("a.json").display()),
                &declared_scope(),
            )
            .status,
        ),
        (
            "unparseable",
            run(
                &report_exit(tmp.path(), "not json at all", 0),
                &declared_scope(),
            )
            .status,
        ),
        (
            "third level",
            run(
                &report_exit(
                    tmp.path(),
                    &report(&[finding("RULES_DRIFT_NEW", "NOTICE", "moved", "one")]),
                    0,
                ),
                &declared_scope(),
            )
            .status,
        ),
        (
            "empty scope",
            run(&report_exit(tmp.path(), &clean, 0), &[]).status,
        ),
        (
            "widened scope",
            run(
                &report_exit(tmp.path(), &clean, 0),
                &["LICENSE_MISSING".to_string()],
            )
            .status,
        ),
    ];
    assert!(!clean.is_empty());
    for (label, status) in cases {
        assert_ne!(status, GateStatus::Pass, "{label} must not report a pass");
        assert_ne!(
            status,
            GateStatus::NotApplicable,
            "{label} must not claim absence"
        );
    }
}

// --- Scope versus exemption --------------------------------------

#[test]
fn reported_count_is_the_filtered_count_not_the_total() {
    const BACKLOG: usize = 211;
    let tmp = tempfile::tempdir().unwrap();
    let mut findings: Vec<String> = Vec::new();
    for _ in 0..BACKLOG {
        findings.push(finding(
            "PUBLICATION_UNVERIFIED",
            "ERROR",
            "unrelated audit backlog",
            "argoset",
        ));
    }
    for code in GOVERNANCE_QUALITY_DECLARED_CODES {
        findings.push(finding(code, "ERROR", "in scope", "driftwatchdog"));
    }
    // Derived from the declared set rather than written out, so adding a
    // code cannot leave this test pinning a stale count.
    let in_scope = GOVERNANCE_QUALITY_DECLARED_CODES.len();
    let body = report(&findings);
    let script = report_exit(tmp.path(), &body, 1);
    let result = run(&script, &declared_scope());
    let diagnostic = result.diagnostic.as_deref().unwrap();
    assert_eq!(
        result.findings.len(),
        in_scope,
        "the filtered count is reported"
    );
    assert!(
        diagnostic.starts_with(&format!("{in_scope} of {}", BACKLOG + in_scope)),
        "{diagnostic}"
    );
    // The total is quoted, never reported as the result: a 211-
    // finding backlog outside the scope must not be carried into
    // this concern's findings.
    for f in &result.findings {
        let rule = f.rule.as_deref().unwrap_or_default();
        assert!(
            GOVERNANCE_QUALITY_DECLARED_CODES.contains(&rule),
            "reported rule {rule} is outside the declared scope"
        );
    }
    assert!(!result
        .findings
        .iter()
        .any(|f| f.title.contains("unrelated audit backlog")));
}

#[test]
fn a_project_may_narrow_the_scope() {
    let tmp = tempfile::tempdir().unwrap();
    let body = report(&[
        finding(
            "RULES_DRIFT_NEW",
            "ERROR",
            "out of the narrowed scope",
            "one",
        ),
        finding(
            "QUALITY_README_CLAIM_CONTRADICTED",
            "ERROR",
            "in the narrowed scope",
            "two",
        ),
    ]);
    let script = report_exit(tmp.path(), &body, 1);
    let result = run(&script, &["QUALITY_README_CLAIM_CONTRADICTED".to_string()]);
    assert_eq!(result.status, GateStatus::Fail);
    assert_eq!(result.findings.len(), 1);
    assert_eq!(
        result.findings[0].rule.as_deref(),
        Some("QUALITY_README_CLAIM_CONTRADICTED")
    );
}

#[test]
fn a_project_may_not_widen_the_scope() {
    let tmp = tempfile::tempdir().unwrap();
    let script = report_exit(tmp.path(), &report(&[]), 0);
    let result = run(&script, &["LICENSE_MISSING".to_string()]);
    assert_eq!(result.status, GateStatus::ReviewRequired);
    assert_eq!(expected_exit_for_status(result.status), Some(2));
    let diagnostic = result.diagnostic.as_deref().unwrap();
    assert!(diagnostic.contains("LICENSE_MISSING"), "{diagnostic}");
    assert!(diagnostic.contains("does not declare"), "{diagnostic}");
}

#[test]
fn an_empty_scope_is_a_configuration_error_not_a_pass() {
    let tmp = tempfile::tempdir().unwrap();
    // A perfectly clean report, scoped to nothing: still a
    // configuration error, because a scope reporting nothing is
    // indistinguishable from a disabled check.
    let script = report_exit(tmp.path(), &report(&[]), 0);
    let result = run(&script, &[]);
    assert_eq!(result.status, GateStatus::ReviewRequired);
    assert_eq!(expected_exit_for_status(result.status), Some(2));
    let diagnostic = result.diagnostic.as_deref().unwrap();
    assert!(diagnostic.contains("empty code scope"), "{diagnostic}");
}

#[test]
fn a_codeless_finding_is_not_this_concerns_business() {
    let tmp = tempfile::tempdir().unwrap();
    let body = r#"{"findings":[{"level":"ERROR","message":"anonymous"}],"summary":{}}"#.to_string();
    let script = report_exit(tmp.path(), &body, 1);
    let result = run(&script, &declared_scope());
    assert_eq!(result.status, GateStatus::Pass);
    assert!(result.findings.is_empty());
}

// --- Declaring a code the checker has not emitted yet -------------
//
// `SPEC_PURPOSE_PLACEHOLDER` is declared ahead of the producing
// checker shipping it. These two tests pin both halves of what that
// means, because only measuring it settles whether declaring ahead is
// safe: the first says a declared, unemitted code is inert and says so
// in the result, the second says the moment the checker emits it the
// gate reports it. The first is not a licence to declare anything; it
// is the measurement that makes the second trustworthy.

#[test]
fn a_declared_code_the_checker_has_not_emitted_is_inert() {
    let tmp = tempfile::tempdir().unwrap();
    // A report carrying only out-of-scope findings, run against the
    // full declared set — which contains a code the checker cannot
    // yet emit.
    let body = report(&[finding(
        "LICENSE_MISSING",
        "ERROR",
        "an unrelated finding",
        "argoset",
    )]);
    let script = report_exit(tmp.path(), &body, 1);
    let result = run(&script, &declared_scope());
    // Declaring an unemittable code is not a verdict and not a
    // failure: the result is exactly what it was before the code was
    // declared.
    assert_eq!(result.status, GateStatus::Pass);
    assert_eq!(expected_exit_for_status(result.status), Some(0));
    assert!(result.findings.is_empty());
    let diagnostic = result.diagnostic.as_deref().unwrap();
    // And it earns no mention either. A declared code with no finding
    // is indistinguishable from a code that was never declared, which
    // is the structural weakness design.md §5 records: this repo
    // cannot detect a declared/emitted divergence on its own.
    assert!(
        !diagnostic.contains("SPEC_PURPOSE_PLACEHOLDER"),
        "an unemitted code must not appear in the result: {diagnostic}"
    );
}

#[test]
fn spec_purpose_placeholder_is_owed_once_the_checker_emits_it() {
    let tmp = tempfile::tempdir().unwrap();
    let body = report(&[finding(
        "SPEC_PURPOSE_PLACEHOLDER",
        "ERROR",
        "spec purpose is the placeholder",
        "driftwatchdog",
    )]);
    let script = report_exit(tmp.path(), &body, 1);
    let result = run(&script, &declared_scope());
    // The whole point of declaring the code ahead of emission: an
    // ERROR finding that is filtered out and reads PASS is the D18
    // failure class, and this is the assertion that it no longer is.
    assert_eq!(result.status, GateStatus::Fail);
    assert_eq!(expected_exit_for_status(result.status), Some(1));
    assert_eq!(result.findings.len(), 1);
    assert_eq!(result.findings[0].severity, GateSeverity::Error);
    assert_eq!(
        result.findings[0].rule.as_deref(),
        Some("SPEC_PURPOSE_PLACEHOLDER")
    );
}

#[test]
fn a_project_may_narrow_its_scope_to_the_new_code_alone() {
    let tmp = tempfile::tempdir().unwrap();
    let body = report(&[finding(
        "SPEC_PURPOSE_PLACEHOLDER",
        "ERROR",
        "spec purpose is the placeholder",
        "driftwatchdog",
    )]);
    let script = report_exit(tmp.path(), &body, 1);
    // Naming the new code in a manifest is no longer a refused
    // widening: it is the narrowest legal scope there is, and it
    // reports the finding.
    let result = run(&script, &["SPEC_PURPOSE_PLACEHOLDER".to_string()]);
    assert_eq!(result.status, GateStatus::Fail);
    assert_eq!(result.findings.len(), 1);
    assert_eq!(
        result.findings[0].rule.as_deref(),
        Some("SPEC_PURPOSE_PLACEHOLDER")
    );
}
