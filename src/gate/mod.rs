//! Generic gate contract.
//!
//! Language- and specification-system-neutral result model for
//! deterministic and semantic checks. This module owns the in-memory
//! domain types; [`dto`] owns the versioned JSON boundary; [`aggregate`]
//! owns the deterministic blocking policy; [`redact`] owns secret-safe,
//! bounded diagnostics; [`adapt`] maps the existing checker protocol to
//! this contract without changing `driftwatch check` persistence.
//!
//! No OpenSpec types appear here. No tool is installed, no network call
//! is made, and no LLM is invoked.

pub mod adapt;
pub mod aggregate;
pub mod dto;
pub mod redact;
pub mod types;

pub use adapt::adapt_checker_outcome;
pub use aggregate::{aggregate, AggregateOutcome, BlockingPolicy, GatePlan, PlannedCheck};
pub use dto::{
    parse_gate_result_document, to_canonical_json, DtoError, GATE_CONTRACT_VERSION,
    MAX_DIAGNOSTIC_BYTES, MAX_EVIDENCE_REFS, MAX_FINDINGS, MAX_MISSING_EVIDENCE,
    MAX_REMEDIATION_BYTES, MAX_RESULTS_PER_DOC, MAX_RESULT_BYTES,
};
pub use redact::{bounded_diagnostic, redact_secrets};
pub use types::{EvidenceRef, Finding, GateResult, GateSeverity, GateStatus};
