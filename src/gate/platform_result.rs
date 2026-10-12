//! Machine-readable emission of a completed gate run as a
//! `platform.gate-result` document.
//!
//! This module is the only producer of that contract in the portfolio.
//! It is a **pure mapping with an exit**: given an executed run it either
//! returns a complete document that conforms to the vendored schema, or a
//! typed [`Refusal`] explaining exactly which value could not be
//! represented without losing or inventing a claim.
//!
//! The contract's `result` and `checks[].status` share one six-value
//! vocabulary and driftwatchdog has five [`GateStatus`] states, so the
//! mapping is total and lossless at major 1:
//!
//! | [`GateStatus`] | platform | claim |
//! |---|---|---|
//! | `PASS` | `passed` | ran and passed |
//! | `FAIL` | `failed` | ran and reported a failure; severity stays orthogonal and unmapped |
//! | `NOT_APPLICABLE` | `skipped` | a proven exemption |
//! | `REVIEW_REQUIRED` | `review_required` | a human decision is still open |
//! | `UNBOUND` | `unbound` | asserted, no command bound, nothing ran |
//!
//! `review_required` and `unbound` are not approximations. The schema defines
//! them with exactly the claims [`GateStatus`] already makes, and forbids the
//! merge this runtime's own status model also forbids: *"A consumer MUST NOT
//! read `skipped` and `unbound` alike, and MUST NOT read `review_required` as
//! a decision already taken."* At contract major 0 those two states had no
//! representation and this emitter refused them; at major 1 refusing them
//! would decline to state a claim the contract states exactly.
//!
//! `errored` is never emitted. Every execution failure driftwatchdog observes
//! (timeout, spawn failure, malformed envelope, exit-code contradiction) is
//! already normalized to `REVIEW_REQUIRED`, which now has an honest value of
//! its own. It stays defined — it is a value of the contract and a consumer
//! of this type must be able to name it — and a test holds it unreachable.
//!
//! Identity and length are where refusal still lives. A check id comes from a
//! project manifest that validates only that it is non-empty, while the
//! platform requires `^[a-z][a-z0-9_.-]{0,63}$`. There is no lossless
//! reversible slug inside 64 characters for arbitrary ids — lowercasing
//! merges `L1-configuration` with `l1-configuration`, truncation merges
//! long ids with each other, and every encoding scheme that would need a
//! parser to be reversible is a scheme the first opaque-string consumer
//! will misread. So conforming ids pass through **unchanged** and everything
//! else is refused, exactly as an over-length diagnostic or evidence key is.
//!
//! Nothing here reads the clock, the filesystem, the database, or the
//! manifest, so a unit test and a production run take the same branch. No
//! JSON-Schema validator is used: the consumer owns validation, and a
//! second copy of the schema's rules inside the producer would mask schema
//! drift rather than surface it.

use serde::{Deserialize, Serialize};

use crate::gate::redact::redact_secrets;
use crate::gate::types::{GateResult, GateStatus};

/// The `pipeline` value this producer reports. A pipeline is a property of
/// whatever produced the document, and for a `driftwatch gate` run that is
/// always driftwatchdog's gate — never the project-owned profile, whose
/// shape no platform pattern can express. The profile is preserved verbatim
/// in the root `driftwatch_profile` additional property instead.
pub const PLATFORM_PIPELINE: &str = "driftwatchdog-gate";

/// The version of the `platform.gate-result` contract this producer claims,
/// emitted as the root `contract` property.
///
/// The registry serves contract majors 0 and 1 simultaneously today, so a
/// document carrying only `passed` / `failed` / `skipped` is ambiguous between
/// a 0.1.0 producer and a 1.0.0 producer that happened to have no open
/// decision and no unbound check. The producer resolves that ambiguity, not
/// the consumer.
///
/// Shape follows the in-repo precedent at `crate::checker::json`
/// (`CHECKER_REPORT_CONTRACT` → a root `contract` field): one named constant,
/// one field name, `name/major.minor.patch`. A second spelling for the same
/// concept would give every future consumer a branch.
pub const PLATFORM_GATE_RESULT_CONTRACT: &str = "platform.gate-result/1.0.0";

