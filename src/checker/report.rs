//! Human-readable status reporting for the `check` command.
//!
//! The `check` subcommand groups the many failure modes of a checker
//! (missing executable, nonzero exit, timeout, malformed output, valid
//! output with zero alerts) into a small enum so the CLI summary is
//! consistent and machine-parseable.

use std::fmt;

use super::protocol::DriftAlert;

/// Coarse status of a checker invocation, recorded on the
/// `check_snapshots.status` column.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// Checker exited zero with a valid `{"alerts": [...]}` document.
    Success,
    /// Checker exited zero and the alerts array was empty.
    Empty,
    /// Checker exited nonzero.
    Failed,
    /// Checker exceeded the configured timeout.
    Timeout,
    /// Checker output was not valid JSON or failed protocol validation.
    BadJson,
    /// The checker's executable could not be started.
    StartFailed,
    /// A signal terminated the child before it could produce a status.
    Unknown,
}

impl Status {
    /// Stable string representation. Used both in the database and in
    /// the CLI summary. Do not change without a migration.
    pub fn as_str(self) -> &'static str {
        match self {
            Status::Success => "success",
            Status::Empty => "empty",
            Status::Failed => "failed",
            Status::Timeout => "timeout",
            Status::BadJson => "bad_json",
            Status::StartFailed => "start_failed",
            Status::Unknown => "unknown",
        }
    }

    /// A failed-status checker is one the user almost certainly wants
    /// to look at. Used to compute the aggregate exit code.
    pub fn is_failure(self) -> bool {
        !matches!(self, Status::Success | Status::Empty)
    }
}

impl fmt::Display for Status {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Numeric severity bucket, derived from the `severity` string of each
/// alert. Used to order the per-checker summary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Info,
    Warning,
    Error,
}

impl Severity {
    /// Best-effort classification. Unknown severities default to
    /// `Warning` so a checker that invents a new string still gets
    /// visible in the report.
    pub fn classify(s: &str) -> Self {
        match s.to_ascii_lowercase().as_str() {
            "error" | "fatal" | "critical" | "blocker" => Severity::Error,
            "info" | "note" | "hint" => Severity::Info,
            _ => Severity::Warning,
        }
    }
}

/// Per-checker outcome, surfaced in the CLI summary and used to compute
/// the aggregate exit code.
#[derive(Debug, Clone)]
pub struct CheckerOutcome {
    pub name: String,
    pub status: Status,
    pub alert_count: usize,
    pub diagnostic: Option<String>,
    pub alerts: Vec<DriftAlert>,
    pub duration_ms: i64,
}

impl CheckerOutcome {
    /// One-line human-readable label used in the CLI summary table.
    pub fn label(&self) -> String {
        match self.status {
            Status::Success => format!("ok ({} alerts)", self.alert_count),
            Status::Empty => "ok (no alerts)".into(),
            Status::Failed => format!("failed ({} alerts)", self.alert_count),
            Status::Timeout => "timeout".into(),
            Status::BadJson => "bad_json".into(),
            Status::StartFailed => "start_failed".into(),
            Status::Unknown => "unknown".into(),
        }
    }
}

/// Map a [`Status`] to a short, screen-friendly label. Useful when a
/// caller wants to print a single word per row.
pub fn label_for_status(s: Status) -> &'static str {
    s.as_str()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_str_is_stable() {
        assert_eq!(Status::Success.as_str(), "success");
        assert_eq!(Status::Empty.as_str(), "empty");
        assert_eq!(Status::Failed.as_str(), "failed");
        assert_eq!(Status::Timeout.as_str(), "timeout");
        assert_eq!(Status::BadJson.as_str(), "bad_json");
        assert_eq!(Status::StartFailed.as_str(), "start_failed");
        assert_eq!(Status::Unknown.as_str(), "unknown");
    }

    #[test]
    fn is_failure_distinguishes_outcomes() {
        assert!(!Status::Success.is_failure());
        assert!(!Status::Empty.is_failure());
        assert!(Status::Failed.is_failure());
        assert!(Status::Timeout.is_failure());
        assert!(Status::BadJson.is_failure());
        assert!(Status::StartFailed.is_failure());
        assert!(Status::Unknown.is_failure());
    }

    #[test]
    fn severity_classify_uses_defaults_for_unknown() {
        assert_eq!(Severity::classify("error"), Severity::Error);
        assert_eq!(Severity::classify("fatal"), Severity::Error);
        assert_eq!(Severity::classify("warning"), Severity::Warning);
        assert_eq!(Severity::classify("warn"), Severity::Warning);
        assert_eq!(Severity::classify("info"), Severity::Info);
        assert_eq!(Severity::classify("note"), Severity::Info);
        assert_eq!(Severity::classify("nonsense"), Severity::Warning);
    }

    #[test]
    fn outcome_label_reflects_status() {
        let o = CheckerOutcome {
            name: "x".into(),
            status: Status::Success,
            alert_count: 3,
            diagnostic: None,
            alerts: vec![],
            duration_ms: 10,
        };
        assert_eq!(o.label(), "ok (3 alerts)");

        let empty = CheckerOutcome {
            status: Status::Empty,
            ..o.clone()
        };
        assert_eq!(empty.label(), "ok (no alerts)");

        let timeout = CheckerOutcome {
            status: Status::Timeout,
            ..o.clone()
        };
        assert_eq!(timeout.label(), "timeout");
    }
}
