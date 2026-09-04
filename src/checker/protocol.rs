//! Checker JSON protocol.
//!
//! A checker is expected to print a single JSON document to stdout
//! describing spec drift. The document is forward-compatible:
//!
//! * the only required top-level key is `alerts`;
//! * each alert requires `severity`, `message`, `source`, and `symbol`
//!   at the normalized boundary;
//! * unknown fields are tolerated and preserved in [`DriftAlert::extra`]
//!   so the on-disk shape can grow without breaking older binaries.
//!
//! The parser never panics on bad input. Malformed JSON or missing
//! required fields produce a [`ProtocolError`] so the caller can
//! persist a failed snapshot instead of dropping the invocation.

use std::collections::BTreeMap;

use serde::Deserialize;

/// Top-level alerts document. Mirrors what a checker writes to stdout.
#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
pub struct AlertsDocument {
    #[serde(default)]
    pub alerts: Vec<DriftAlert>,
}

/// A single normalized drift alert at the internal boundary.
///
/// The required fields are typed as `Option<String>` so the parser can
/// surface a clean [`ProtocolError::MissingField`] for absent or empty
/// values rather than a generic JSON deserialization error. `extra`
/// preserves unknown JSON fields by their original key. This keeps
/// the wire format extensible: future versions can add fields
/// (e.g. `line`, `code`, `category`, `metadata`) without breaking
/// older binaries.
#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
pub struct DriftAlert {
    #[serde(default)]
    pub severity: Option<String>,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub symbol: Option<String>,
    #[serde(default, flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

/// Errors that [`parse_alerts_document`] can return.
#[derive(Debug, thiserror::Error, PartialEq)]
pub enum ProtocolError {
    #[error("checker output is not valid JSON: {0}")]
    Json(String),
    #[error("checker output is not a JSON object")]
    NotObject,
    #[error("alert #{index} is missing required field `{field}`")]
    MissingField { index: usize, field: &'static str },
}

/// Parse a checker's stdout. The bytes are expected to be UTF-8 JSON; a
/// lossy decode is applied as a fallback so the diagnostic returned to
/// the user is at least readable.
pub fn parse_alerts_document(bytes: &[u8]) -> Result<AlertsDocument, ProtocolError> {
    // First try strict UTF-8; if that fails, lossy-decode and retry so a
    // checker that wrote a stray Latin-1 byte still produces a meaningful
    // error rather than a confusing JSON parse failure.
    let text = match std::str::from_utf8(bytes) {
        Ok(s) => s.to_string(),
        Err(_) => String::from_utf8_lossy(bytes).into_owned(),
    };
    let value: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| ProtocolError::Json(e.to_string()))?;
    let obj = value.as_object().ok_or(ProtocolError::NotObject)?;
    // Reject extra top-level keys that are not `alerts`: this keeps the
    // wire format honest. We tolerate any value for the `alerts` key.
    for key in obj.keys() {
        if key != "alerts" {
            return Err(ProtocolError::Json(format!(
                "unknown top-level field `{key}`"
            )));
        }
    }
    let doc: AlertsDocument = serde_json::from_value(value)
        .map_err(|e| ProtocolError::Json(format!("alerts shape: {e}")))?;
    // Validate each alert's required fields. `serde` already extracted
    // `severity` / `message` / `source` / `symbol`, but we still need
    // to reject missing or empty values — a checker that wrote `""`
    // or omitted the field is not providing a usable alert.
    for (i, alert) in doc.alerts.iter().enumerate() {
        let check = |field: &'static str, value: &Option<String>| -> Result<(), ProtocolError> {
            match value {
                None => Err(ProtocolError::MissingField { index: i, field }),
                Some(s) if s.is_empty() => Err(ProtocolError::MissingField { index: i, field }),
                Some(_) => Ok(()),
            }
        };
        check("severity", &alert.severity)?;
        check("message", &alert.message)?;
        check("source", &alert.source)?;
        check("symbol", &alert.symbol)?;
    }
    Ok(doc)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_valid_alerts_document() {
        let doc = parse_alerts_document(
            br#"{
                "alerts": [
                    {
                        "severity": "warning",
                        "message": "x",
                        "source": "specs/db.md",
                        "symbol": "DbPool"
                    }
                ]
            }"#,
        )
        .unwrap();
        assert_eq!(doc.alerts.len(), 1);
        assert_eq!(doc.alerts[0].severity.as_deref(), Some("warning"));
        assert_eq!(doc.alerts[0].symbol.as_deref(), Some("DbPool"));
    }

    #[test]
    fn parses_empty_alerts_array() {
        let doc = parse_alerts_document(br#"{"alerts": []}"#).unwrap();
        assert!(doc.alerts.is_empty());
    }

    #[test]
    fn tolerates_unknown_alert_fields() {
        let doc = parse_alerts_document(
            br#"{
                "alerts": [
                    {
                        "severity": "warning",
                        "message": "x",
                        "source": "s",
                        "symbol": "S",
                        "line": 42,
                        "code": "DB001",
                        "category": "drift"
                    }
                ]
            }"#,
        )
        .unwrap();
        assert_eq!(doc.alerts.len(), 1);
        assert!(doc.alerts[0].extra.contains_key("line"));
        assert!(doc.alerts[0].extra.contains_key("code"));
    }

    #[test]
    fn rejects_missing_severity() {
        let err =
            parse_alerts_document(br#"{"alerts":[{"message":"x","source":"s","symbol":"S"}]}"#)
                .unwrap_err();
        assert!(matches!(
            err,
            ProtocolError::MissingField {
                field: "severity",
                ..
            }
        ));
    }

    #[test]
    fn rejects_missing_message() {
        let err =
            parse_alerts_document(br#"{"alerts":[{"severity":"w","source":"s","symbol":"S"}]}"#)
                .unwrap_err();
        assert!(matches!(
            err,
            ProtocolError::MissingField {
                field: "message",
                ..
            }
        ));
    }

    #[test]
    fn rejects_missing_source() {
        let err =
            parse_alerts_document(br#"{"alerts":[{"severity":"w","message":"x","symbol":"S"}]}"#)
                .unwrap_err();
        assert!(matches!(
            err,
            ProtocolError::MissingField {
                field: "source",
                ..
            }
        ));
    }

    #[test]
    fn rejects_missing_symbol() {
        let err =
            parse_alerts_document(br#"{"alerts":[{"severity":"w","message":"x","source":"s"}]}"#)
                .unwrap_err();
        assert!(matches!(
            err,
            ProtocolError::MissingField {
                field: "symbol",
                ..
            }
        ));
    }

    #[test]
    fn rejects_empty_required_string() {
        let err = parse_alerts_document(
            br#"{"alerts":[{"severity":"","message":"x","source":"s","symbol":"S"}]}"#,
        )
        .unwrap_err();
        assert!(matches!(
            err,
            ProtocolError::MissingField {
                field: "severity",
                ..
            }
        ));
    }

    #[test]
    fn rejects_malformed_json() {
        let err = parse_alerts_document(br#"{"alerts": [{"severity": "#).unwrap_err();
        assert!(matches!(err, ProtocolError::Json(_)));
    }

    #[test]
    fn rejects_non_object_root() {
        let err = parse_alerts_document(br#"[1,2,3]"#).unwrap_err();
        assert!(matches!(err, ProtocolError::NotObject));
    }

    #[test]
    fn rejects_unknown_top_level_field() {
        let err = parse_alerts_document(br#"{"alerts":[],"extra":"nope"}"#).unwrap_err();
        assert!(matches!(err, ProtocolError::Json(_)));
    }

    #[test]
    fn allows_alerts_only_object() {
        let doc = parse_alerts_document(br#"{"alerts":[]}"#).unwrap();
        assert!(doc.alerts.is_empty());
    }

    #[test]
    fn handles_lossy_utf8() {
        // 0xFF is invalid UTF-8; the parser should still report a
        // JSON error (the lossy decode is applied first), not panic.
        let bytes: Vec<u8> = vec![
            b'{', b'"', b'a', b'l', b'e', b'r', b't', b's', b'"', 0xFF, 0xFE,
        ];
        let err = parse_alerts_document(&bytes).unwrap_err();
        // Either the lossy decode lands on a JSON error or the result
        // is a parse error. Either way, the parser must not panic.
        assert!(matches!(err, ProtocolError::Json(_)));
    }
}
