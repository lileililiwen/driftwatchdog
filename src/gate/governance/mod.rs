//! The `governance-quality` Gate concern: a code-scoped adapter over an
//! external governance checker's report.
//!
//! Two governance checks ship in the sibling `workspace-governance`
//! repository and gate nothing anywhere until something runs them. This
//! module makes them a real concern without turning the whole
//! governance audit into one repository's gate result: the concern
//! **declares which finding codes it owns**, runs the project-bound
//! command, keeps only the findings whose `code` is in the effective
//! code set, and derives the status from those findings' levels.
//!
//! ## Why the status comes from the findings, not the exit code
//!
//! The governance checker exits `0` when clean and `1` when it has
//! errors, and never `2`. Its exit code therefore reflects the whole
//! portfolio audit — hundreds of findings this concern does not own —
//! while a clean, correctly-scoped run over the same audit must read
//! `PASS`. Applying the repository's usual exit-code authority rule
//! here would either redden every repository for findings outside the
//! scope or hide the review class entirely. So:
//!
//! * `0` and `1` are the checker's two *expected* exit statuses and are
//!   recorded in the diagnostic, never mapped to a status;
//! * any other exit status is a checker that ran and misbehaved, which
//!   is `REVIEW_REQUIRED`;
//! * the status is the mapping in [`level_status`] applied to the
//!   **matching** findings only.
//!
//! ## Wire shape
//!
//! The producing checker's report is project-owned data, not a Gate
//! envelope: a top-level object with `findings` and `summary`, where
//! each finding carries a `code`, a `level` (`ERROR` or `WARNING`), a
//! `message`, and a `project`. Driftwatchdog embeds no governance
//! scanner and binds no checker path: a project that has not bound a
//! command is `UNBOUND`, and a command that cannot start is also
//! `UNBOUND` — nothing ran, so nothing can be claimed.
//!
//! ## No cannot-run case is a pass
//!
//! Every failure mode below yields a non-`PASS` result. `UNBOUND` is
//! reserved for "nothing ran" and a checker that started and could not
//! finish is `REVIEW_REQUIRED`; the two are never collapsed, because
//! collapsing them would let a broken checker present as a missing one.
//!
//! | Condition | Status | Exit |
//! |---|---|---|
//! | the bound command could not start (spawn failure, or the shell's not-found status) | `UNBOUND` | 1 |
//! | unexpected exit other than 0/1 and the shell's not-found status | `REVIEW_REQUIRED` | 2 |
//! | unparseable or truncated report | `REVIEW_REQUIRED` | 2 |
//! | timeout | `REVIEW_REQUIRED` | 2 |
//! | a matching finding with a level the concern does not define | `REVIEW_REQUIRED` | 2 |

use std::collections::BTreeSet;

use crate::checker::runner::{run_checker, CheckerRun};
use crate::gate::adapters::{
    default_severity_for_status, expected_exit_for_status, gate_id_for, source_for, AdapterInput,
};
use crate::gate::concerns::GOVERNANCE_QUALITY;
use crate::gate::dto::{
    MAX_DIAGNOSTIC_BYTES, MAX_FINDINGS, MAX_LOCATION_BYTES, MAX_MISSING_EVIDENCE,
    MAX_REMEDIATION_BYTES, MAX_TITLE_BYTES,
};
use crate::gate::redact::{bound_text, redact_secrets};
use crate::gate::types::{Finding, GateResult, GateSeverity, GateStatus};

/// The finding codes this concern owns, sorted for stable diagnostics.
///
/// A code is declared here once and nowhere else: the effective code
/// set may only be a subset of this list, and a project that names a
/// code outside it is refused rather than trusted.
///
/// Declaring a code and the checker emitting it are separate facts.
/// `SPEC_PURPOSE_PLACEHOLDER` is declared ahead of the producing
/// checker shipping it, so the concern is *able* to see the finding the
/// moment it exists rather than filtering it out. A declared code the
/// checker has not emitted contributes no finding and no signal, and is
/// not named in the result: declaring ahead of emission makes the gate
/// able to see a code, it does not make the gate report on it.
pub const GOVERNANCE_QUALITY_DECLARED_CODES: &[&str] = &[
    "QUALITY_README_CLAIM_CONTRADICTED",
    "QUALITY_README_CLAIM_UNSCOPED",
    "RULES_DRIFT_BASELINE_STALE",
    "RULES_DRIFT_NEW",
    "SPEC_PURPOSE_PLACEHOLDER",
];

