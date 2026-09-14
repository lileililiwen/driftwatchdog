//! Bounded, traceable gate evidence.
//!
//! A gate result never embeds raw unbounded output. Instead, producers
//! store artifact bytes under `.driftwatch/artifacts/` and results
//! reference them by [`ArtifactRecord::key`] (surfaced on the wire as
//! [`crate::gate::types::EvidenceRef`]). Each record carries kind,
//! producer identity, size, a `sha256:` digest, redaction status, and a
//! capped, secret-redacted preview.
//!
//! Invariants:
//!
//! * artifact files never leave the project state directory —
//!   adapter-supplied paths that escape it are rejected with a safe
//!   diagnostic ([`EvidenceError::PathEscape`]);
//! * previews are redacted (recognized patterns plus configured
//!   sensitive values) and truncated at a UTF-8 char boundary;
//! * cleanup removes bulky files but keeps the metadata row, so a
//!   reference stays auditable with an unavailable-content marker;
//! * a result whose evidence is missing or unavailable must not claim
//!   evidence-backed `PASS` (see [`evidence_backed_pass`]).
//!
//! No OpenSpec types appear here. No tool is installed, no network call
//! is made, and no LLM is invoked.

use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::gate::redact::bound_text_with_extra;
use crate::gate::types::{GateResult, GateStatus};
use crate::util::truncate_char_boundary;

/// Directory under the state root that holds artifact files.
pub const ARTIFACT_DIR_NAME: &str = "artifacts";
/// Largest single artifact accepted for persistence (1 MiB).
pub const MAX_ARTIFACT_BYTES: usize = 1024 * 1024;
/// Largest persisted preview per artifact.
pub const MAX_PREVIEW_BYTES: usize = 1024;
/// Largest artifact key accepted (matches the DTO evidence-key bound).
pub const MAX_KEY_BYTES: usize = 256;
/// Marker served when artifact content is gone or was never stored.
pub const UNAVAILABLE_PREVIEW: &str = "[evidence unavailable]";

/// Artifact classes from the design boundary: command output, files,
/// JSON/SARIF tool output, screenshots, traces, and provider responses.
/// Wire strings are `snake_case` and stable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactKind {
    CommandOutput,
    ToolJson,
    Sarif,
    Screenshot,
    Trace,
    ProviderResponse,
    File,
}

impl ArtifactKind {
    pub fn as_str(self) -> &'static str {
        match self {
            ArtifactKind::CommandOutput => "command_output",
            ArtifactKind::ToolJson => "tool_json",
            ArtifactKind::Sarif => "sarif",
            ArtifactKind::Screenshot => "screenshot",
            ArtifactKind::Trace => "trace",
            ArtifactKind::ProviderResponse => "provider_response",
            ArtifactKind::File => "file",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "command_output" => Some(ArtifactKind::CommandOutput),
            "tool_json" => Some(ArtifactKind::ToolJson),
            "sarif" => Some(ArtifactKind::Sarif),
            "screenshot" => Some(ArtifactKind::Screenshot),
            "trace" => Some(ArtifactKind::Trace),
            "provider_response" => Some(ArtifactKind::ProviderResponse),
            "file" => Some(ArtifactKind::File),
            _ => None,
        }
    }

    fn extension(self) -> &'static str {
        match self {
            ArtifactKind::CommandOutput | ArtifactKind::Trace | ArtifactKind::File => "log",
            ArtifactKind::ToolJson | ArtifactKind::ProviderResponse => "json",
            ArtifactKind::Sarif => "sarif",
            ArtifactKind::Screenshot => "bin",
        }
    }
}

/// Fields needed to persist one artifact. `sensitive` carries
/// caller-configured secret values (e.g. tokens from the project
/// environment) redacted in addition to the recognized patterns.
/// `created_at` defaults to now (RFC 3339) when `None`.
#[derive(Debug, Clone)]
pub struct NewArtifact<'a> {
    pub key: &'a str,
    pub kind: ArtifactKind,
    pub producer: &'a str,
    pub producer_version: Option<&'a str>,
    pub media_type: Option<&'a str>,
    pub sensitive: Vec<&'a str>,
    pub created_at: Option<&'a str>,
}

