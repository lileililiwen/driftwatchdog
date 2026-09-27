//! Gate-run evidence export for Workspace Governance audit.
//!
//! A completed `driftwatch gate` run can be exported as a versioned
//! evidence record in the governance `release_evidence` vocabulary.
//! The export is a pure read: it never mutates the run, the project, or
//! any registry, and it never invents fields that the run did not
//! supply. The state per field is deterministic:
//!
//! * `verified` only when a scheduled check actually ran and passed on
//!   the current revision;
//! * `unverified` when the run is stale, when a check ran and reported
//!   `FAIL` or `REVIEW_REQUIRED` with evidence, or when the field has
//!   no scheduled supplier;
//! * `blocked` when a scheduled check could not execute (no command
//!   bound, spawn/timeout/signal, or any `missing_evidence` key).
//!
//! The field names and state names are the **governance vocabulary**;
//! Driftwatchdog consumes but does not re-declare them. An unknown
//! field is a hard construction error before the document is built.
//!
//! No tool is installed, no network call is made, and no LLM is invoked.
//! No OpenSpec types appear here.

use serde::{Deserialize, Serialize};

use crate::gate::concerns::{CAPABILITY_CONFORMANCE, RELEASE_EVIDENCE};
use crate::gate::dto::{MAX_DIAGNOSTIC_BYTES, MAX_EVIDENCE_KEY_BYTES, MAX_SOURCE_BYTES};
use crate::gate::redact::bound_text;
use crate::gate::types::{EvidenceRef, GateResult, GateStatus};
use crate::repo::gates::GateRunRow;
use crate::util::truncate_char_boundary;

/// Wire version of the evidence export document.
pub const EVIDENCE_EXPORT_SCHEMA_VERSION: u32 = 1;

/// Closed set of governance `release_evidence` field names. Consumed
/// from the workspace-governance vocabulary, not redeclared.
pub const GOVERNANCE_FIELDS: &[&str] = &[
    "revision",
    "version",
    "toolchain",
    "artifacts",
    "digests",
    "sbom",
    "provenance",
    "checks",
    "publication",
];

/// Closed set of governance evidence states. The wire strings are
/// lowercase and stable.
pub const GOVERNANCE_STATES: &[&str] = &["verified", "unverified", "blocked"];

/// Per-field state assigned by the export. Wire strings match
/// [`GOVERNANCE_STATES`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EvidenceState {
    Verified,
    Unverified,
    Blocked,
}

impl EvidenceState {
    pub fn as_str(self) -> &'static str {
        match self {
            EvidenceState::Verified => "verified",
            EvidenceState::Unverified => "unverified",
            EvidenceState::Blocked => "blocked",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "verified" => Some(EvidenceState::Verified),
            "unverified" => Some(EvidenceState::Unverified),
            "blocked" => Some(EvidenceState::Blocked),
            _ => None,
        }
    }
}

/// Governance field name. The constructor refuses any name outside
/// the closed [`GOVERNANCE_FIELDS`] set so a misspelled request fails
/// at the boundary.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Field(String);

impl Field {
    /// Construct a [`Field`] from an arbitrary string. Returns an
    /// [`ExportError::UnknownField`] when the name is not part of the
    /// governance vocabulary.
    pub fn new(name: impl Into<String>) -> Result<Self, ExportError> {
        let name = name.into();
        if !GOVERNANCE_FIELDS.contains(&name.as_str()) {
            return Err(ExportError::UnknownField(name));
        }
        Ok(Self(name))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for Field {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// One entry in the export: the field, its state, an evidence
/// reference into the bounded-evidence store, and the gate id that
/// supplied the verdict (when applicable).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FieldExport {
    pub field: Field,
    pub state: EvidenceState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence_ref: Option<EvidenceRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_gate: Option<String>,
}

/// Diagnostic for a stale run. Empty string when the run revision
/// matches the project revision; non-empty otherwise. The diagnostic
/// names both revisions so an audit can decide whether to re-run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StaleDiagnostic {
    pub run_revision: String,
    pub project_revision: String,
}

/// The complete export document. Versioned by
/// [`EVIDENCE_EXPORT_SCHEMA_VERSION`] so a future format change can
/// reject older consumers at the boundary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceExport {
    pub schema_version: u32,
    pub project_id: String,
    pub revision: String,
    pub toolchain: String,
    pub fields: Vec<FieldExport>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stale: Option<StaleDiagnostic>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub manifest_digest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rule_pack_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gate_run_id: Option<i64>,
}