/// The checker's two expected exit statuses: clean, and has-errors.
/// Both are recorded in the diagnostic; neither maps to a status,
/// because both are also the exit statuses of a run whose *other*
/// codes have errors.
const EXPECTED_EXITS: [i32; 2] = [0, 1];

/// The status `sh` returns for a command it could not execute.
///
/// The manifest binds a command *string*, and the runner always invokes
/// it as `sh -c`, so "the checker file is missing" reaches this adapter
/// as the shell's not-found status rather than as a spawn failure. The
/// two are the same condition — nothing ran — and design §5 keeps them
/// apart from a checker that started and misbehaved, so both map to
/// `UNBOUND`. POSIX fixes this value at 127; it is a property of how the
/// command is bound, not a path to any particular checker.
const SHELL_CANNOT_EXECUTE: i32 = 127;

/// One finding as the producing checker reports it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct GovernanceFinding {
    code: String,
    level: String,
    message: String,
    project: Option<String>,
}

/// A parsed governance report: the findings plus the total the checker
/// produced, which is reported alongside the filtered count so a
/// caller can tell a scoped result from an unscoped backlog.
#[derive(Debug, Clone, PartialEq, Eq)]
struct GovernanceReport {
    findings: Vec<GovernanceFinding>,
    /// Total findings in the report, before filtering. Counted from the
    /// array, not from `summary`, so the number the diagnostic quotes
    /// is the number of findings the checker actually emitted.
    total: usize,
}

/// Parse a governance report. Returns `None` for anything that is not a
/// JSON object carrying a `findings` array, which the caller reports as
/// unparseable output rather than as a clean run.
fn parse_report(bytes: &[u8]) -> Option<GovernanceReport> {
    let trimmed = String::from_utf8_lossy(bytes).trim().to_string();
    if !trimmed.starts_with('{') {
        return None;
    }
    let value: serde_json::Value = serde_json::from_str(&trimmed).ok()?;
    let obj = value.as_object()?;
    let arr = obj.get("findings")?.as_array()?;
    let total = arr.len();
    let mut findings = Vec::new();
    for item in arr {
        let code = item
            .get("code")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        if code.is_empty() {
            // A finding with no code names nothing this concern can
            // own, so it is not this concern's business.
            continue;
        }
        findings.push(GovernanceFinding {
            code: code.to_string(),
            level: item
                .get("level")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .trim()
                .to_string(),
            message: item
                .get("message")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string(),
            project: item
                .get("project")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
        });
    }
    Some(GovernanceReport { findings, total })
}

/// The envelope mapping, defined over the levels the governance checker
/// emits. `ERROR` dominates: a run carrying both is a `FAIL`.
///
/// Total over the two declared levels, and only over them. A third
/// level never reaches this function — the caller has already turned it
/// into `REVIEW_REQUIRED` — because a level this concern does not
/// define is a contract change, not a quieter warning.
fn level_status(has_error: bool, has_warning: bool) -> GateStatus {
    if has_error {
        GateStatus::Fail
    } else if has_warning {
        GateStatus::ReviewRequired
    } else {
        GateStatus::Pass
    }
}

/// Map one finding level onto `ERROR` / `WARNING`. Anything else — a
/// third level, a missing level, a different spelling of a known one —
/// is returned verbatim so the diagnostic can name it.
fn classify_level(level: &str) -> Result<GateSeverity, String> {
    match level.to_ascii_uppercase().as_str() {
        "ERROR" => Ok(GateSeverity::Error),
        "WARNING" => Ok(GateSeverity::Warning),
        _ => Err(level.to_string()),
    }
}

/// What a run has to say, gathered so every return site states the same
/// three things in the same shape.
struct Outcome {
    findings: Vec<Finding>,
    diagnostic: String,
    remediation: String,
    missing_evidence: Vec<String>,
}

impl Outcome {
    /// A run that reported nothing, for every path that never produced
    /// findings of its own.
    fn said(diagnostic: impl Into<String>, missing_evidence: &[&str]) -> Self {
        Self {
            findings: Vec::new(),
            diagnostic: diagnostic.into(),
            remediation: String::new(),
            missing_evidence: missing_evidence.iter().map(|s| (*s).to_string()).collect(),
        }
    }

