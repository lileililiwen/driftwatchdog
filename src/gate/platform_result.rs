//! Machine-readable emission of a completed gate run as a
//! `platform.gate-result` document.
//!
//! This module is the only producer of that contract in the portfolio.
//! It is a **pure mapping with an exit**: given an executed run it either
//! returns a complete document that conforms to the vendored schema, or a
//! typed [`Refusal`] explaining exactly which value could not be
//! represented without losing or inventing a claim.
//!
//! The two vocabularies do not have the same shape. Driftwatchdog has five
//! [`GateStatus`] states; the platform has four values
//! (`passed`, `failed`, `errored`, `skipped`) and no human-decision state:
//!
//! | [`GateStatus`] | platform | judgement |
//! |---|---|---|
//! | `PASS` | `passed` | direct |
//! | `FAIL` | `failed` | direct; severity stays orthogonal and unmapped |
//! | `NOT_APPLICABLE` | `skipped` | the proven absence is the one `skipped` can honestly name |
//! | `REVIEW_REQUIRED` | — | **unrepresentable**: a human decision is still open and the platform has no such state |
//! | `UNBOUND` | — | **unrepresentable**: coercing it to `skipped` would merge a never-attempted check with a proven exemption, which [`GateStatus`]'s own invariant forbids |
//!
//! `errored` is never emitted. The platform defines no execution-failure
//! taxonomy for this producer, and every execution failure driftwatchdog
//! observes (timeout, spawn failure, malformed envelope, exit-code
//! contradiction) is already normalized to `REVIEW_REQUIRED`, which is
//! itself refused. A run that would need `errored` semantics arrives as a
//! refusal naming its checks, which is more useful than an invented mapping.
//!
//! Identity is the second problem. [`GateStatus`] aside, a check id comes
//! from a project manifest that validates only that it is non-empty, while
//! the platform requires `^[a-z][a-z0-9_.-]{0,63}$`. There is no lossless
//! reversible slug inside 64 characters for arbitrary ids — lowercasing
//! merges `L1-configuration` with `l1-configuration`, truncation merges
//! long ids with each other, and every encoding scheme that would need a
//! parser to be reversible is a scheme the first opaque-string consumer
//! will misread. So conforming ids pass through **unchanged** and everything
//! else is refused.
//!
//! Nothing here reads the clock, the filesystem, the database, or the
//! manifest, so a unit test and a production run take the same branch. No
//! JSON-Schema validator is used: the consumer owns validation, and a
//! second copy of the schema's rules inside the producer would mask schema
//! drift rather than surface it.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::gate::redact::redact_secrets;
use crate::gate::types::{GateResult, GateStatus};

/// The `pipeline` value this producer reports. A pipeline is a property of
/// whatever produced the document, and for a `driftwatch gate` run that is
/// always driftwatchdog's gate — never the project-owned profile, whose
/// shape no platform pattern can express. The profile is preserved verbatim
/// in the root `driftwatch_profile` additional property instead.
pub const PLATFORM_PIPELINE: &str = "driftwatchdog-gate";

/// Platform `result` / `checks[].status` vocabulary. `Errored` is part of
/// the platform contract and is deliberately never constructed; see the
/// module docs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PlatformStatus {
    Passed,
    Failed,
    Errored,
    Skipped,
}

impl PlatformStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            PlatformStatus::Passed => "passed",
            PlatformStatus::Failed => "failed",
            PlatformStatus::Errored => "errored",
            PlatformStatus::Skipped => "skipped",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "passed" => Some(PlatformStatus::Passed),
            "failed" => Some(PlatformStatus::Failed),
            "errored" => Some(PlatformStatus::Errored),
            "skipped" => Some(PlatformStatus::Skipped),
            _ => None,
        }
    }
}

/// What one driftwatchdog status means in the platform vocabulary.
///
/// This is a value, not a `Result`: the mapping is total and never fails.
/// Unrepresentability is a property of the *data*, and naming it as a
/// variant makes the table impossible to complete by accident.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusMapping {
    /// The status has an honest platform equivalent.
    Representable(PlatformStatus),
    /// The status has none. Carries the driftwatchdog wire string so the
    /// diagnostic can name the exact state rather than a paraphrase.
    Unrepresentable(&'static str),
}