/// Platform `result` / `checks[].status` vocabulary, contract major 1.
///
/// `Errored` is part of the contract and is deliberately never constructed by
/// [`map_status`]; see the module docs. It is still defined and still
/// round-trips through [`PlatformStatus::parse`], because a consumer of this
/// type must be able to name every value the contract defines.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PlatformStatus {
    Passed,
    Failed,
    /// Never produced by this runtime. See the module docs.
    Errored,
    Skipped,
    /// A human decision is still open and no one has decided the outcome.
    ReviewRequired,
    /// The check was asserted, no command was bound to it, and nothing ran.
    Unbound,
}

impl PlatformStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            PlatformStatus::Passed => "passed",
            PlatformStatus::Failed => "failed",
            PlatformStatus::Errored => "errored",
            PlatformStatus::Skipped => "skipped",
            PlatformStatus::ReviewRequired => "review_required",
            PlatformStatus::Unbound => "unbound",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "passed" => Some(PlatformStatus::Passed),
            "failed" => Some(PlatformStatus::Failed),
            "errored" => Some(PlatformStatus::Errored),
            "skipped" => Some(PlatformStatus::Skipped),
            "review_required" => Some(PlatformStatus::ReviewRequired),
            "unbound" => Some(PlatformStatus::Unbound),
            _ => None,
        }
    }
}

/// Map one [`GateStatus`] onto the platform vocabulary.
///
/// **Total and infallible at contract major 1**: every state has an honest
/// value, so there is no failure arm and no `Result`. Adding a sixth
/// [`GateStatus`] variant is a compile error here rather than a silent
/// emission.
///
/// The same function serves the aggregate `result` and every
/// `checks[].status`, which is also what the contract states: the vocabulary
/// is shared and each value carries the same meaning at both levels.
pub fn map_status(status: GateStatus) -> PlatformStatus {
    match status {
        GateStatus::Pass => PlatformStatus::Passed,
        GateStatus::Fail => PlatformStatus::Failed,
        GateStatus::NotApplicable => PlatformStatus::Skipped,
        GateStatus::ReviewRequired => PlatformStatus::ReviewRequired,
        GateStatus::Unbound => PlatformStatus::Unbound,
    }
}

/// One emitted check. `driftwatch_gate_id` is an additional property the
/// schema permits; it repeats the driftwatchdog id verbatim. The mapping is
/// the identity today, so it is redundant — it is emitted precisely so that
/// the round trip stays machine-checkable if the mapping ever has to
/// normalise, and so a consumer never has to guess which vocabulary an id
/// belongs to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlatformCheck {
    pub check_id: String,
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    pub driftwatch_gate_id: String,
}

/// A complete `platform.gate-result` document.
///
/// `contract` is the declared contract version
/// ([`PLATFORM_GATE_RESULT_CONTRACT`]) and `driftwatch_profile` is an
/// additional property carrying the project-owned profile verbatim. The schema
/// constrains neither, which is exactly why the profile cannot be used as
/// `pipeline`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlatformGateResult {
    pub contract: String,
    pub gate_id: String,
    pub pipeline: String,
    pub evaluated_at: String,
    pub result: String,
    pub subject_ref: Option<String>,
    pub checks: Vec<PlatformCheck>,
    pub evidence_refs: Vec<String>,
    pub driftwatch_profile: String,
}

/// Everything the emitter needs about one executed run. Borrowed, so the
/// mapping allocates only what the document contains.
#[derive(Debug, Clone, Copy)]
pub struct PlatformRun<'a> {
    /// The run's manifest digest (`sha256:` + hex); also the run's identity
    /// in `gate_runs`.
    pub gate_id: &'a str,
    /// The pipeline reporting itself. The CLI passes [`PLATFORM_PIPELINE`];
    /// the parameter exists so the pattern check is reachable and testable.
    pub pipeline: &'a str,
    /// Project-owned profile name, carried verbatim.
    pub profile: &'a str,
    /// The run's RFC 3339 timestamp.
    pub evaluated_at: &'a str,
    /// The run's gated revision, when one was captured.
    pub subject_ref: Option<&'a str>,
    /// The aggregate status of the run.
    pub aggregate: GateStatus,
    /// Every normalized result, in plan order.
    pub results: &'a [GateResult],
}

