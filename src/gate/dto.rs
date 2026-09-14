//! Versioned JSON boundary for gate results.
//!
//! Wire shape:
//!
//! ```json
//! {"version": 1, "results": [<GateResult>, ...]}
//! ```
//!
//! Compatibility rules:
//!
//! * `version` is required and must equal [`GATE_CONTRACT_VERSION`];
//!   unknown versions are rejected (callers decide how to surface them,
//!   typically as `REVIEW_REQUIRED` with missing evidence).
//! * documents larger than [`MAX_RESULT_BYTES`] or with more than
//!   [`MAX_RESULTS_PER_DOC`] results are rejected before allocation-heavy
//!   work;
//! * over-long strings are truncated at a char boundary, over-long
//!   arrays are capped deterministically (first N kept), and every
//!   diagnostic/preview/remediation passes through secret redaction.

use crate::gate::redact::bound_text;
use crate::gate::types::{EvidenceRef, Finding, GateResult, GateSeverity, GateStatus};
use serde::{Deserialize, Serialize};

pub const GATE_CONTRACT_VERSION: u32 = 1;
pub const MAX_RESULT_BYTES: usize = 256 * 1024;
pub const MAX_RESULTS_PER_DOC: usize = 256;
pub const MAX_FINDINGS: usize = 256;
pub const MAX_EVIDENCE_REFS: usize = 64;
pub const MAX_MISSING_EVIDENCE: usize = 64;
pub const MAX_DIAGNOSTIC_BYTES: usize = 2048;
pub const MAX_REMEDIATION_BYTES: usize = 2048;
pub const MAX_GATE_ID_BYTES: usize = 128;
pub const MAX_SOURCE_BYTES: usize = 256;
pub const MAX_TITLE_BYTES: usize = 1024;
pub const MAX_LOCATION_BYTES: usize = 512;
pub const MAX_EVIDENCE_KEY_BYTES: usize = 256;
pub const MAX_PREVIEW_BYTES: usize = 1024;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum DtoError {
    #[error("document is not valid JSON: {0}")]
    Json(String),
    #[error("document is not a JSON object")]
    NotObject,
    #[error("document is missing required field `version`")]
    MissingVersion,
    #[error("unsupported gate contract version {got}, expected {expected}")]
    UnknownVersion { got: u64, expected: u32 },
    #[error("document is missing required field `results`")]
    MissingResults,
    #[error("document is {bytes} bytes, exceeding the limit of {max} bytes")]
    TooLarge { bytes: usize, max: usize },
    #[error("document has {count} results, exceeding the limit of {max}")]
    TooManyResults { count: usize, max: usize },
    #[error("result #{index} is missing required field `{field}`")]
    MissingField { index: usize, field: &'static str },
    #[error("result #{index} has invalid status `{value}`")]
    InvalidStatus { index: usize, value: String },
    #[error("result #{index} has invalid severity `{value}`")]
    InvalidSeverity { index: usize, value: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct WireDocument {
    version: Option<u64>,
    results: Option<Vec<WireResult>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct WireResult {
    gate_id: Option<String>,
    source: Option<String>,
    status: Option<String>,
    severity: Option<String>,
    #[serde(default)]
    findings: Vec<WireFinding>,
    #[serde(default)]
    evidence: Vec<WireEvidence>,
    #[serde(default)]
    missing_evidence: Vec<String>,
    #[serde(default)]
    diagnostic: Option<String>,
    #[serde(default)]
    remediation: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct WireFinding {
    title: Option<String>,
    severity: Option<String>,
    #[serde(default)]
    location: Option<String>,
    #[serde(default)]
    rule: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct WireEvidence {
    key: Option<String>,
    #[serde(default)]
    digest: Option<String>,
    #[serde(default)]
    media_type: Option<String>,
    #[serde(default)]
    byte_size: Option<u64>,
    #[serde(default)]
    preview: Option<String>,
}

/// Parse and normalize one versioned gate-result document.
pub fn parse_gate_result_document(bytes: &[u8]) -> Result<Vec<GateResult>, DtoError> {
    if bytes.len() > MAX_RESULT_BYTES {
        return Err(DtoError::TooLarge {
            bytes: bytes.len(),
            max: MAX_RESULT_BYTES,
        });
    }
    let text = match std::str::from_utf8(bytes) {
        Ok(s) => s.to_string(),
        Err(_) => String::from_utf8_lossy(bytes).into_owned(),
    };
    let value: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| DtoError::Json(e.to_string()))?;
    let obj = value.as_object().ok_or(DtoError::NotObject)?;
    if !obj.contains_key("version") {
        return Err(DtoError::MissingVersion);
    }
    if !obj.contains_key("results") {
        return Err(DtoError::MissingResults);
    }
    let doc: WireDocument =
        serde_json::from_value(value).map_err(|e| DtoError::Json(format!("shape: {e}")))?;
    let version = doc.version.ok_or(DtoError::MissingVersion)?;
    if version != u64::from(GATE_CONTRACT_VERSION) {
        return Err(DtoError::UnknownVersion {
            got: version,
            expected: GATE_CONTRACT_VERSION,
        });
    }
    let wire_results = doc.results.ok_or(DtoError::MissingResults)?;
    if wire_results.len() > MAX_RESULTS_PER_DOC {
        return Err(DtoError::TooManyResults {
            count: wire_results.len(),
            max: MAX_RESULTS_PER_DOC,
        });
    }
    let mut out = Vec::with_capacity(wire_results.len());
    for (index, w) in wire_results.iter().enumerate() {
        out.push(normalize_result(index, w)?);
    }
    Ok(out)
}

fn required(index: usize, field: &'static str, v: &Option<String>) -> Result<String, DtoError> {
    match v {
        Some(s) if !s.is_empty() => Ok(s.clone()),
        _ => Err(DtoError::MissingField { index, field }),
    }
}

fn normalize_result(index: usize, w: &WireResult) -> Result<GateResult, DtoError> {
    let gate_id_raw = required(index, "gate_id", &w.gate_id)?;
    let source_raw = required(index, "source", &w.source)?;
    let status_raw = required(index, "status", &w.status)?;
    let severity_raw = required(index, "severity", &w.severity)?;
    let status = GateStatus::parse(&status_raw).ok_or(DtoError::InvalidStatus {
        index,
        value: status_raw,
    })?;
    let severity = GateSeverity::parse(&severity_raw).ok_or(DtoError::InvalidSeverity {
        index,
        value: severity_raw,
    })?;

    let gate_id = bound_text(&gate_id_raw, MAX_GATE_ID_BYTES);
    let source = bound_text(&source_raw, MAX_SOURCE_BYTES);

    let mut findings = Vec::new();
    for f in w.findings.iter().take(MAX_FINDINGS) {
        let title_raw = match &f.title {
            Some(s) if !s.is_empty() => s.clone(),
            _ => continue,
        };
        let severity = match &f.severity {
            Some(s) => GateSeverity::parse(s).unwrap_or(GateSeverity::Warning),
            None => GateSeverity::Warning,
        };
        findings.push(Finding {
            title: bound_text(&title_raw, MAX_TITLE_BYTES),
            severity,
            location: f
                .location
                .as_ref()
                .map(|s| bound_text(s, MAX_LOCATION_BYTES)),
            rule: f.rule.as_ref().map(|s| bound_text(s, MAX_LOCATION_BYTES)),
        });
    }

    let mut evidence = Vec::new();
    for e in w.evidence.iter().take(MAX_EVIDENCE_REFS) {
        let key = match &e.key {
            Some(s) if !s.is_empty() => bound_text(s, MAX_EVIDENCE_KEY_BYTES),
            _ => continue,
        };
        evidence.push(EvidenceRef {
            key,
            digest: e.digest.as_ref().map(|s| bound_text(s, 128)),
            media_type: e.media_type.as_ref().map(|s| bound_text(s, 128)),
            byte_size: e.byte_size,
            preview: e.preview.as_ref().map(|s| bound_text(s, MAX_PREVIEW_BYTES)),
        });
    }

    let missing_evidence: Vec<String> = w
        .missing_evidence
        .iter()
        .take(MAX_MISSING_EVIDENCE)
        .map(|s| bound_text(s, MAX_EVIDENCE_KEY_BYTES))
        .collect();

    Ok(GateResult {
        gate_id,
        source,
        status,
        severity,
        findings,
        evidence,
        missing_evidence,
        diagnostic: w
            .diagnostic
            .as_ref()
            .map(|s| bound_text(s, MAX_DIAGNOSTIC_BYTES)),
        remediation: w
            .remediation
            .as_ref()
            .map(|s| bound_text(s, MAX_REMEDIATION_BYTES)),
    })
}

/// Serialize results to the canonical versioned document.
/// Output already passed through bounds/redaction at construction;
/// this function only wraps the version envelope deterministically.
pub fn to_canonical_json(results: &[GateResult]) -> String {
    #[derive(Serialize)]
    struct Doc<'a> {
        version: u32,
        results: &'a [GateResult],
    }
    serde_json::to_string(&Doc {
        version: GATE_CONTRACT_VERSION,
        results,
    })
    .expect("GateResult serializes")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc(json: &str) -> Result<Vec<GateResult>, DtoError> {
        parse_gate_result_document(json.as_bytes())
    }

    #[test]
    fn parses_success_result() {
        let out = doc(
            r#"{"version":1,"results":[{"gate_id":"g","source":"s","status":"PASS","severity":"info"}]}"#,
        )
        .unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].status, GateStatus::Pass);
    }

    #[test]
    fn rejects_unknown_version() {
        let err = doc(
            r#"{"version":99,"results":[{"gate_id":"g","source":"s","status":"PASS","severity":"info"}]}"#,
        )
        .unwrap_err();
        assert!(matches!(err, DtoError::UnknownVersion { .. }));
    }

    #[test]
    fn rejects_missing_version_and_results() {
        assert!(matches!(
            doc(r#"{"results":[]}"#).unwrap_err(),
            DtoError::MissingVersion
        ));
        assert!(matches!(
            doc(r#"{"version":1}"#).unwrap_err(),
            DtoError::MissingResults
        ));
    }

    #[test]
    fn rejects_malformed_and_non_object() {
        assert!(matches!(
            doc(r#"{"version": "#).unwrap_err(),
            DtoError::Json(_)
        ));
        assert!(matches!(doc(r#"[1,2]"#).unwrap_err(), DtoError::NotObject));
    }

    #[test]
    fn rejects_oversized_document() {
        let big = vec![b'x'; MAX_RESULT_BYTES + 1];
        assert!(matches!(
            parse_gate_result_document(&big).unwrap_err(),
            DtoError::TooLarge { .. }
        ));
    }

    #[test]
    fn secret_bearing_input_is_redacted() {
        let out = doc(
            r#"{"version":1,"results":[{"gate_id":"g","source":"s","status":"FAIL","severity":"error","diagnostic":"api_key=hunter2-secret"}]}"#,
        )
        .unwrap();
        let d = out[0].diagnostic.as_deref().unwrap();
        assert!(!d.contains("hunter2-secret"));
        assert!(d.contains("[REDACTED]"));
    }

    #[test]
    fn oversized_strings_truncate_at_char_boundary() {
        let big = "a".repeat(5000);
        let json = format!(
            r#"{{"version":1,"results":[{{"gate_id":"g","source":"s","status":"FAIL","severity":"error","diagnostic":"{big}"}}]}}"#
        );
        let out = doc(&json).unwrap();
        let d = out[0].diagnostic.as_deref().unwrap();
        assert!(d.len() <= MAX_DIAGNOSTIC_BYTES + 3);
    }

    #[test]
    fn canonical_json_round_trips() {
        let out = doc(
            r#"{"version":1,"results":[{"gate_id":"g","source":"s","status":"PASS","severity":"info"}]}"#,
        )
        .unwrap();
        let s = to_canonical_json(&out);
        let back = parse_gate_result_document(s.as_bytes()).unwrap();
        assert_eq!(back, out);
    }
}