    fn with_remediation(mut self, remediation: &str) -> Self {
        self.remediation = remediation.to_string();
        self
    }
}

/// Build a result that asserts nothing ran, or that a run could not
/// finish, without repeating the `GateResult` shape at every call site.
fn concern_result(
    tool_id: &str,
    gate_id: Option<&str>,
    status: GateStatus,
    outcome: Outcome,
    evidence: Vec<crate::gate::types::EvidenceRef>,
) -> GateResult {
    let mut missing = outcome.missing_evidence;
    missing.truncate(MAX_MISSING_EVIDENCE);
    let remediation = if outcome.remediation.is_empty() {
        "Bind a command that can finish and report for this concern."
    } else {
        &outcome.remediation
    };
    GateResult {
        gate_id: gate_id_for(tool_id, gate_id),
        source: source_for(tool_id),
        status,
        severity: default_severity_for_status(status),
        findings: outcome.findings,
        evidence,
        missing_evidence: missing,
        diagnostic: Some(bound_text(
            &redact_secrets(outcome.diagnostic.trim()),
            MAX_DIAGNOSTIC_BYTES,
        )),
        remediation: Some(bound_text(
            &redact_secrets(remediation),
            MAX_REMEDIATION_BYTES,
        )),
    }
}

/// Run the project-bound governance command and normalise its report
/// into this repository's envelope.
///
/// `codes` is the **effective** code set for this run: the concern's
/// declared set narrowed by the project's manifest. It is a scope
/// decision, not an exemption — it may only shrink, and the following
/// rules hold regardless of how it was chosen:
///
/// * a `codes` entry the concern does not declare is refused as
///   `REVIEW_REQUIRED` naming the code, before any command runs;
/// * an empty `codes` while the concern is enabled is
///   `REVIEW_REQUIRED` naming the empty scope, never `PASS` — a scope
///   that reports nothing is indistinguishable from a silently disabled
///   check;
/// * a run that cannot complete is never `PASS`: a command that could
///   not start is `UNBOUND` (exit 1), and a command that started and
///   could not finish is `REVIEW_REQUIRED` (exit 2).
///
/// The envelope mapping, by matching findings only:
///
/// | Matching findings | Status | Exit |
/// |---|---|---|
/// | none | `PASS` | 0 |
/// | one or more `WARNING` | `REVIEW_REQUIRED` | 2 |
/// | one or more `ERROR` | `FAIL` | 1 |
///
/// `ERROR` dominates a mixed run. The reported count is the filtered
/// count; the checker's total is quoted beside it and never stands in
/// for it.
pub fn run_governance_quality_adapter(
    tool_id: &str,
    gate_id: Option<&str>,
    input: &AdapterInput,
    codes: &[String],
) -> GateResult {
    let evidence = input.evidence.clone();

    // Scope is validated before execution: a configuration that cannot
    // be honoured is refused without spawning anything.
    if let Some(unsupported) = codes
        .iter()
        .find(|c| !GOVERNANCE_QUALITY_DECLARED_CODES.contains(&c.as_str()))
    {
        return concern_result(
            tool_id,
            gate_id,
            GateStatus::ReviewRequired,
            Outcome::said(
                format!(
                    "{GOVERNANCE_QUALITY} code scope names `{unsupported}`, which the concern does \
                     not declare; declared codes are: {}",
                    GOVERNANCE_QUALITY_DECLARED_CODES.join(", ")
                ),
                &[&format!("{GOVERNANCE_QUALITY}:codes")],
            )
            .with_remediation(
                "Remove the unsupported code from the configured scope; a project may narrow the \
                 concern's scope but must not widen it.",
            ),
            evidence,
        );
    }
    if codes.is_empty() {
        return concern_result(
            tool_id,
            gate_id,
            GateStatus::ReviewRequired,
            Outcome::said(
                format!(
                    "{GOVERNANCE_QUALITY} is enabled with an empty code scope, so it reports \
                     nothing and can never be told apart from a disabled check; declared codes \
                     are: {}",
                    GOVERNANCE_QUALITY_DECLARED_CODES.join(", ")
                ),
                &[&format!("{GOVERNANCE_QUALITY}:codes")],
            )
            .with_remediation(
                "Declare at least one of the concern's codes, or deselect the concern \
                 (`enabled = false`) instead of scoping it away.",
            ),
            evidence,
        );
    }
    let scope: BTreeSet<&str> = codes.iter().map(|c| c.as_str()).collect();

    let spec = input.checker_spec(tool_id);
    let run = run_checker(&spec);

    // Nothing started: there is nothing to claim, and the reason names
    // the command that could not be started.
    if let Some(detail) = run.spawn_error.clone() {
        return nothing_ran(
            tool_id,
            gate_id,
            &evidence,
            format!(
                "could not start `{}`: {detail}",
                bound_text(&spec.program, 256)
            ),
        );
    }
    // The shell could not execute what the manifest bound. The checker's
    // own stderr names the path, so the missing file is on the record.
    if run.exit_code == Some(SHELL_CANNOT_EXECUTE) {
        let stderr = if run.stderr.trim().is_empty() {
            String::new()
        } else {
            format!(" | stderr: {}", redact_secrets(run.stderr.trim()))
        };
        return nothing_ran(
            tool_id,
            gate_id,
            &evidence,
            format!(
                "the bound command did not execute (`{}`){stderr}",
                bound_text(&spec.args.join(" "), 256)
            ),
        );
    }
    if run.timed_out {
        return concern_result(
            tool_id,
            gate_id,
            GateStatus::ReviewRequired,
            Outcome::said(
                format!(
                    "{GOVERNANCE_QUALITY} exceeded its timeout of {}ms and did not finish; \
                     nothing is known about its codes",
                    input.timeout_ms
                ),
                &[
                    &format!("{GOVERNANCE_QUALITY}:timeout"),
                    &format!("{GOVERNANCE_QUALITY}:output"),
                ],
            )
            .with_remediation(
                "Raise the bound check's timeout, or make the governance checker's run finish.",
            ),
            evidence,
        );
    }
    if run.signalled {
        return concern_result(
            tool_id,
            gate_id,
            GateStatus::ReviewRequired,
            Outcome::said(
                format!(
                    "{GOVERNANCE_QUALITY} was killed by a signal before it could finish; nothing \
                     is known about its codes"
                ),
                &[&format!("{GOVERNANCE_QUALITY}:output")],
            )
            .with_remediation(
                "Re-run the bound governance check; a signalled run reports nothing.",
            ),
            evidence,
        );
    }

    let exit = run.exit_code.unwrap_or(-1);
    if !EXPECTED_EXITS.contains(&exit) {
        return concern_result(
            tool_id,
            gate_id,
            GateStatus::ReviewRequired,
            Outcome::said(
                run_evidence_diagnostic(&run, exit, "unexpected exit status"),
                &[&format!("{GOVERNANCE_QUALITY}:output")],
            )
            .with_remediation(
                "The governance checker must exit 0 (clean) or 1 (errors). Any other status means \
                 it ran and misbehaved, so its report cannot be trusted.",
            ),
            evidence,
        );
    }

    let Some(report) = parse_report(run.stdout.as_bytes()) else {
        return concern_result(
            tool_id,
            gate_id,
            GateStatus::ReviewRequired,
            Outcome::said(
                run_evidence_diagnostic(&run, exit, "unparseable governance report"),
                &[&format!("{GOVERNANCE_QUALITY}:output")],
            )
            .with_remediation(
                "The governance checker must print a JSON object with a `findings` array on \
                 stdout. Truncated or unreadable output may be hiding a finding, so it is never \
                 a pass.",
            ),
            evidence,
        );
    };

    // Filter first, then classify: a level is only this concern's
    // business on a finding this concern owns.
    let mut matched: Vec<(&GovernanceFinding, GateSeverity)> = Vec::new();
    for finding in &report.findings {
        if !scope.contains(finding.code.as_str()) {
            continue;
        }
        match classify_level(&finding.level) {
            Ok(severity) => matched.push((finding, severity)),
            Err(raw) => {
                return concern_result(
                    tool_id,
                    gate_id,
                    GateStatus::ReviewRequired,
                    Outcome::said(
                        format!(
                            "{GOVERNANCE_QUALITY} matched finding code `{}` carries unrecognised \
                             level `{raw}`; the declared levels are ERROR and WARNING",
                            finding.code
                        ),
                        &[&format!("{GOVERNANCE_QUALITY}:output")],
                    )
                    .with_remediation(
                        "The governance contract moved. Reconcile the checker's finding levels \
                         with the concern's mapping before trusting this run.",
                    ),
                    evidence,
                );
            }
        }
    }

    let mut has_error = false;
    let mut has_warning = false;
    let mut error_count = 0usize;
    let mut warning_count = 0usize;
    let mut findings: Vec<Finding> = Vec::new();
    for (finding, severity) in &matched {
        match severity {
            GateSeverity::Error => {
                has_error = true;
                error_count += 1;
            }
            GateSeverity::Warning => {
                has_warning = true;
                warning_count += 1;
            }
            GateSeverity::Info => {}
        }
        if findings.len() >= MAX_FINDINGS {
            continue;
        }
        let title = if finding.message.is_empty() {
            finding.code.clone()
        } else {
            finding.message.clone()
        };
        findings.push(Finding {
            title: bound_text(&redact_secrets(&title), MAX_TITLE_BYTES),
            severity: *severity,
            location: finding
                .project
                .as_deref()
                .map(|p| bound_text(&redact_secrets(p), MAX_LOCATION_BYTES)),
            rule: Some(bound_text(&finding.code, 256)),
        });
    }

    let status = level_status(has_error, has_warning);
    // The status is derived from the findings, so the exit code this
    // result carries is derived too. It is stated in the diagnostic
    // because the command's own exit (recorded above) is not the one
    // that maps to it.
    let expected_exit = expected_exit_for_status(status).unwrap_or(1);
    // The unmatched count is every finding the checker reported that this
    // concern did not own, not a count of this concern's own codes: a
    // fleet-wide run reports every code the workspace configures, so
    // describing the remainder as "this concern's codes" would name codes
    // the concern never declared and cannot act on.
    let diagnostic = format!(
        "{} of {} governance findings matched this concern's scope ({error_count} error, \
         {warning_count} warning, {} reported by other codes and left to the governance audit); \
         checker exit {exit} is expected for this run and does not set the status, which maps to \
         exit {expected_exit}",
        matched.len(),
        report.total,
        report.total.saturating_sub(matched.len()),
    );
    let remediation = match status {
        GateStatus::Pass => {
            "No finding in this concern's scope. Codes outside the scope stay \
                             reported by the governance audit itself."
        }
        GateStatus::Fail => {
            "Fix the matching ERROR findings, or narrow this project's scope \
                             explicitly. Scope is not an exemption: it must not be used to \
                             suppress a code the concern owns."
        }
        _ => "Resolve the matching WARNING findings, or record why the finding is acceptable.",
    };
    concern_result(
        tool_id,
        gate_id,
        status,
        Outcome {
            findings,
            diagnostic,
            remediation: remediation.to_string(),
            missing_evidence: vec![format!("{GOVERNANCE_QUALITY}:output")],
        },
        evidence,
    )
}