/// One persisted artifact: metadata row plus (while retained) a file
/// under `.driftwatch/artifacts/`. `rel_path` is relative to the state
/// root; `None` means content was never stored (unavailable marker).
/// `preview` is always redacted and bounded; `None` means unavailable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactRecord {
    pub key: String,
    pub kind: ArtifactKind,
    pub producer: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub producer_version: Option<String>,
    pub created_at: String,
    pub byte_size: u64,
    pub digest: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub media_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rel_path: Option<String>,
    pub redacted: bool,
    pub available: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preview: Option<String>,
}

impl ArtifactRecord {
    /// Marker row for evidence that cannot be safely stored (redaction
    /// failure, oversize content, unreadable input). The reference stays
    /// auditable; content is explicitly unavailable.
    pub fn unavailable(key: &str, kind: ArtifactKind, producer: &str, created_at: &str) -> Self {
        Self {
            key: truncate_char_boundary(key, MAX_KEY_BYTES),
            kind,
            producer: producer.to_string(),
            producer_version: None,
            created_at: created_at.to_string(),
            byte_size: 0,
            digest: "sha256:unavailable".to_string(),
            media_type: None,
            rel_path: None,
            redacted: true,
            available: false,
            preview: None,
        }
    }

    /// Safe preview for consumers: the redacted bounded preview while
    /// retained, otherwise the unavailable marker. Never raw bytes.
    pub fn safe_preview(&self) -> &str {
        if self.available {
            self.preview.as_deref().unwrap_or(UNAVAILABLE_PREVIEW)
        } else {
            UNAVAILABLE_PREVIEW
        }
    }
}

/// Failure modes for evidence persistence. Messages are safe
/// diagnostics: they carry the artifact key and the policy, never raw
/// content.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum EvidenceError {
    #[error("artifact key must be non-empty and at most {max} bytes")]
    BadKey { max: usize },
    #[error("artifact is {bytes} bytes, exceeding the limit of {max} bytes")]
    TooLarge { bytes: usize, max: usize },
    #[error("artifact path for key '{key}' escapes the state directory; keep artifacts under `.driftwatch/artifacts/`")]
    PathEscape { key: String },
    #[error("io error persisting artifact for key '{key}': {message}")]
    Io { key: String, message: String },
}

/// `sha256:` digest of raw bytes as lowercase hex.
pub fn digest_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let sum = hasher.finalize();
    let mut out = String::with_capacity(7 + 64);
    out.push_str("sha256:");
    for b in sum {
        out.push(HEX[(b >> 4) as usize]);
        out.push(HEX[(b & 0x0f) as usize]);
    }
    out
}

const HEX: [char; 16] = [
    '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'a', 'b', 'c', 'd', 'e', 'f',
];

/// Validate an artifact key: non-empty within [`MAX_KEY_BYTES`].
pub fn validate_key(key: &str) -> Result<(), EvidenceError> {
    if key.is_empty() || key.len() > MAX_KEY_BYTES {
        return Err(EvidenceError::BadKey { max: MAX_KEY_BYTES });
    }
    Ok(())
}

