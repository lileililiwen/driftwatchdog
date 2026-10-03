//! Gate domain types (in-memory, serializable via [`crate::gate::dto`]).

use serde::{Deserialize, Serialize};

/// Result status. Wire strings are `SCREAMING_SNAKE_CASE` and stable.
///
/// `UNBOUND` is deliberately distinct from `NOT_APPLICABLE`: a bound
/// command that reports "not applicable" is a *proven* exemption, while
/// `UNBOUND` means the check was asserted and no command was ever
/// bound to it, so nothing ran. The two claims are opposites and must
/// never share a status, a severity, or a findings list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum GateStatus {
    Pass,
    Fail,
    ReviewRequired,
    NotApplicable,
    /// A required check bound no command and declared no exemption.
    Unbound,
}

impl GateStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            GateStatus::Pass => "PASS",
            GateStatus::Fail => "FAIL",
            GateStatus::ReviewRequired => "REVIEW_REQUIRED",
            GateStatus::NotApplicable => "NOT_APPLICABLE",
            GateStatus::Unbound => "UNBOUND",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "PASS" => Some(GateStatus::Pass),
            "FAIL" => Some(GateStatus::Fail),
            "REVIEW_REQUIRED" => Some(GateStatus::ReviewRequired),
            "NOT_APPLICABLE" => Some(GateStatus::NotApplicable),
            "UNBOUND" => Some(GateStatus::Unbound),
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
            GateStatus::Unbound,
        ] {
            assert_eq!(GateStatus::parse(s.as_str()), Some(s));
        }
        assert_eq!(GateStatus::parse("pass"), None);
    }

    /// The four pre-existing wire strings are unchanged, so persisted
    /// and exported payloads from earlier releases keep parsing.
    #[test]
    fn existing_status_wire_strings_are_unchanged() {
        assert_eq!(GateStatus::Pass.as_str(), "PASS");
        assert_eq!(GateStatus::Fail.as_str(), "FAIL");
        assert_eq!(GateStatus::ReviewRequired.as_str(), "REVIEW_REQUIRED");
        assert_eq!(GateStatus::NotApplicable.as_str(), "NOT_APPLICABLE");
    }

    #[test]
    fn unbound_is_distinct_from_not_applicable() {
        assert_eq!(GateStatus::Unbound.as_str(), "UNBOUND");
        assert_ne!(GateStatus::Unbound, GateStatus::NotApplicable);
        assert_eq!(GateStatus::parse("UNBOUND"), Some(GateStatus::Unbound));
        assert_eq!(
            GateStatus::parse("NOT_APPLICABLE"),
            Some(GateStatus::NotApplicable)
        );
    }

    #[test]
    fn status_serialises_as_screaming_snake_case() {
        assert_eq!(
            serde_json::to_string(&GateStatus::Unbound).unwrap(),
            "\"UNBOUND\""
        );
        let back: GateStatus = serde_json::from_str("\"UNBOUND\"").unwrap();
        assert_eq!(back, GateStatus::Unbound);
    }

    #[test]
    fn severity_classifies_like_checker() {
        assert_eq!(GateSeverity::parse("error"), Some(GateSeverity::Error));
        assert_eq!(GateSeverity::parse("FATAL"), Some(GateSeverity::Error));
        assert_eq!(GateSeverity::parse("warn"), Some(GateSeverity::Warning));
        assert_eq!(GateSeverity::parse("nonsense"), None);
    }
}