/// The `UNBOUND` result: there was nothing to run, so nothing is
/// claimed. Shared by the two ways a run fails to start (the runner
/// could not spawn, and the shell could not execute) so the two cannot
/// drift into presenting differently.
fn nothing_ran(
    tool_id: &str,
    gate_id: Option<&str>,
    evidence: &[crate::gate::types::EvidenceRef],
    detail: String,
) -> GateResult {
    concern_result(
        tool_id,
        gate_id,
        GateStatus::Unbound,
        Outcome::said(
            format!(
                "{GOVERNANCE_QUALITY} had nothing to run: {detail}; nothing ran, so nothing is \
                 claimed"
            ),
            &[&format!("{GOVERNANCE_QUALITY}:command")],
        )
        .with_remediation(
            "Bind a command that exists for this concern, or deselect the concern. A check that \
             cannot start is UNBOUND, never a pass.",
        ),
        evidence.to_vec(),
    )
}

/// Diagnostic for a run that started and could not be read: the exit
/// status and the redacted stderr, which is the evidence such a run
/// actually has.
fn run_evidence_diagnostic(run: &CheckerRun, exit: i32, headline: &str) -> String {
    let mut detail = format!("{headline}; exit {exit}");
    if !run.stderr.trim().is_empty() {
        detail.push_str(" | stderr: ");
        detail.push_str(&redact_secrets(run.stderr.trim()));
    } else if run.stdout_truncated {
        detail.push_str(" | stdout truncated");
    }
    detail
}

#[cfg(test)]
mod tests;