/// Derive a confined file name for `key`: only ASCII alphanumerics,
/// `-`, `_`, `.` survive; everything else becomes `_`. Returns `None`
/// when nothing usable remains (caller rejects the key).
fn safe_file_stem(key: &str) -> Option<String> {
    let stem: String = key
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let trimmed = stem.trim_matches(|c| c == '.' || c == '_');
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

/// Resolve where `key` would be stored under `state_dir`, verifying the
/// result stays inside the state root. The digest fragment keeps
/// colliding keys distinct across producers.
pub fn confined_path(
    state_dir: &Path,
    key: &str,
    kind: ArtifactKind,
    digest: &str,
) -> Result<PathBuf, EvidenceError> {
    let stem = safe_file_stem(key).ok_or(EvidenceError::BadKey { max: MAX_KEY_BYTES })?;
    let short = digest.rsplit(':').next().unwrap_or(digest);
    let short: String = short.chars().take(16).collect();
    let name = format!("{stem}-{short}.{}", kind.extension());
    let dir = state_dir.join(ARTIFACT_DIR_NAME);
    let candidate = dir.join(&name);
    // Belt and braces: the name is generated from a sanitized stem, but
    // verify confinement explicitly so a future refactor cannot regress
    // into a path escape.
    if candidate.parent() != Some(dir.as_path()) {
        return Err(EvidenceError::PathEscape {
            key: key.to_string(),
        });
    }
    Ok(candidate)
}

/// Validate an adapter-supplied artifact path: it must be relative, must
/// not escape the state root via `..`, and must resolve inside
/// `state_dir`. Returns the path relative to the state root using
/// forward slashes. Absolute paths, `..` escapes, and anything outside
/// the state directory are rejected with [`EvidenceError::PathEscape`].
pub fn confine_adapter_path(
    state_dir: &Path,
    key: &str,
    supplied: &Path,
) -> Result<String, EvidenceError> {
    let escape = || EvidenceError::PathEscape {
        key: key.to_string(),
    };
    if supplied.is_absolute() {
        return Err(escape());
    }
    let mut rel = PathBuf::new();
    for comp in supplied.components() {
        match comp {
            Component::Normal(part) => rel.push(part),
            // `CurDir` (`.`) is harmless and skipped; anything else
            // (`..`, prefixes, root) is an escape attempt.
            Component::CurDir => {}
            _ => return Err(escape()),
        }
    }
    if rel.as_os_str().is_empty() {
        return Err(escape());
    }
    // Lexical join is safe now: `rel` holds only normal components.
    let _joined = state_dir.join(&rel);
    Ok(rel.to_string_lossy().replace('\\', "/"))
}

/// Build the [`ArtifactRecord`] for `content` without touching the
/// filesystem: digest over raw bytes, redacted bounded preview from a
/// UTF-8-lossy decode, storage path confined to `state_dir`.
/// Content over [`MAX_ARTIFACT_BYTES`] is rejected before any write.
pub fn build_record(
    state_dir: &Path,
    new: &NewArtifact<'_>,
    content: &[u8],
    now_rfc3339: &str,
) -> Result<(ArtifactRecord, PathBuf), EvidenceError> {
    validate_key(new.key)?;
    if content.len() > MAX_ARTIFACT_BYTES {
        return Err(EvidenceError::TooLarge {
            bytes: content.len(),
            max: MAX_ARTIFACT_BYTES,
        });
    }
    let digest = digest_bytes(content);
    let path = confined_path(state_dir, new.key, new.kind, &digest)?;
    let created_at = new.created_at.unwrap_or(now_rfc3339).to_string();
    let text = String::from_utf8_lossy(content);
    let preview = bound_text_with_extra(&text, &new.sensitive, MAX_PREVIEW_BYTES);
    let rel = path
        .strip_prefix(state_dir)
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .unwrap_or_else(|_| path.to_string_lossy().into_owned());
    Ok((
        ArtifactRecord {
            key: new.key.to_string(),
            kind: new.kind,
            producer: truncate_char_boundary(new.producer, 256),
            producer_version: new.producer_version.map(|v| truncate_char_boundary(v, 128)),
            created_at,
            byte_size: content.len() as u64,
            digest,
            media_type: new.media_type.map(|v| truncate_char_boundary(v, 128)),
            rel_path: Some(rel),
            redacted: true,
            available: true,
            preview: Some(preview),
        },
        path,
    ))
}

/// Persist `content` as an artifact under `state_dir` and return its
/// record. Creates `.driftwatch/artifacts/` on demand; the write goes
/// through a temp file plus atomic rename so concurrent writers never
/// leave a half-written artifact behind.
pub fn store_bytes(
    state_dir: &Path,
    new: &NewArtifact<'_>,
    content: &[u8],
) -> Result<ArtifactRecord, EvidenceError> {
    let now = chrono::Utc::now().to_rfc3339();
    let (record, path) = build_record(state_dir, new, content, &now)?;
    let dir = state_dir.join(ARTIFACT_DIR_NAME);
    std::fs::create_dir_all(&dir).map_err(|e| EvidenceError::Io {
        key: new.key.to_string(),
        message: e.to_string(),
    })?;
    // Temp file in the same directory so `rename` is atomic.
    let tmp = dir.join(format!(
        ".tmp-{}-{}",
        std::process::id(),
        short_digest(&record.digest)
    ));
    std::fs::write(&tmp, content).map_err(|e| EvidenceError::Io {
        key: new.key.to_string(),
        message: e.to_string(),
    })?;
    if let Err(e) = std::fs::rename(&tmp, &path) {
        let _ = std::fs::remove_file(&tmp);
        return Err(EvidenceError::Io {
            key: new.key.to_string(),
            message: e.to_string(),
        });
    }
    Ok(record)
}

fn short_digest(digest: &str) -> String {
    digest
        .rsplit(':')
        .next()
        .unwrap_or(digest)
        .chars()
        .take(12)
        .collect()
}

/// Evidence-backed `PASS`: true only when the result claims `PASS`,
/// cites at least one evidence reference, and every cited key resolves
/// to an available artifact whose digest matches when the reference
/// carries one. Missing or unavailable evidence — including the
/// redaction-failure markers — never yields `true`, so such a result
/// must not be presented as evidence-backed `PASS`.
pub fn evidence_backed_pass(
    result: &GateResult,
    resolve: &dyn Fn(&str) -> Option<ArtifactRecord>,
) -> bool {
    if result.status != GateStatus::Pass || result.evidence.is_empty() {
        return false;
    }
    result.evidence.iter().all(|e| match resolve(&e.key) {
        Some(rec) => {
            if !rec.available {
                return false;
            }
            match &e.digest {
                Some(want) => rec.digest == *want,
                None => true,
            }
        }
        None => false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gate::types::{EvidenceRef, GateSeverity};

    fn new_artifact<'a>(key: &'a str) -> NewArtifact<'a> {
        NewArtifact {
            key,
            kind: ArtifactKind::CommandOutput,
            producer: "probe",
            producer_version: Some("1.0"),
            media_type: None,
            sensitive: vec![],
            created_at: Some("2026-09-14T00:00:00Z"),
        }
    }

    fn pass_result(evidence_keys: &[&str]) -> GateResult {
        GateResult {
            gate_id: "g".into(),
            source: "s".into(),
            status: GateStatus::Pass,
            severity: GateSeverity::Info,
            findings: vec![],
            evidence: evidence_keys
                .iter()
                .map(|k| EvidenceRef {
                    key: k.to_string(),
                    digest: None,
                    media_type: None,
                    byte_size: None,
                    preview: None,
                })
                .collect(),
            missing_evidence: vec![],
            diagnostic: None,
            remediation: None,
        }
    }

    #[test]
    fn kind_round_trips() {
        for k in [
            ArtifactKind::CommandOutput,
            ArtifactKind::ToolJson,
            ArtifactKind::Sarif,
            ArtifactKind::Screenshot,
            ArtifactKind::Trace,
            ArtifactKind::ProviderResponse,
            ArtifactKind::File,
        ] {
            assert_eq!(ArtifactKind::parse(k.as_str()), Some(k));
        }
        assert_eq!(ArtifactKind::parse("nope"), None);
    }

    #[test]
    fn digest_is_prefixed_hex() {
        let d = digest_bytes(b"hello");
        assert!(d.starts_with("sha256:"));
        assert_eq!(d.len(), 7 + 64);
        assert_eq!(d, digest_bytes(b"hello"));
        assert_ne!(d, digest_bytes(b"other"));
    }

    #[test]
    fn validate_key_rejects_empty_and_oversize() {
        assert!(validate_key("k").is_ok());
        assert!(matches!(
            validate_key(""),
            Err(EvidenceError::BadKey { .. })
        ));
        assert!(matches!(
            validate_key(&"k".repeat(MAX_KEY_BYTES + 1)),
            Err(EvidenceError::BadKey { .. })
        ));
    }

    #[test]
    fn confined_path_stays_inside_state_dir() {
        let root = Path::new("/tmp/proj/.driftwatch");
        let p = confined_path(
            root,
            "check/output log",
            ArtifactKind::CommandOutput,
            "sha256:abc",
        )
        .unwrap();
        assert!(p.starts_with(root.join(ARTIFACT_DIR_NAME)));
        // Hostile keys collapse to something safe, never `..`.
        let evil =
            confined_path(root, "../../etc/passwd", ArtifactKind::File, "sha256:abc").unwrap();
        assert!(evil.starts_with(root.join(ARTIFACT_DIR_NAME)));
        assert!(!evil.to_string_lossy().contains(".."));
    }

    #[test]
    fn adapter_path_rejects_escapes_and_absolute() {
        let root = Path::new("/tmp/proj/.driftwatch");
        assert!(confine_adapter_path(root, "k", Path::new("/etc/passwd")).is_err());
        assert!(confine_adapter_path(root, "k", Path::new("../outside.bin")).is_err());
        assert!(confine_adapter_path(root, "k", Path::new("a/../../b")).is_err());
        let ok = confine_adapter_path(root, "k", Path::new("artifacts/a.json")).unwrap();
        assert_eq!(ok, "artifacts/a.json");
        // `.` components are harmless.
        let dot = confine_adapter_path(root, "k", Path::new("./artifacts/a.json")).unwrap();
        assert_eq!(dot, "artifacts/a.json");
    }

    #[test]
    fn build_record_redacts_configured_token() {
        let root = Path::new("/tmp/proj/.driftwatch");
        let mut new = new_artifact("run/output");
        new.sensitive = vec!["tok-SECRET-123"];
        let (rec, path) = build_record(
            root,
            &new,
            b"stderr: tok-SECRET-123 failed",
            "2026-09-14T00:00:00Z",
        )
        .unwrap();
        assert!(path.starts_with(root));
        let preview = rec.preview.unwrap();
        assert!(!preview.contains("tok-SECRET-123"));
        assert!(preview.contains("[REDACTED]"));
        assert!(rec.digest.starts_with("sha256:"));
        assert!(rec.redacted && rec.available);
    }

    #[test]
    fn build_record_rejects_oversize_before_write() {
        let root = Path::new("/tmp/proj/.driftwatch");
        let big = vec![b'x'; MAX_ARTIFACT_BYTES + 1];
        let err = build_record(root, &new_artifact("k"), &big, "2026-09-14T00:00:00Z").unwrap_err();
        assert!(matches!(err, EvidenceError::TooLarge { .. }));
    }

    #[test]
    fn store_bytes_round_trips_through_state_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let state = tmp.path().join(".driftwatch");
        let rec = store_bytes(&state, &new_artifact("gate/out"), b"hello evidence").unwrap();
        let rel = rec.rel_path.clone().unwrap();
        let on_disk = std::fs::read(state.join(&rel)).unwrap();
        assert_eq!(on_disk, b"hello evidence");
        assert_eq!(rec.digest, digest_bytes(b"hello evidence"));
    }

    #[test]
    fn unavailable_marker_never_claims_pass() {
        let rec = ArtifactRecord::unavailable(
            "k",
            ArtifactKind::Sarif,
            "semgrep",
            "2026-09-14T00:00:00Z",
        );
        assert!(!rec.available);
        assert_eq!(rec.safe_preview(), UNAVAILABLE_PREVIEW);
        let result = pass_result(&["k"]);
        let resolve = |key: &str| {
            if key == "k" {
                Some(rec.clone())
            } else {
                None
            }
        };
        assert!(!evidence_backed_pass(&result, &resolve));
    }

    #[test]
    fn evidence_backed_pass_requires_all_refs_available() {
        let mk = |key: &str| ArtifactRecord {
            key: key.into(),
            kind: ArtifactKind::CommandOutput,
            producer: "p".into(),
            producer_version: None,
            created_at: "2026-09-14T00:00:00Z".into(),
            byte_size: 3,
            digest: "sha256:abc".into(),
            media_type: None,
            rel_path: Some("artifacts/a.log".into()),
            redacted: true,
            available: true,
            preview: Some("ok".into()),
        };
        let good = mk("a");
        let resolve = |key: &str| if key == "a" { Some(good.clone()) } else { None };
        assert!(evidence_backed_pass(&pass_result(&["a"]), &resolve));
        // Missing key.
        assert!(!evidence_backed_pass(&pass_result(&["a", "b"]), &resolve));
        // No evidence cited at all.
        assert!(!evidence_backed_pass(&pass_result(&[]), &resolve));
        // Non-PASS status.
        let mut fail = pass_result(&["a"]);
        fail.status = GateStatus::Fail;
        assert!(!evidence_backed_pass(&fail, &resolve));
        // Digest mismatch.
        let mut stale = pass_result(&["a"]);
        stale.evidence[0].digest = Some("sha256:stale".into());
        assert!(!evidence_backed_pass(&stale, &resolve));
        // Matching digest passes.
        let mut fresh = pass_result(&["a"]);
        fresh.evidence[0].digest = Some("sha256:abc".into());
        assert!(evidence_backed_pass(&fresh, &resolve));
    }
}