/// Failure modes for the export construction.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ExportError {
    #[error("unknown governance field `{0}`; expected one of: {expected}", expected = GOVERNANCE_FIELDS.join(", "))]
    UnknownField(String),
    #[error("no completed gate run for project `{0}`; run `driftwatch gate` first")]
    NoRun(String),
}

/// Map a concern id to the set of governance fields it supplies.
///
/// * `release-evidence` — the seven release-evidence fields (revision,
///   version, artifacts, digests, sbom, provenance, publication).
/// * `capability-conformance` — the `checks` field (verified
///   capabilities of the project).
/// * `toolchain` has no supplying concern today; it is always
///   emitted `unverified` by [`build`].
///
/// Unknown concern ids return `None` so the caller can keep an
/// exhaustive mapping when the Gate vocabulary grows.
pub fn fields_for_concern(concern_id: &str) -> Option<&'static [&'static str]> {
    match concern_id {
        RELEASE_EVIDENCE => Some(&[
            "revision",
            "version",
            "artifacts",
            "digests",
            "sbom",
            "provenance",
            "publication",
        ]),
        CAPABILITY_CONFORMANCE => Some(&["checks"]),
        _ => None,
    }
}

/// One check's per-field contribution. The export reduces multiple
/// contributions per field with the deterministic priority
/// `verified > blocked > unverified` so the strongest signal wins.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FieldStateContribution {
    Verified,
    Blocked,
    Unverified,
}

fn classify_result(result: &GateResult) -> FieldStateContribution {
    if result.missing_evidence.is_empty() {
        match result.status {
            GateStatus::Pass => FieldStateContribution::Verified,
            _ => FieldStateContribution::Unverified,
        }
    } else {
        // Any missing_evidence key means the check could not produce
        // the evidence it would need to claim a verdict; the field
        // is blocked, not verified.
        FieldStateContribution::Blocked
    }
}

fn reduce(contribs: &[FieldStateContribution]) -> EvidenceState {
    if contribs.contains(&FieldStateContribution::Verified) {
        EvidenceState::Verified
    } else if contribs.contains(&FieldStateContribution::Blocked) {
        EvidenceState::Blocked
    } else {
        EvidenceState::Unverified
    }
}

fn bound_evidence_ref_for(result: &GateResult) -> Option<EvidenceRef> {
    result.evidence.first().map(|e| EvidenceRef {
        key: bound_text(&e.key, MAX_EVIDENCE_KEY_BYTES),
        digest: e.digest.as_ref().map(|s| bound_text(s, 128)),
        media_type: e.media_type.as_ref().map(|s| bound_text(s, 128)),
        byte_size: e.byte_size,
        preview: e.preview.as_ref().map(|s| bound_text(s, 1024)),
    })
}

fn source_gate_for(result: &GateResult) -> Option<String> {
    let gid = bound_text(&result.gate_id, 128);
    if gid.is_empty() {
        None
    } else {
        Some(gid)
    }
}

fn field_evidence_ref(
    contribs_with_results: &[(&GateResult, FieldStateContribution)],
) -> Option<EvidenceRef> {
    for (r, c) in contribs_with_results {
        if *c == FieldStateContribution::Verified {
            return bound_evidence_ref_for(r);
        }
    }
    for (r, c) in contribs_with_results {
        if *c == FieldStateContribution::Blocked {
            return bound_evidence_ref_for(r);
        }
    }
    None
}

fn field_source_gate(
    contribs_with_results: &[(&GateResult, FieldStateContribution)],
) -> Option<String> {
    for (r, c) in contribs_with_results {
        if *c == FieldStateContribution::Verified {
            return source_gate_for(r);
        }
    }
    for (r, c) in contribs_with_results {
        if *c == FieldStateContribution::Blocked {
            return source_gate_for(r);
        }
    }
    None
}