/// Map one [`GateStatus`] onto the platform vocabulary.
///
/// Total by construction: every [`GateStatus`] variant has an arm, so adding
/// a sixth state is a compile error here rather than a silent emission.
pub fn map_status(status: GateStatus) -> StatusMapping {
    match status {
        GateStatus::Pass => StatusMapping::Representable(PlatformStatus::Passed),
        GateStatus::Fail => StatusMapping::Representable(PlatformStatus::Failed),
        GateStatus::NotApplicable => StatusMapping::Representable(PlatformStatus::Skipped),
        GateStatus::ReviewRequired => {
            StatusMapping::Unrepresentable(GateStatus::ReviewRequired.as_str())
        }
        GateStatus::Unbound => StatusMapping::Unrepresentable(GateStatus::Unbound.as_str()),
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
/// `driftwatch_profile` is an additional property carrying the project-owned
/// profile verbatim; the schema constrains it not at all, which is exactly
/// why the profile cannot be used as `pipeline`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlatformGateResult {
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
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum Refusal {
    #[error(
        "the aggregate status {status} has no platform.gate-result equivalent \
         (platform result is one of passed, failed, errored, skipped); {because}"
    )]
    AggregateStatus {
        status: &'static str,
        because: &'static str,
    },

    #[error("checks with status {status} have no platform.gate-result equivalent and are not coerced: {ids}")]
    CheckStatus { status: &'static str, ids: String },

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

const REVIEW_REQUIRED_BECAUSE: &str =
    "driftwatchdog's REVIEW_REQUIRED means a human decision is still open and the platform has no human-decision state";
const UNBOUND_BECAUSE: &str =
    "coercion to skipped would merge a never-attempted check with a proven exemption, which driftwatchdog forbids";

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
/// Refusals are evaluated in a fixed order — statuses, then root identity,
/// then per-check shape — so a substantive refusal is never masked by a
/// cosmetic one, and the diagnostic is identical on every run.
pub fn build(run: PlatformRun<'_>) -> Result<PlatformGateResult, Refusal> {
    // 1. Aggregate status. `REVIEW_REQUIRED` and `UNBOUND` have no platform
    //    state and are refused, never lowered.
    let result = match map_status(run.aggregate) {
        StatusMapping::Representable(s) => s,
        StatusMapping::Unrepresentable(status) => {
            return Err(Refusal::AggregateStatus {
                status,
                because: unrepresentable_because(status),
            })
        }
    };

    // 2. Per-check statuses. This is stricter than "required checks only": an
    //    optional check under review is still a check a human has not
    //    decided about, and publishing it as `skipped` would make the same
    //    false claim. Collect every offender so one diagnostic names them all.
    let mut unrepresentable: BTreeMap<&'static str, Vec<String>> = BTreeMap::new();
    let mut mapped: Vec<(PlatformStatus, Option<String>)> = Vec::with_capacity(run.results.len());
    for r in run.results {
        let status = match map_status(r.status) {
            StatusMapping::Representable(s) => s,
            StatusMapping::Unrepresentable(status) => {
                unrepresentable
                    .entry(status)
                    .or_default()
                    .push(r.gate_id.clone());
                // Reserve a slot so index alignment with `results` holds;
                // this value is never read because the loop below refuses.
                mapped.push((PlatformStatus::Passed, None));
                continue;
            }
        };
        // Redaction removes content; bounding would invent it. Only
        // `redact_secrets` is applied, and the length is then checked rather
        // than clamped.
        let detail = r
            .diagnostic
            .as_deref()
            .map(redact_secrets)
            .filter(|d| !d.is_empty());
        mapped.push((status, detail));
    }
    if let Some((status, ids)) = unrepresentable.into_iter().next() {
        return Err(Refusal::CheckStatus {
            status,
            ids: sorted_ids(ids),
        });
    }

    // 3. Root identity.
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

    // 4. Per-check shape: id, then detail, then evidence references.
    let mut checks = Vec::with_capacity(run.results.len());
    for (r, (status, detail)) in run.results.iter().zip(mapped) {
        if !matches_check_id(&r.gate_id) {
            return Err(Refusal::CheckId {
                value: r.gate_id.clone(),
                pattern: CHECK_ID_PATTERN,
                max: MAX_CHECK_ID_CHARS,
            });
        }
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
            status: status.as_str().to_string(),
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
        gate_id: run.gate_id.to_string(),
        pipeline: run.pipeline.to_string(),
        evaluated_at: run.evaluated_at.to_string(),
        result: result.as_str().to_string(),
        subject_ref: run.subject_ref.map(str::to_string),
        checks,
        evidence_refs,
        driftwatch_profile: run.profile.to_string(),
    })
}

fn unrepresentable_because(status: &'static str) -> &'static str {
    match status {
        "UNBOUND" => UNBOUND_BECAUSE,
        _ => REVIEW_REQUIRED_BECAUSE,
    }
}

fn sorted_ids(ids: Vec<String>) -> String {
    let mut ids = ids;
    ids.sort();
    ids.dedup();
    ids.join(", ")
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