/// Why a document could not be emitted. Every variant names the exact value
/// and the exact constraint it violated.
///
/// There is no status variant: at contract major 1 every [`GateStatus`] has an
/// honest value, so a refusal is always about an *identifier or a length* the
/// contract bounds and driftwatchdog will not truncate to fit.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum Refusal {
    #[error("gate_id `{value}` does not match the platform pattern {pattern}")]
    GateId {
        value: String,
        pattern: &'static str,
    },

    #[error("pipeline `{value}` does not match the platform pattern {pattern}")]
    Pipeline {
        value: String,
        pattern: &'static str,
    },

    #[error("subject_ref is {length} characters; the platform allows at most {max} and driftwatchdog does not truncate it")]
    SubjectRef { length: usize, max: usize },

    #[error("evaluated_at `{value}` is not an RFC 3339 date-time")]
    EvaluatedAt { value: String },

    #[error(
        "check id `{value}` does not match the platform pattern {pattern}; \
         no reversible slug exists inside {max} characters, so driftwatchdog refuses instead of coercing it"
    )]
    CheckId {
        value: String,
        pattern: &'static str,
        max: usize,
    },

    #[error(
        "check `{check_id}` has a {length}-character diagnostic; the platform allows at most {max} \
         for detail and driftwatchdog does not truncate a diagnostic to fit"
    )]
    Detail {
        check_id: String,
        length: usize,
        max: usize,
    },

    #[error("evidence reference key `{key}` cannot be represented: the platform requires {min} to {max} characters")]
    EvidenceRef { key: String, min: usize, max: usize },
}

pub const GATE_ID_PATTERN: &str = "^[A-Za-z0-9][A-Za-z0-9._:-]{0,127}$";
pub const PIPELINE_PATTERN: &str = "^[a-z][a-z0-9_.-]{0,127}$";
pub const CHECK_ID_PATTERN: &str = "^[a-z][a-z0-9_.-]{0,63}$";

/// `maxLength` for `subject_ref`, in Unicode code points.
pub const MAX_SUBJECT_REF_CHARS: usize = 256;
/// `maxLength` for `checks[].detail`, in Unicode code points.
pub const MAX_DETAIL_CHARS: usize = 512;
/// `maxLength` for each `evidence_refs` entry, in Unicode code points.
pub const MAX_EVIDENCE_REF_CHARS: usize = 512;
/// `maxLength` for `checks[].check_id`, in Unicode code points.
pub const MAX_CHECK_ID_CHARS: usize = 64;

