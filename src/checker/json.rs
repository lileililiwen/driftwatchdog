//! Machine-readable checker-report envelope for `driftwatch check`.
//!
//! The renderer is a pure projection of the per-checker
//! [`CheckerOutcome`](super::report::CheckerOutcome) values the runner
//! already produces. It must not change execution, isolation, persistence
//! or the human-format text. Every diagnostic that does not belong in
//! the document (the `--dry-run` banner, persistence notes, the
//! `correlation skipped` warning) is emitted to stderr by the caller
//! in JSON mode; only the document itself goes to stdout.
//!
//! The contract id is `driftwatch-checker/0.1.0`; future additions
//! appear as new top-level fields, which consumers must ignore
//! (forward-compat rule of the sibling protocol).
//!
//! The status vocabulary is five short, screen-friendly tokens:
//!
//! * `ok` — checker exited zero with a valid alerts document that
//!   carried no alerts (covers both `Status::Success` with zero alerts
//!   and `Status::Empty`).
//! * `alerting` — checker exited zero with a valid alerts document
//!   that produced one or more alerts.
//! * `failed` — checker exited nonzero, could not be spawned, or was
//!   killed by a signal (the `Status::Failed` / `Status::StartFailed`
//!   / `Status::Unknown` cases all collapse here).
//! * `timeout` — checker exceeded its configured timeout.
//! * `protocol-error` — checker emitted invalid JSON or a document
//!   that failed the alerts protocol.
//!
//! Per-checker `alerts` reuse the same wire vocabulary the
//! [`super::protocol::parse_alerts_document`] function accepts, so
//! downstream consumers need one parser, not two.

use std::collections::BTreeMap;

use serde::Serialize;

use super::report::{CheckerOutcome, Status};

/// Versioned contract id. Bump only when the wire shape changes in a
/// non-additive way (renamed field, removed field, changed type).
pub const CHECKER_REPORT_CONTRACT: &str = "driftwatch-checker/0.1.0";

/// Per-alert row in the document. Mirrors the existing checker wire
/// shape so consumers that already parse the alerts protocol need no
/// second parser. `extra` carries any unknown JSON fields the parser
/// preserved.
#[derive(Debug, Serialize)]
pub struct CheckerReportAlert {
    pub severity: Option<String>,
    pub message: Option<String>,
    pub source: Option<String>,
    pub symbol: Option<String>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty", default)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

/// Per-checker row in the document. `error` carries the bounded
/// diagnostic text for failure modes that did not parse an alert list;
/// it is `None` for `ok` / `alerting` outcomes and is always present
/// for `failed` / `timeout` / `protocol-error`.
#[derive(Debug, Serialize)]
pub struct CheckerReportRow {
    pub name: String,
    pub status: &'static str,
    pub alerts: Vec<CheckerReportAlert>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub error: Option<String>,
}

/// Summary counts derived from the per-checker rows. The status
/// vocabulary is fixed to the five contract strings, so the keys
/// never carry dynamic data.
#[derive(Debug, Serialize)]
pub struct CheckerReportSummary {
    pub total: usize,
    pub ok: usize,
    pub alerting: usize,
    pub failed: usize,
    pub timeout: usize,
    pub protocol_error: usize,
    pub alerts: usize,
}

/// Top-level document. The order of fields is the order consumers
/// should be able to rely on.
#[derive(Debug, Serialize)]
pub struct CheckerReportDocument {
    pub contract: &'static str,
    pub tool: &'static str,
    pub version: &'static str,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub project: Option<String>,
    pub generated_at: String,
    pub checkers: Vec<CheckerReportRow>,
    pub summary: CheckerReportSummary,
}

/// Project identity carried into the document. `None` for non-project
/// invocations so the field is omitted in that case.
pub struct ProjectInfo<'a> {
    pub root: Option<&'a std::path::Path>,
}