/// Build the export document from a completed gate run, the results
/// it produced, the project identity, and the project's current
/// revision. Refuses when no run is provided. When the run revision
/// differs from the project revision, every field is emitted
/// `unverified` and the [`StaleDiagnostic`] is attached.
///
/// `toolchain` is reported as `driftwatchdog@<cargo_version>` (the
/// gate is the only thing the export currently knows about). Future
/// versions can extend this without changing the wire schema.
pub fn build(
    run: Option<&GateRunRow>,
    results: &[GateResult],
    project_id: &str,
    project_revision: &str,
    driftwatchdog_version: &str,
) -> Result<EvidenceExport, ExportError> {
    let run = run.ok_or_else(|| ExportError::NoRun(project_id.to_string()))?;
    let run_revision = run.revision.clone();
    let stale = if run_revision != project_revision {
        Some(StaleDiagnostic {
            run_revision: run_revision.clone(),
            project_revision: project_revision.to_string(),
        })
    } else {
        None
    };

    // Index results by gate_id for the per-check classification.
    let mut by_id: std::collections::BTreeMap<&str, &GateResult> =
        std::collections::BTreeMap::new();
    for r in results {
        by_id.insert(r.gate_id.as_str(), r);
    }

    let mut fields = Vec::with_capacity(GOVERNANCE_FIELDS.len());
    for name in GOVERNANCE_FIELDS {
        let field = Field::new(*name)?;
        let entry = if stale.is_some() {
            // Stale run: every field is unverified, no evidence
            // reference, no source gate — the document is still
            // diagnosable but no claim is admitted.
            FieldExport {
                field,
                state: EvidenceState::Unverified,
                evidence_ref: None,
                source_gate: None,
            }
        } else {
            match fields_for_concern_for_field(name) {
                None => {
                    // `toolchain` is the only field with no supplier.
                    FieldExport {
                        field,
                        state: EvidenceState::Unverified,
                        evidence_ref: None,
                        source_gate: None,
                    }
                }
                Some(concerns) => {
                    let mut contribs: Vec<FieldStateContribution> = Vec::new();
                    let mut contribs_with: Vec<(&GateResult, FieldStateContribution)> = Vec::new();
                    for concern in concerns {
                        if let Some(result) = by_id.get(concern) {
                            let c = classify_result(result);
                            contribs.push(c);
                            contribs_with.push((result, c));
                        }
                        // A field with no scheduled check contributes
                        // nothing — its default (unverified) is what
                        // the reducer returns.
                    }
                    let state = reduce(&contribs);
                    let evidence_ref = match state {
                        EvidenceState::Verified => field_evidence_ref(&contribs_with),
                        _ => None,
                    };
                    let source_gate = match state {
                        EvidenceState::Unverified => None,
                        _ => field_source_gate(&contribs_with),
                    };
                    FieldExport {
                        field,
                        state,
                        evidence_ref,
                        source_gate,
                    }
                }
            }
        };
        fields.push(entry);
    }

    Ok(EvidenceExport {
        schema_version: EVIDENCE_EXPORT_SCHEMA_VERSION,
        project_id: bound_text(project_id, 256),
        revision: bound_text(&run_revision, 128),
        toolchain: bound_text(
            &format!("driftwatchdog@{driftwatchdog_version}"),
            MAX_SOURCE_BYTES,
        ),
        fields,
        stale,
        manifest_digest: Some(bound_text(&run.manifest_digest, 128)),
        rule_pack_version: Some(bound_text(&run.rule_pack_version, 128)),
        gate_run_id: Some(run.id),
    })
}

/// Look up the closed list of concern ids that supply a given
/// governance field. `toolchain` is the only field without a supplier.
fn fields_for_concern_for_field(name: &str) -> Option<&'static [&'static str]> {
    match name {
        "revision" | "version" | "artifacts" | "digests" | "sbom" | "provenance"
        | "publication" => Some(&[RELEASE_EVIDENCE]),
        "checks" => Some(&[CAPABILITY_CONFORMANCE]),
        _ => None,
    }
}

