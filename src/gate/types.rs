//! Gate domain types (in-memory, serializable via [`crate::gate::dto`]).

use serde::{Deserialize, Serialize};

/// Result status. Wire strings are `SCREAMING_SNAKE_CASE` and stable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum GateStatus {
    Pass,
    Fail,
    ReviewRequired,
    NotApplicable,
}

impl GateStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            GateStatus::Pass => "PASS",
            GateStatus::Fail => "FAIL",
            GateStatus::ReviewRequired => "REVIEW_REQUIRED",
            GateStatus::NotApplicable => "NOT_APPLICABLE",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "PASS" => Some(GateStatus::Pass),
            "FAIL" => Some(GateStatus::Fail),
            "REVIEW_REQUIRED" => Some(GateStatus::ReviewRequired),
            "NOT_APPLICABLE" => Some(GateStatus::NotApplicable),
            _ => None,
        }
    }
}

/// Severity of a result or finding. Orthogonal to [`GateStatus`]:
/// a `FAIL` with `Info` severity is still blocking when required.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GateSeverity {
    Info,
    Warning,
    Error,
}

impl GateSeverity {
    pub fn as_str(self) -> &'static str {
        match self {
            GateSeverity::Info => "info",
            GateSeverity::Warning => "warning",
            GateSeverity::Error => "error",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "info" | "note" | "hint" => Some(GateSeverity::Info),
            "warning" | "warn" => Some(GateSeverity::Warning),
            "error" | "fatal" | "critical" | "blocker" => Some(GateSeverity::Error),
            _ => None,
        }
    }
}

/// A single tool finding inside a result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Finding {
    pub title: String,
    pub severity: GateSeverity,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rule: Option<String>,
}

/// Bounded reference to an artifact kept under `.driftwatch/`.
/// Raw unbounded output is never embedded here; consumers follow the
/// reference and read a capped preview instead.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceRef {
    pub key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub digest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub media_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub byte_size: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preview: Option<String>,
}

/// One normalized gate result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GateResult {
    pub gate_id: String,
    pub source: String,
    pub status: GateStatus,
    pub severity: GateSeverity,
    #[serde(default)]
    pub findings: Vec<Finding>,
    #[serde(default)]
    pub evidence: Vec<EvidenceRef>,
    #[serde(default)]
    pub missing_evidence: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diagnostic: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remediation: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_round_trips() {
        for s in [
            GateStatus::Pass,
            GateStatus::Fail,
            GateStatus::ReviewRequired,
            GateStatus::NotApplicable,
        ] {
            assert_eq!(GateStatus::parse(s.as_str()), Some(s));
        }
        assert_eq!(GateStatus::parse("pass"), None);
    }

    #[test]
    fn severity_classifies_like_checker() {
        assert_eq!(GateSeverity::parse("error"), Some(GateSeverity::Error));
        assert_eq!(GateSeverity::parse("FATAL"), Some(GateSeverity::Error));
        assert_eq!(GateSeverity::parse("warn"), Some(GateSeverity::Warning));
        assert_eq!(GateSeverity::parse("nonsense"), None);
    }
}
