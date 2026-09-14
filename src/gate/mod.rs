//! Generic gate contract.
//!
//! Language- and specification-system-neutral result model for
//! deterministic and semantic checks. This module owns the in-memory
//! domain types; [`dto`] owns the versioned JSON boundary; [`aggregate`]
//! owns the deterministic blocking policy; [`redact`] owns secret-safe,
//! bounded diagnostics; [`adapt`] maps the existing checker protocol to
//! this contract without changing `driftwatch check` persistence;
//! [`evidence`] owns bounded artifact persistence, path confinement, and
//! the evidence-backed-`PASS` guard.
//!
//! No OpenSpec types appear here. No tool is installed, no network call
//! is made, and no LLM is invoked.

pub mod adapt;
pub mod aggregate;
pub mod dto;
pub mod evidence;
pub mod manifest;
pub mod redact;
pub mod types;

pub use adapt::adapt_checker_outcome;
pub use aggregate::{aggregate, AggregateOutcome, BlockingPolicy, GatePlan, PlannedCheck};
pub use dto::{
    parse_gate_result_document, to_canonical_json, DtoError, GATE_CONTRACT_VERSION,
    MAX_DIAGNOSTIC_BYTES, MAX_EVIDENCE_REFS, MAX_FINDINGS, MAX_MISSING_EVIDENCE,
    MAX_REMEDIATION_BYTES, MAX_RESULTS_PER_DOC, MAX_RESULT_BYTES,
};
pub use evidence::{
    build_record, confine_adapter_path, confined_path, digest_bytes, evidence_backed_pass,
    store_bytes, validate_key, ArtifactKind, ArtifactRecord, EvidenceError, NewArtifact,
    ARTIFACT_DIR_NAME, MAX_ARTIFACT_BYTES, MAX_KEY_BYTES, MAX_PREVIEW_BYTES, UNAVAILABLE_PREVIEW,
};
pub use manifest::{
    load as load_gate_manifest, manifest_path as gate_manifest_path, parse as parse_gate_manifest,
    render_plan as render_gate_plan, resolve as resolve_gate_manifest,
};
pub use redact::{
    bound_text_with_extra, bounded_diagnostic, redact_secrets, redact_secrets_with_extra,
};
pub use types::{EvidenceRef, Finding, GateResult, GateSeverity, GateStatus};
