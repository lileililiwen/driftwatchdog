//! Compatibility adapter: existing checker outcomes → gate results.
//!
//! The current `driftwatch check` protocol and its SQLite snapshots are
//! unchanged. This adapter only *reads* a [`CheckerOutcome`] and returns
//! the equivalent normalized [`GateResult`] so future gate plans can
//! consume old checkers without re-running them.
//!
//! Mapping:
//!
//! * `Empty` (zero alerts) → `PASS`, no findings;
//! * `Success` (alerts present) → `FAIL` with one finding per alert;
//! * `Failed` / `Timeout` / `BadJson` / `StartFailed` / `Unknown` →
//!   `REVIEW_REQUIRED` with the checker output listed as missing
//!   evidence and a bounded, secret-safe diagnostic.

use crate::checker::{CheckerOutcome, Status};
use crate::gate::dto::{MAX_DIAGNOSTIC_BYTES, MAX_LOCATION_BYTES, MAX_TITLE_BYTES};
use crate::gate::redact::bound_text;
use crate::gate::types::{Finding, GateResult, GateSeverity, GateStatus};

fn finding_from_alert(message: &str, severity: &str, source: &str, symbol: &str) -> Finding {
    let severity = GateSeverity::parse(severity).unwrap_or(GateSeverity::Warning);
    let mut location = source.to_string();
    if !symbol.is_empty() {
        location.push(':');
        location.push_str(symbol);
    }
    Finding {
        title: bound_text(message, MAX_TITLE_BYTES),
        severity,
        location: Some(bound_text(&location, MAX_LOCATION_BYTES)),
        rule: None,
    }
}

/// Adapt one checker outcome to its equivalent gate result.
pub fn adapt_checker_outcome(outcome: &CheckerOutcome) -> GateResult {
    let gate_id = bound_text(&outcome.name, 128);
    let source = format!("checker:{}", outcome.name);
    let source = bound_text(&source, 256);
    let diagnostic = outcome
        .diagnostic
        .as_deref()
        .map(|s| bound_text(s, MAX_DIAGNOSTIC_BYTES));

    match outcome.status {
        Status::Empty => GateResult {
            gate_id,
            source,
            status: GateStatus::Pass,
            severity: GateSeverity::Info,
            findings: vec![],
            evidence: vec![],
            missing_evidence: vec![],
            diagnostic,
            remediation: None,
        },
        Status::Success => {
            let mut top = GateSeverity::Info;
            let mut findings = Vec::with_capacity(outcome.alerts.len().min(256));
            for alert in outcome.alerts.iter().take(256) {
                let message = alert.message.as_deref().unwrap_or("");
                let severity_str = alert.severity.as_deref().unwrap_or("warning");
                let sev = GateSeverity::parse(severity_str).unwrap_or(GateSeverity::Warning);
                if severity_rank(sev) > severity_rank(top) {
                    top = sev;
                }
                findings.push(finding_from_alert(
                    message,
                    severity_str,
                    alert.source.as_deref().unwrap_or(""),
                    alert.symbol.as_deref().unwrap_or(""),
                ));
            }
            // Classify from the raw severity strings so a checker that
            // invents a new word still follows the checker default
            // (unknown → warning) via `finding_from_alert`'s fallback.
            GateResult {
                gate_id,
                source,
                status: GateStatus::Fail,
                severity: top,
                findings,
                evidence: vec![],
                missing_evidence: vec![],
                diagnostic,
                remediation: Some(
                    "Investigate the reported findings and add regression coverage.".to_string(),
                ),
            }
        }
        Status::Failed
        | Status::Timeout
        | Status::BadJson
        | Status::StartFailed
        | Status::Unknown => GateResult {
            gate_id: gate_id.clone(),
            source,
            status: GateStatus::ReviewRequired,
            severity: GateSeverity::Warning,
            findings: vec![],
            evidence: vec![],
            missing_evidence: vec![format!("checker:{0}:output", outcome.name)],
            diagnostic,
            remediation: Some(
                "Re-run the checker; resolve tool or output errors before trusting this gate."
                    .to_string(),
            ),
        },
    }
}

fn severity_rank(s: GateSeverity) -> u8 {
    match s {
        GateSeverity::Info => 0,
        GateSeverity::Warning => 1,
        GateSeverity::Error => 2,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checker::protocol::DriftAlert;
    use std::collections::BTreeMap;

    fn outcome(status: Status, alerts: Vec<DriftAlert>) -> CheckerOutcome {
        CheckerOutcome {
            name: "arch".into(),
            status,
            alert_count: alerts.len(),
            diagnostic: Some("parsed 1 alert(s)".into()),
            alerts,
            duration_ms: 5,
        }
    }

    fn alert(severity: &str) -> DriftAlert {
        DriftAlert {
            severity: Some(severity.into()),
            message: Some("Connection timeout does not follow spec".into()),
            source: Some("specs/database.md".into()),
            symbol: Some("DbPool".into()),
            extra: BTreeMap::new(),
        }
    }

    #[test]
    fn empty_maps_to_pass() {
        let r = adapt_checker_outcome(&outcome(Status::Empty, vec![]));
        assert_eq!(r.status, GateStatus::Pass);
        assert!(r.findings.is_empty());
        assert!(r.missing_evidence.is_empty());
    }

    #[test]
    fn success_with_high_severity_maps_to_fail_with_source_location() {
        let r = adapt_checker_outcome(&outcome(Status::Success, vec![alert("error")]));
        assert_eq!(r.status, GateStatus::Fail);
        assert_eq!(r.severity, GateSeverity::Error);
        assert_eq!(r.findings.len(), 1);
        assert!(r.findings[0]
            .location
            .as_deref()
            .unwrap()
            .contains("DbPool"));
        assert!(r.source.starts_with("checker:"));
        assert!(r.remediation.is_some());
    }

    #[test]
    fn infra_failures_map_to_review_with_missing_evidence() {
        for s in [
            Status::Failed,
            Status::Timeout,
            Status::BadJson,
            Status::StartFailed,
            Status::Unknown,
        ] {
            let r = adapt_checker_outcome(&outcome(s, vec![]));
            assert_eq!(r.status, GateStatus::ReviewRequired, "{s:?}");
            assert_eq!(r.missing_evidence, vec!["checker:arch:output".to_string()]);
        }
    }

    #[test]
    fn diagnostic_secrets_are_redacted() {
        let mut o = outcome(Status::Empty, vec![]);
        o.diagnostic = Some("api_key=topsecret123".into());
        let r = adapt_checker_outcome(&o);
        assert!(!r.diagnostic.as_deref().unwrap().contains("topsecret123"));
    }
}