impl CheckerReportDocument {
    /// Build a document from the outcomes the runner already produced.
    /// The project root is included when known so consumers can confirm
    /// which directory the run targeted; it is `None` (and the field
    /// is omitted) for non-project invocations.
    pub fn build(
        outcomes: &[CheckerOutcome],
        project: ProjectInfo<'_>,
        generated_at: String,
    ) -> Self {
        let mut checkers = Vec::with_capacity(outcomes.len());
        let mut total = 0usize;
        let mut ok = 0usize;
        let mut alerting = 0usize;
        let mut failed = 0usize;
        let mut timeout = 0usize;
        let mut protocol_error = 0usize;
        let mut alerts_total = 0usize;
        for o in outcomes {
            total += 1;
            let token = match o.status {
                Status::Success if o.alert_count > 0 => {
                    alerting += 1;
                    "alerting"
                }
                Status::Success => {
                    ok += 1;
                    "ok"
                }
                Status::Empty => {
                    ok += 1;
                    "ok"
                }
                Status::Failed => {
                    failed += 1;
                    "failed"
                }
                Status::Timeout => {
                    timeout += 1;
                    "timeout"
                }
                Status::BadJson => {
                    protocol_error += 1;
                    "protocol-error"
                }
                Status::StartFailed => {
                    failed += 1;
                    "failed"
                }
                Status::Unknown => {
                    failed += 1;
                    "failed"
                }
            };
            alerts_total += o.alert_count;
            // The `error` field is reserved for failure modes that did
            // not parse an alert list (`failed` / `timeout` /
            // `protocol-error`). The diagnostic for `ok` and `alerting`
            // rows is a human-oriented note ("parsed N alerts") that
            // would only confuse a JSON consumer, so it stays out of
            // the document.
            let error = match o.status {
                Status::Success | Status::Empty => None,
                Status::Failed
                | Status::Timeout
                | Status::BadJson
                | Status::StartFailed
                | Status::Unknown => o.diagnostic.clone(),
            };
            let row = CheckerReportRow {
                name: o.name.clone(),
                status: token,
                alerts: o
                    .alerts
                    .iter()
                    .map(|a| CheckerReportAlert {
                        severity: a.severity.clone(),
                        message: a.message.clone(),
                        source: a.source.clone(),
                        symbol: a.symbol.clone(),
                        extra: a.extra.clone(),
                    })
                    .collect(),
                error,
            };
            checkers.push(row);
        }
        let summary = CheckerReportSummary {
            total,
            ok,
            alerting,
            failed,
            timeout,
            protocol_error,
            alerts: alerts_total,
        };
        let project_root = project.root.map(|p| p.to_string_lossy().into_owned());
        Self {
            contract: CHECKER_REPORT_CONTRACT,
            tool: "driftwatchdog",
            version: env!("CARGO_PKG_VERSION"),
            project: project_root,
            generated_at,
            checkers,
            summary,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checker::protocol::DriftAlert;
    use std::collections::BTreeMap;

    fn outcome(
        name: &str,
        status: Status,
        alert_count: usize,
        diagnostic: Option<&str>,
    ) -> CheckerOutcome {
        CheckerOutcome {
            name: name.to_string(),
            status,
            alert_count,
            diagnostic: diagnostic.map(str::to_string),
            alerts: vec![],
            duration_ms: 0,
        }
    }

    fn outcome_with_alerts(name: &str, status: Status, alerts: Vec<DriftAlert>) -> CheckerOutcome {
        let count = alerts.len();
        CheckerOutcome {
            name: name.to_string(),
            status,
            alert_count: count,
            diagnostic: None,
            alerts,
            duration_ms: 0,
        }
    }

    #[test]
    fn empty_outcome_renders_empty_array() {
        let doc = CheckerReportDocument::build(&[], ProjectInfo { root: None }, "t".to_string());
        assert_eq!(doc.checkers.len(), 0);
        assert_eq!(doc.summary.total, 0);
        assert_eq!(doc.summary.alerts, 0);
        assert_eq!(doc.contract, "driftwatch-checker/0.1.0");
        assert_eq!(doc.tool, "driftwatchdog");
        assert!(doc.project.is_none());
    }

    #[test]
    fn mixed_outcomes_map_to_five_status_tokens() {
        let outcomes = vec![
            outcome("clean", Status::Empty, 0, Some("parsed 0 alerts")),
            outcome("alerting", Status::Success, 2, Some("parsed 2 alert(s)")),
            outcome("broken", Status::Failed, 0, Some("exit 1")),
            outcome("slow", Status::Timeout, 0, Some("exceeded 100 ms")),
            outcome("malformed", Status::BadJson, 0, Some("missing alerts")),
        ];
        let doc = CheckerReportDocument::build(&outcomes, ProjectInfo { root: None }, "t".into());
        assert_eq!(doc.checkers.len(), 5);
        assert_eq!(doc.checkers[0].status, "ok");
        assert_eq!(doc.checkers[1].status, "alerting");
        assert_eq!(doc.checkers[2].status, "failed");
        assert_eq!(doc.checkers[3].status, "timeout");
        assert_eq!(doc.checkers[4].status, "protocol-error");
        // Declaration order is preserved so the consumer can map rows
        // back to the configuration without an extra key.
        let names: Vec<String> = doc.checkers.iter().map(|r| r.name.clone()).collect();
        assert_eq!(
            names,
            vec!["clean", "alerting", "broken", "slow", "malformed"]
        );
        // `error` is reserved for the failure modes: clean and
        // alerting rows keep the human diagnostic out of the
        // document, the three failure modes keep it.
        assert!(doc.checkers[0].error.is_none());
        assert!(doc.checkers[1].error.is_none());
        assert!(doc.checkers[2].error.is_some());
        assert!(doc.checkers[3].error.is_some());
        assert!(doc.checkers[4].error.is_some());
        // Summary counts match the per-row mapping.
        assert_eq!(doc.summary.total, 5);
        assert_eq!(doc.summary.ok, 1);
        assert_eq!(doc.summary.alerting, 1);
        assert_eq!(doc.summary.failed, 1);
        assert_eq!(doc.summary.timeout, 1);
        assert_eq!(doc.summary.protocol_error, 1);
        assert_eq!(doc.summary.alerts, 2);
    }

    #[test]
    fn success_with_zero_alerts_is_ok_not_alerting() {
        // The protocol allows a checker to exit zero with `{"alerts":[]}`
        // (Status::Empty) or with `{"alerts":[...0 alerts]}`; the latter
        // is technically Success with alert_count = 0 and must still
        // map to `ok`, not `alerting`.
        let outcomes = vec![outcome("zero", Status::Success, 0, None)];
        let doc = CheckerReportDocument::build(&outcomes, ProjectInfo { root: None }, "t".into());
        assert_eq!(doc.checkers[0].status, "ok");
        assert_eq!(doc.summary.ok, 1);
        assert_eq!(doc.summary.alerting, 0);
    }

    #[test]
    fn start_failed_and_unknown_collapse_to_failed() {
        let outcomes = vec![
            outcome("spawn", Status::StartFailed, 0, Some("missing executable")),
            outcome("killed", Status::Unknown, 0, Some("signal")),
        ];
        let doc = CheckerReportDocument::build(&outcomes, ProjectInfo { root: None }, "t".into());
        assert_eq!(doc.checkers[0].status, "failed");
        assert_eq!(doc.checkers[1].status, "failed");
        assert_eq!(doc.summary.failed, 2);
    }

    #[test]
    fn alerts_rows_preserve_wire_fields_and_extra_map() {
        let mut extra = BTreeMap::new();
        extra.insert("line".to_string(), serde_json::json!(42));
        let alerts = vec![DriftAlert {
            severity: Some("warning".to_string()),
            message: Some("msg".to_string()),
            source: Some("src".to_string()),
            symbol: Some("sym".to_string()),
            extra,
        }];
        let outcomes = vec![outcome_with_alerts("ok", Status::Success, alerts)];
        let doc = CheckerReportDocument::build(&outcomes, ProjectInfo { root: None }, "t".into());
        assert_eq!(doc.checkers.len(), 1);
        assert_eq!(doc.checkers[0].alerts.len(), 1);
        let a = &doc.checkers[0].alerts[0];
        assert_eq!(a.severity.as_deref(), Some("warning"));
        assert_eq!(a.message.as_deref(), Some("msg"));
        assert_eq!(a.source.as_deref(), Some("src"));
        assert_eq!(a.symbol.as_deref(), Some("sym"));
        assert_eq!(a.extra.get("line"), Some(&serde_json::json!(42)));
    }

    #[test]
    fn empty_alerts_array_is_always_present_per_checker() {
        let outcomes = vec![outcome("clean", Status::Empty, 0, None)];
        let doc = CheckerReportDocument::build(&outcomes, ProjectInfo { root: None }, "t".into());
        let json = serde_json::to_string(&doc).unwrap();
        // The literal `"alerts":[]` substring must appear, not just an
        // omitted field, so consumers can iterate `checkers[].alerts`
        // uniformly.
        assert!(
            json.contains("\"alerts\":[]"),
            "missing empty alerts marker: {json}"
        );
    }

    #[test]
    fn project_root_is_omitted_when_unknown() {
        let outcomes = vec![outcome("a", Status::Empty, 0, None)];
        let doc = CheckerReportDocument::build(&outcomes, ProjectInfo { root: None }, "t".into());
        let json = serde_json::to_string(&doc).unwrap();
        assert!(
            !json.contains("\"project\""),
            "project field leaked: {json}"
        );
    }

    #[test]
    fn project_root_is_included_when_known() {
        let dir = tempfile::tempdir().unwrap();
        let outcomes = vec![outcome("a", Status::Empty, 0, None)];
        let doc = CheckerReportDocument::build(
            &outcomes,
            ProjectInfo {
                root: Some(dir.path()),
            },
            "t".into(),
        );
        let json = serde_json::to_string(&doc).unwrap();
        assert!(json.contains("\"project\":"), "missing project: {json}");
    }

    #[test]
    fn error_field_is_omitted_when_absent() {
        let outcomes = vec![outcome("clean", Status::Empty, 0, None)];
        let doc = CheckerReportDocument::build(&outcomes, ProjectInfo { root: None }, "t".into());
        let json = serde_json::to_string(&doc).unwrap();
        // The clean row has no `error` field; the absence of the key
        // is the contract.
        assert!(!json.contains("\"error\""), "error field leaked: {json}");
    }
}