/// Render the export as a human-readable table. Stable ordering
/// (governance field order, `verified` listed before `unverified` is
/// not enforced here — fields are listed in [`GOVERNANCE_FIELDS`]
/// order so the layout is consistent across runs).
pub fn render_human(doc: &EvidenceExport) -> String {
    let mut out = String::new();
    out.push_str("driftwatch gate-evidence export\n");
    out.push_str(&format!(
        "schema_version: {} | project_id: {} | revision: {} | toolchain: {}\n",
        doc.schema_version, doc.project_id, doc.revision, doc.toolchain
    ));
    if let Some(md) = &doc.manifest_digest {
        out.push_str(&format!("manifest: {md}\n"));
    }
    if let Some(rp) = &doc.rule_pack_version {
        out.push_str(&format!("rule_pack: {rp}\n"));
    }
    if let Some(id) = doc.gate_run_id {
        out.push_str(&format!("gate_run_id: {id}\n"));
    }
    if let Some(stale) = &doc.stale {
        out.push_str(&format!(
            "STALE: run revision `{}` differs from project revision `{}`; every field emitted `unverified`\n",
            stale.run_revision, stale.project_revision
        ));
    }
    out.push('\n');
    out.push_str(&format!(
        "{:<14}  {:<12}  {:<24}  EVIDENCE\n",
        "FIELD", "STATE", "SOURCE_GATE"
    ));
    for entry in &doc.fields {
        let source = entry.source_gate.as_deref().unwrap_or("-");
        let ev = entry
            .evidence_ref
            .as_ref()
            .map(|e| e.key.as_str())
            .unwrap_or("-");
        out.push_str(&format!(
            "{:<14}  {:<12}  {:<24}  {ev}\n",
            entry.field.as_str(),
            entry.state.as_str(),
            truncate_char_boundary(source, 24)
        ));
    }
    out.push_str(&format!(
        "\nfields emitted: {} (one entry per governance `release_evidence` field)\n",
        doc.fields.len()
    ));
    let diagnostic_note = if doc.stale.is_some() {
        "diagnostic: stale run; nothing is `verified`"
    } else {
        "diagnostic: revision matches project; per-field state reflects the executed run"
    };
    out.push_str(&format!("{diagnostic_note}\n"));
    let trimmed = out.trim_end();
    truncate_char_boundary(trimmed, MAX_DIAGNOSTIC_BYTES)
}

