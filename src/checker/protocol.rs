//! Checker JSON protocol.
//!
//! A checker is expected to print a single JSON document to stdout
//! describing spec drift. The document is forward-compatible:
//!
//! * the only required top-level key is `alerts` (missing `alerts`
//!   is a protocol error, not success);
//! * each alert requires `severity`, `message`, `source`, and `symbol`
//!   at the normalized boundary;
//! * unknown top-level fields are ignored so newer checkers can add
//!   fields without breaking older binaries;
//! * unknown per-alert fields are tolerated and preserved in
//!   [`DriftAlert::extra`].
//!
//! The parser never panics on bad input. Malformed JSON or missing
//! required fields produce a [`ProtocolError`] so the caller can
//! persist a failed snapshot instead of dropping the invocation.

use std::collections::BTreeMap;

use serde::Deserialize;

/// Maximum alerts accepted in a single document.
pub const MAX_ALERTS: usize = 10_000;
/// Maximum bytes accepted for a single alert `message`.
pub const MAX_MESSAGE_BYTES: usize = 64 * 1024;

/// Top-level alerts document. Mirrors what a checker writes to stdout.
/// `alerts` is `Option` so a missing key maps to
/// [`ProtocolError::MissingAlerts`] instead of silent success.
#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
pub struct AlertsDocument {
    pub alerts: Option<Vec<DriftAlert>>,
}

impl AlertsDocument {
    /// Alert list; empty when the document carried `[]`.
    /// Callers should only use this after successful parsing, where
    /// `alerts` is guaranteed `Some`.
    pub fn alerts_list(&self) -> &[DriftAlert] {
        self.alerts.as_deref().unwrap_or(&[])
    }
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
    #[error(
        "checker output is missing required field `\"alerts\"` (got an object without `alerts`)"
    )]
    MissingAlerts,
    #[error("alert #{index} is missing required field `{field}`")]
    MissingField { index: usize, field: &'static str },
    #[error("checker reported {count} alerts, exceeding the limit of {max}")]
    TooManyAlerts { count: usize, max: usize },
    #[error("alert #{index} message is {bytes} bytes, exceeding the limit of {max} bytes")]
    AlertTooLarge {
        index: usize,
        bytes: usize,
        max: usize,
    },
}

/// Parse a checker's stdout. The bytes are expected to be UTF-8 JSON; a
/// lossy decode is applied as a fallback so the diagnostic returned to
/// the user is at least readable.
///
/// Forward-compatibility: unknown top-level fields are ignored. A
/// missing `alerts` key is [`ProtocolError::MissingAlerts`] (an empty
/// object `{}` is never success). Alert count and per-message size are
/// capped at [`MAX_ALERTS`] / [`MAX_MESSAGE_BYTES`].
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
    // Unknown top-level fields are ignored for forward compatibility;
    // only `alerts` is read. A missing key is an explicit error.
    if !obj.contains_key("alerts") {
        return Err(ProtocolError::MissingAlerts);
    }
    let doc: AlertsDocument = serde_json::from_value(value)
        .map_err(|e| ProtocolError::Json(format!("alerts shape: {e}")))?;
    let alerts = doc.alerts.as_deref().unwrap_or(&[]);
    if alerts.len() > MAX_ALERTS {
        return Err(ProtocolError::TooManyAlerts {
            count: alerts.len(),
            max: MAX_ALERTS,
        });
    }
    // Validate each alert's required fields. `serde` already extracted
    // `severity` / `message` / `source` / `symbol`, but we still need
    // to reject missing or empty values — a checker that wrote `""`
    // or omitted the field is not providing a usable alert.
    for (i, alert) in alerts.iter().enumerate() {
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
        if let Some(m) = alert.message.as_deref() {
            if m.len() > MAX_MESSAGE_BYTES {
                return Err(ProtocolError::AlertTooLarge {
                    index: i,
                    bytes: m.len(),
                    max: MAX_MESSAGE_BYTES,
                });
            }
        }
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
        assert_eq!(doc.alerts_list().len(), 1);
        assert_eq!(doc.alerts_list()[0].severity.as_deref(), Some("warning"));
        assert_eq!(doc.alerts_list()[0].symbol.as_deref(), Some("DbPool"));
    }

    #[test]
    fn parses_empty_alerts_array() {
        let doc = parse_alerts_document(br#"{"alerts": []}"#).unwrap();
        assert!(doc.alerts_list().is_empty());
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
        assert_eq!(doc.alerts_list().len(), 1);
        assert!(doc.alerts_list()[0].extra.contains_key("line"));
        assert!(doc.alerts_list()[0].extra.contains_key("code"));
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
    fn ignores_unknown_top_level_field() {
        let doc = parse_alerts_document(br#"{"alerts":[],"extra":"nope","newField":1}"#).unwrap();
        assert!(doc.alerts_list().is_empty());
    }

    #[test]
    fn allows_alerts_only_object() {
        let doc = parse_alerts_document(br#"{"alerts":[]}"#).unwrap();
        assert!(doc.alerts_list().is_empty());
    }

    #[test]
    fn rejects_missing_alerts_key() {
        let err = parse_alerts_document(br#"{}"#).unwrap_err();
        assert!(matches!(err, ProtocolError::MissingAlerts));
        assert!(err.to_string().contains("missing"));
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