/// Build the platform document, or refuse.
///
/// Pure: the only inputs are the arguments of `run`. Nothing is printed here
/// and nothing is written anywhere — the caller prints a complete document or
/// prints nothing at all, so a partial emission is not representable.
///
/// Refusals are evaluated in a fixed order — the root identity, then the
/// per-check shape — so a substantive refusal is never masked by a cosmetic
/// one, and the diagnostic is identical on every run. No status refuses at
/// contract major 1: [`map_status`] is total.
pub fn build(run: PlatformRun<'_>) -> Result<PlatformGateResult, Refusal> {
    // 1. Root identity.
    if !matches_gate_id(run.gate_id) {
        return Err(Refusal::GateId {
            value: run.gate_id.to_string(),
            pattern: GATE_ID_PATTERN,
        });
    }
    if !matches_pipeline(run.pipeline) {
        return Err(Refusal::Pipeline {
            value: run.pipeline.to_string(),
            pattern: PIPELINE_PATTERN,
        });
    }
    if let Some(subject) = run.subject_ref {
        let length = subject.chars().count();
        if length > MAX_SUBJECT_REF_CHARS {
            return Err(Refusal::SubjectRef {
                length,
                max: MAX_SUBJECT_REF_CHARS,
            });
        }
    }
    if chrono::DateTime::parse_from_rfc3339(run.evaluated_at).is_err() {
        return Err(Refusal::EvaluatedAt {
            value: run.evaluated_at.to_string(),
        });
    }

    // 2. Per-check shape: id, then detail, then evidence references. The
    //    status itself is mapped above the identity checks — it is total, so
    //    it can neither refuse nor be masked.
    let mut checks = Vec::with_capacity(run.results.len());
    for r in run.results {
        if !matches_check_id(&r.gate_id) {
            return Err(Refusal::CheckId {
                value: r.gate_id.clone(),
                pattern: CHECK_ID_PATTERN,
                max: MAX_CHECK_ID_CHARS,
            });
        }
        // Redaction removes content; bounding would invent it. Only
        // `redact_secrets` is applied, and the length is then checked rather
        // than clamped.
        let detail = r
            .diagnostic
            .as_deref()
            .map(redact_secrets)
            .filter(|d| !d.is_empty());
        if let Some(detail) = detail.as_deref() {
            let length = detail.chars().count();
            if length > MAX_DETAIL_CHARS {
                return Err(Refusal::Detail {
                    check_id: r.gate_id.clone(),
                    length,
                    max: MAX_DETAIL_CHARS,
                });
            }
        }
        checks.push(PlatformCheck {
            check_id: r.gate_id.clone(),
            status: map_status(r.status).as_str().to_string(),
            detail,
            driftwatch_gate_id: r.gate_id.clone(),
        });
    }

    let mut evidence_refs: Vec<String> = Vec::new();
    for r in run.results {
        for e in &r.evidence {
            let key = redact_secrets(&e.key);
            let length = key.chars().count();
            if length == 0 || length > MAX_EVIDENCE_REF_CHARS {
                return Err(Refusal::EvidenceRef {
                    key: e.key.clone(),
                    min: 1,
                    max: MAX_EVIDENCE_REF_CHARS,
                });
            }
            // First-seen order, duplicates removed: deterministic, and a
            // consumer resolving a key reaches the same artifact the run did.
            if !evidence_refs.iter().any(|k| k == &key) {
                evidence_refs.push(key);
            }
        }
    }

    Ok(PlatformGateResult {
        contract: PLATFORM_GATE_RESULT_CONTRACT.to_string(),
        gate_id: run.gate_id.to_string(),
        pipeline: run.pipeline.to_string(),
        evaluated_at: run.evaluated_at.to_string(),
        result: map_status(run.aggregate).as_str().to_string(),
        subject_ref: run.subject_ref.map(str::to_string),
        checks,
        evidence_refs,
        driftwatch_profile: run.profile.to_string(),
    })
}

/// Total-length caps from the `{0,n}` quantifiers in the patterns. These are
/// schema `maxLength`s expressed as whole-string limits: the leading class
/// plus its repetition.
pub const MAX_GATE_ID_CHARS: usize = 128;
pub const MAX_PIPELINE_CHARS: usize = 128;

/// `^[A-Za-z0-9][A-Za-z0-9._:-]{0,127}$` as a direct ASCII scan. The
/// pattern is an anchored ASCII character class, so a scan is equivalent to
/// the regex and costs no dependency. The `{0,127}` tail is enforced as a
/// whole-string length cap — a scan that checks the alphabet but forgets the
/// quantifier accepts ids the schema rejects.
fn matches_gate_id(s: &str) -> bool {
    if s.chars().count() > MAX_GATE_ID_CHARS {
        return false;
    }
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphanumeric() => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | ':' | '-'))
}

/// `^[a-z][a-z0-9_.-]{0,127}$` as a direct ASCII scan.
fn matches_pipeline(s: &str) -> bool {
    if s.chars().count() > MAX_PIPELINE_CHARS {
        return false;
    }
    let mut chars = s.chars();
    match chars.next() {
        Some('a'..='z') => {}
        _ => return false,
    }
    chars.all(|c| matches!(c, 'a'..='z' | '0'..='9' | '_' | '.' | '-'))
}

/// `^[a-z][a-z0-9_.-]{0,63}$` as a direct ASCII scan.
fn matches_check_id(s: &str) -> bool {
    if s.chars().count() > MAX_CHECK_ID_CHARS {
        return false;
    }
    let mut chars = s.chars();
    match chars.next() {
        Some('a'..='z') => {}
        _ => return false,
    }
    chars.all(|c| matches!(c, 'a'..='z' | '0'..='9' | '_' | '.' | '-'))
}

#[cfg(test)]
#[path = "platform_result_tests.rs"]
mod tests;