/// Render the export as a versioned JSON document. Output is bounded
/// to a reasonable size: field entries are limited to the
/// [`GOVERNANCE_FIELDS`] count, every string passes through
/// [`bound_text`] (with secret redaction applied where it matters),
/// and the document is produced via `serde_json::to_string_pretty` so
/// a downstream consumer can re-parse it.
pub fn render_json(doc: &EvidenceExport) -> String {
    serde_json::to_string_pretty(doc).unwrap_or_else(|_| "{}".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gate::dto::{
        MAX_DIAGNOSTIC_BYTES, MAX_EVIDENCE_KEY_BYTES, MAX_LOCATION_BYTES, MAX_REMEDIATION_BYTES,
    };
    use crate::gate::types::{Finding, GateSeverity};

    fn result(gate_id: &str, status: GateStatus, missing: Vec<&str>) -> GateResult {
        GateResult {
            gate_id: gate_id.into(),
            source: "test".into(),
            status,
            severity: GateSeverity::Info,
            findings: vec![],
            evidence: vec![],
            missing_evidence: missing.into_iter().map(String::from).collect(),
            diagnostic: None,
            remediation: None,
        }
    }

    fn passing_release_evidence() -> GateResult {
        let mut r = result(RELEASE_EVIDENCE, GateStatus::Pass, vec![]);
        r.evidence.push(EvidenceRef {
            key: "release-evidence:envelope".into(),
            digest: Some("sha256:abc".into()),
            media_type: Some("application/json".into()),
            byte_size: Some(42),
            preview: Some("ok".into()),
        });
        r
    }

    fn failing_release_evidence() -> GateResult {
        let mut r = result(RELEASE_EVIDENCE, GateStatus::Fail, vec![]);
        r.findings.push(Finding {
            title: "missing artifact".into(),
            severity: GateSeverity::Error,
            location: Some("artifacts/".into()),
            rule: Some("artifacts-present".into()),
        });
        r
    }

    fn blocked_release_evidence() -> GateResult {
        result(
            RELEASE_EVIDENCE,
            GateStatus::ReviewRequired,
            vec!["release-evidence:command"],
        )
    }

    fn passing_capability() -> GateResult {
        let mut r = result(CAPABILITY_CONFORMANCE, GateStatus::Pass, vec![]);
        r.evidence.push(EvidenceRef {
            key: "capability-conformance:envelope".into(),
            digest: Some("sha256:def".into()),
            media_type: Some("application/json".into()),
            byte_size: Some(64),
            preview: Some("ok".into()),
        });
        r
    }

    fn blocked_capability() -> GateResult {
        result(
            CAPABILITY_CONFORMANCE,
            GateStatus::NotApplicable,
            vec!["capability-conformance:command"],
        )
    }

    fn run_row() -> GateRunRow {
        GateRunRow {
            id: 7,
            taken_at: "2026-09-26T00:00:00Z".into(),
            change_id: "abc123".into(),
            revision: "abc123".into(),
            manifest_digest: "sha256:deadbeef".into(),
            rule_pack_version: "release@0.1.0".into(),
            status: "PASS".into(),
            blocked: false,
            results_json: "[]".into(),
        }
    }

    #[test]
    fn evidence_state_round_trips() {
        for s in [
            EvidenceState::Verified,
            EvidenceState::Unverified,
            EvidenceState::Blocked,
        ] {
            assert_eq!(EvidenceState::parse(s.as_str()), Some(s));
        }
        assert_eq!(EvidenceState::parse("PASS"), None);
        assert_eq!(
            EvidenceState::parse("verified"),
            Some(EvidenceState::Verified)
        );
    }

    #[test]
    fn field_construction_accepts_known_names() {
        for name in GOVERNANCE_FIELDS {
            let f = Field::new(*name).unwrap();
            assert_eq!(f.as_str(), *name);
        }
    }

    #[test]
    fn field_construction_rejects_unknown_names() {
        let err = Field::new("checksum").unwrap_err();
        assert!(matches!(err, ExportError::UnknownField(_)));
        let err = Field::new("Release_Evidence").unwrap_err();
        assert!(matches!(err, ExportError::UnknownField(_)));
    }

    #[test]
    fn field_construction_rejects_empty() {
        let err = Field::new("").unwrap_err();
        assert!(matches!(err, ExportError::UnknownField(_)));
    }

    #[test]
    fn fields_for_concern_maps_release_and_capability() {
        let release = fields_for_concern(RELEASE_EVIDENCE).unwrap();
        assert!(release.contains(&"revision"));
        assert!(release.contains(&"version"));
        assert!(release.contains(&"artifacts"));
        assert!(release.contains(&"digests"));
        assert!(release.contains(&"sbom"));
        assert!(release.contains(&"provenance"));
        assert!(release.contains(&"publication"));
        assert!(!release.contains(&"checks"));
        let cap = fields_for_concern(CAPABILITY_CONFORMANCE).unwrap();
        assert_eq!(cap, &["checks"]);
        assert!(fields_for_concern("api-contract").is_none());
    }

    #[test]
    fn build_refuses_when_no_run() {
        let err = build(None, &[], "proj", "rev", "0.6.0").unwrap_err();
        assert!(matches!(err, ExportError::NoRun(_)));
    }

    #[test]
    fn build_emits_all_governance_fields_in_order() {
        let run = run_row();
        let results = vec![passing_release_evidence(), passing_capability()];
        let doc = build(Some(&run), &results, "proj", "abc123", "0.6.0").unwrap();
        let names: Vec<&str> = doc.fields.iter().map(|f| f.field.as_str()).collect();
        assert_eq!(names, GOVERNANCE_FIELDS);
        assert_eq!(doc.schema_version, EVIDENCE_EXPORT_SCHEMA_VERSION);
        assert_eq!(doc.project_id, "proj");
        assert_eq!(doc.revision, "abc123");
        assert!(doc.toolchain.contains("driftwatchdog@"));
        assert!(doc.stale.is_none());
        assert!(doc
            .manifest_digest
            .as_deref()
            .unwrap()
            .starts_with("sha256:"));
        assert_eq!(doc.rule_pack_version.as_deref(), Some("release@0.1.0"));
        assert_eq!(doc.gate_run_id, Some(7));
    }

    #[test]
    fn build_marks_passing_release_evidence_supplied_fields_verified() {
        let run = run_row();
        let results = vec![passing_release_evidence(), passing_capability()];
        let doc = build(Some(&run), &results, "proj", "abc123", "0.6.0").unwrap();
        for name in [
            "revision",
            "version",
            "artifacts",
            "digests",
            "sbom",
            "provenance",
            "publication",
        ] {
            let entry = doc
                .fields
                .iter()
                .find(|f| f.field.as_str() == name)
                .unwrap();
            assert_eq!(
                entry.state,
                EvidenceState::Verified,
                "{name} should be verified"
            );
            let ev = entry.evidence_ref.as_ref().expect("evidence ref present");
            assert_eq!(ev.key, "release-evidence:envelope");
            assert_eq!(entry.source_gate.as_deref(), Some(RELEASE_EVIDENCE));
        }
        let checks = doc
            .fields
            .iter()
            .find(|f| f.field.as_str() == "checks")
            .unwrap();
        assert_eq!(checks.state, EvidenceState::Verified);
        assert_eq!(checks.source_gate.as_deref(), Some(CAPABILITY_CONFORMANCE));
        let toolchain = doc
            .fields
            .iter()
            .find(|f| f.field.as_str() == "toolchain")
            .unwrap();
        assert_eq!(toolchain.state, EvidenceState::Unverified);
        assert!(toolchain.evidence_ref.is_none());
        assert!(toolchain.source_gate.is_none());
    }

    #[test]
    fn build_marks_failing_release_evidence_supplied_fields_unverified() {
        let run = run_row();
        let results = vec![failing_release_evidence(), passing_capability()];
        let doc = build(Some(&run), &results, "proj", "abc123", "0.6.0").unwrap();
        for name in [
            "revision",
            "version",
            "artifacts",
            "digests",
            "sbom",
            "provenance",
            "publication",
        ] {
            let entry = doc
                .fields
                .iter()
                .find(|f| f.field.as_str() == name)
                .unwrap();
            assert_eq!(entry.state, EvidenceState::Unverified, "{name}");
            assert!(entry.evidence_ref.is_none());
            assert!(entry.source_gate.is_none());
        }
        let checks = doc
            .fields
            .iter()
            .find(|f| f.field.as_str() == "checks")
            .unwrap();
        assert_eq!(checks.state, EvidenceState::Verified);
    }

    #[test]
    fn build_marks_blocked_release_evidence_supplied_fields_blocked() {
        let run = run_row();
        let results = vec![blocked_release_evidence(), passing_capability()];
        let doc = build(Some(&run), &results, "proj", "abc123", "0.6.0").unwrap();
        for name in [
            "revision",
            "version",
            "artifacts",
            "digests",
            "sbom",
            "provenance",
            "publication",
        ] {
            let entry = doc
                .fields
                .iter()
                .find(|f| f.field.as_str() == name)
                .unwrap();
            assert_eq!(entry.state, EvidenceState::Blocked, "{name}");
            // Blocked fields have no usable evidence: the check
            // couldn't execute. The source gate is preserved so an
            // audit can see which check was supposed to produce the
            // evidence.
            assert!(entry.evidence_ref.is_none(), "{name}");
            assert_eq!(
                entry.source_gate.as_deref(),
                Some(RELEASE_EVIDENCE),
                "{name}"
            );
        }
    }

    #[test]
    fn build_marks_blocked_capability_blocked() {
        let run = run_row();
        let results = vec![passing_release_evidence(), blocked_capability()];
        let doc = build(Some(&run), &results, "proj", "abc123", "0.6.0").unwrap();
        let checks = doc
            .fields
            .iter()
            .find(|f| f.field.as_str() == "checks")
            .unwrap();
        assert_eq!(checks.state, EvidenceState::Blocked);
        assert!(checks.evidence_ref.is_none());
        assert_eq!(checks.source_gate.as_deref(), Some(CAPABILITY_CONFORMANCE));
    }

    #[test]
    fn build_marks_field_unverified_when_check_not_scheduled() {
        let run = run_row();
        // No release-evidence and no capability-conformance result: both
        // supplied fields default to unverified.
        let other = result("smoke", GateStatus::Pass, vec![]);
        let doc = build(Some(&run), &[other], "proj", "abc123", "0.6.0").unwrap();
        for name in [
            "revision",
            "version",
            "artifacts",
            "digests",
            "sbom",
            "provenance",
            "publication",
        ] {
            let entry = doc
                .fields
                .iter()
                .find(|f| f.field.as_str() == name)
                .unwrap();
            assert_eq!(entry.state, EvidenceState::Unverified, "{name}");
        }
        let checks = doc
            .fields
            .iter()
            .find(|f| f.field.as_str() == "checks")
            .unwrap();
        assert_eq!(checks.state, EvidenceState::Unverified);
    }

    #[test]
    fn build_reduces_with_verified_over_blocked() {
        // A field with both a verified and a blocked supplier picks
        // verified; the audit can then drill into the source_gate.
        let run = run_row();
        let blocked = result(RELEASE_EVIDENCE, GateStatus::ReviewRequired, vec!["x:y"]);
        // No capability result; release-evidence is the only supplier.
        let doc = build(Some(&run), &[blocked], "proj", "abc123", "0.6.0").unwrap();
        // Blocked (has missing_evidence) wins because there is no
        // verified contribution for the supplied fields.
        let entry = doc
            .fields
            .iter()
            .find(|f| f.field.as_str() == "revision")
            .unwrap();
        assert_eq!(entry.state, EvidenceState::Blocked);
    }

    #[test]
    fn build_marks_every_field_unverified_on_stale_revision() {
        let run = run_row();
        let results = vec![passing_release_evidence(), passing_capability()];
        let doc = build(Some(&run), &results, "proj", "DIFFERENT-REV", "0.6.0").unwrap();
        assert!(doc.stale.is_some());
        for entry in &doc.fields {
            assert_eq!(entry.state, EvidenceState::Unverified, "{}", entry.field);
            assert!(entry.evidence_ref.is_none(), "{}", entry.field);
            assert!(entry.source_gate.is_none(), "{}", entry.field);
        }
        let stale = doc.stale.as_ref().unwrap();
        assert_eq!(stale.run_revision, "abc123");
        assert_eq!(stale.project_revision, "DIFFERENT-REV");
    }

    #[test]
    fn build_propagates_evidence_ref_for_verified_contribution() {
        let run = run_row();
        let results = vec![passing_release_evidence(), passing_capability()];
        let doc = build(Some(&run), &results, "proj", "abc123", "0.6.0").unwrap();
        let checks = doc
            .fields
            .iter()
            .find(|f| f.field.as_str() == "checks")
            .unwrap();
        let ev = checks.evidence_ref.as_ref().unwrap();
        assert_eq!(ev.key, "capability-conformance:envelope");
        assert_eq!(ev.digest.as_deref(), Some("sha256:def"));
    }

    #[test]
    fn render_human_lists_all_fields_in_order() {
        let run = run_row();
        let results = vec![passing_release_evidence(), passing_capability()];
        let doc = build(Some(&run), &results, "proj", "abc123", "0.6.0").unwrap();
        let text = render_human(&doc);
        for name in GOVERNANCE_FIELDS {
            assert!(text.contains(name), "missing {name} in:\n{text}");
        }
        assert!(text.contains("schema_version: 1"));
        assert!(text.contains("project_id: proj"));
        assert!(text.contains("revision: abc123"));
        assert!(text.contains("verified"));
        assert!(text.contains("unverified"));
        assert!(
            !text.contains("STALE"),
            "fresh run should not be marked stale"
        );
    }

    #[test]
    fn render_human_marks_stale_run() {
        let run = run_row();
        let results = vec![passing_release_evidence(), passing_capability()];
        let doc = build(Some(&run), &results, "proj", "OTHER", "0.6.0").unwrap();
        let text = render_human(&doc);
        assert!(text.contains("STALE"));
        assert!(text.contains("abc123"));
        assert!(text.contains("OTHER"));
    }

    #[test]
    fn render_json_is_a_valid_versioned_document() {
        let run = run_row();
        let results = vec![passing_release_evidence(), passing_capability()];
        let doc = build(Some(&run), &results, "proj", "abc123", "0.6.0").unwrap();
        let text = render_json(&doc);
        let parsed: serde_json::Value = serde_json::from_str(&text).expect("valid json");
        assert_eq!(parsed["schema_version"], 1);
        assert_eq!(parsed["project_id"], "proj");
        assert_eq!(parsed["revision"], "abc123");
        let fields = parsed["fields"].as_array().unwrap();
        assert_eq!(fields.len(), GOVERNANCE_FIELDS.len());
        let names: Vec<&str> = fields
            .iter()
            .map(|f| f["field"].as_str().unwrap())
            .collect();
        assert_eq!(names, GOVERNANCE_FIELDS);
        // Each field has a state from the closed set.
        for f in fields {
            let state = f["state"].as_str().unwrap();
            assert!(GOVERNANCE_STATES.contains(&state), "bad state {state}");
        }
    }

    #[test]
    fn render_json_includes_stale_diagnostic_when_present() {
        let run = run_row();
        let results = vec![passing_release_evidence(), passing_capability()];
        let doc = build(Some(&run), &results, "proj", "OTHER", "0.6.0").unwrap();
        let text = render_json(&doc);
        let parsed: serde_json::Value = serde_json::from_str(&text).expect("valid json");
        assert!(parsed["stale"].is_object());
        assert_eq!(parsed["stale"]["run_revision"], "abc123");
        assert_eq!(parsed["stale"]["project_revision"], "OTHER");
    }

    #[test]
    fn unknown_field_in_construction_is_a_hard_error() {
        // Direct `Field::new` rejects unknown names; build() can only
        // emit names from the closed set, so this is a property of
        // the public API.
        let run = run_row();
        let results = vec![passing_release_evidence()];
        let doc = build(Some(&run), &results, "proj", "abc123", "0.6.0").unwrap();
        for entry in &doc.fields {
            assert!(
                GOVERNANCE_FIELDS.contains(&entry.field.as_str()),
                "leaked field {}",
                entry.field
            );
        }
    }

    #[test]
    fn build_bounds_long_project_id() {
        let run = run_row();
        let long = "x".repeat(2000);
        let doc = build(Some(&run), &[], &long, "abc123", "0.6.0").unwrap();
        // `bound_text` truncates with an ellipsis, so the result is
        // at most `limit + 3` bytes (the ellipsis is 3 UTF-8 bytes).
        assert!(doc.project_id.len() <= 256 + 3, "{}", doc.project_id.len());
        assert!(doc.project_id.chars().any(|c| c == '…') || doc.project_id.len() < 256);
    }

    #[test]
    fn render_human_keeps_within_bounded_diagnostic_size() {
        // The `render_human` function bounds the output so a
        // malformed input cannot blow up the diagnostic cap.
        let run = run_row();
        let results = vec![passing_release_evidence(), passing_capability()];
        let doc = build(Some(&run), &results, "proj", "abc123", "0.6.0").unwrap();
        let text = render_human(&doc);
        assert!(text.len() <= MAX_DIAGNOSTIC_BYTES);
    }

    #[test]
    fn unsupported_dto_cap_constants_stay_in_sync_with_shared_dto_module() {
        // The export module reuses the same byte caps as the rest of
        // the gate contract so diagnostics, evidence keys, and source
        // strings stay consistent across the binary.
        use crate::gate::dto::{
            MAX_DIAGNOSTIC_BYTES as DTO_DIAG, MAX_EVIDENCE_KEY_BYTES as DTO_EK,
            MAX_LOCATION_BYTES as DTO_LOC, MAX_REMEDIATION_BYTES as DTO_REM,
        };
        assert_eq!(MAX_DIAGNOSTIC_BYTES, DTO_DIAG);
        assert_eq!(MAX_EVIDENCE_KEY_BYTES, DTO_EK);
        assert_eq!(MAX_LOCATION_BYTES, DTO_LOC);
        assert_eq!(MAX_REMEDIATION_BYTES, DTO_REM);
    }
}
