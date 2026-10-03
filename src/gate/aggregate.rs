//! Deterministic aggregation with an explicit blocking policy.
//!
//! Rules (in evaluation order):
//!
//! * any **required** `FAIL` blocks → aggregate `FAIL`;
//! * a **required** `UNBOUND` blocks → aggregate `FAIL`, independently
//!   of the project-declared blocking list. A required check that bound
//!   no command and declared no exemption is a never-attempted absence,
//!   not a proven exemption, and must not be reducible to a `PASS` by
//!   editing `blocking:`;
//! * a **required** `NOT_APPLICABLE` never passes the gate — it is
//!   reported as missing coverage (`REVIEW_REQUIRED` when blocking,
//!   otherwise tracked explicitly without silently becoming `PASS`) —
//!   unless the manifest declared an explicit, reasoned exemption for
//!   that check, in which case the exemption is acknowledged on the
//!   record and does not block;
//! * a **required** `REVIEW_REQUIRED` blocks only when
//!   [`BlockingPolicy::review_required_blocks`] is set; otherwise it is
//!   still listed in `pending_reviews` so it is never silently dropped;
//! * optional results never block, but their ids are preserved in the
//!   outcome for auditability.
//!
//! The plan-level `exemption-budget` result is aggregated by these same
//! rules and gets no special case: a failing budget arrives as a required
//! `FAIL` and blocks, an exhausted budget arrives as an optional
//! `REVIEW_REQUIRED` and is listed without blocking.

use crate::gate::types::{GateResult, GateStatus};
use serde::{Deserialize, Serialize};

/// One check selected for a gate run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannedCheck {
    pub gate_id: String,
    pub source: String,
    pub required: bool,
    /// The manifest declared a written `not_applicable` exemption for
    /// this check. An exemption is an *acknowledged* absence: it is
    /// still reported, but it does not become pending coverage. The
    /// runtime's command-free path must not report an exempted check as
    /// `UNBOUND`, and the exemption is validated at parse time.
    #[serde(default)]
    pub exempt: bool,
}

/// Blocking policy resolved before execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockingPolicy {
    pub review_required_blocks: bool,
}

impl Default for BlockingPolicy {
    fn default() -> Self {
        Self {
            review_required_blocks: true,
        }
    }
}

/// Plan resolved before execution: selected checks plus policy identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GatePlan {
    pub version: u32,
    pub checks: Vec<PlannedCheck>,
    pub policy: BlockingPolicy,
}

/// Deterministic aggregate outcome. `blocked` is the machine signal;
/// `status` is the human summary. A non-blocking review still yields
/// `status == PASS` with `blocked == false` but keeps the gate id in
/// `pending_reviews`, so `REVIEW_REQUIRED` is never silently converted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AggregateOutcome {
    pub status: GateStatus,
    pub blocked: bool,
    pub failures: Vec<String>,
    pub pending_reviews: Vec<String>,
    /// Checks proven inapplicable, or explicitly exempted by the
    /// manifest with a written reason.
    pub not_applicable: Vec<String>,
    /// Checks that asserted coverage and bound no command. Kept apart
    /// from `not_applicable` on purpose: this is the never-attempted
    /// absence, that is the proven or acknowledged one. Optional
    /// entries carry the `optional:` marker.
    #[serde(default)]
    pub unbound: Vec<String>,
}

pub fn aggregate(plan: &GatePlan, results: &[GateResult]) -> AggregateOutcome {
    let mut by_id = std::collections::BTreeMap::<&str, &GateResult>::new();
    for r in results {
        by_id.insert(r.gate_id.as_str(), r);
    }

    let mut failures: Vec<String> = Vec::new();
    let mut pending_reviews: Vec<String> = Vec::new();
    let mut not_applicable: Vec<String> = Vec::new();
    let mut unbound: Vec<String> = Vec::new();

    for check in &plan.checks {
        match by_id.get(check.gate_id.as_str()) {
            None => {
                // No result at all for a planned check: required checks
                // become pending reviews, optional checks are ignored.
                if check.required {
                    pending_reviews.push(check.gate_id.clone());
                }
            }
            Some(r) => match r.status {
                GateStatus::Fail => {
                    if check.required {
                        failures.push(check.gate_id.clone());
                    }
                }
                GateStatus::Unbound => {
                    // A required unbound check is a failing result, not a
                    // review: the project cannot make it pass by removing
                    // REVIEW_REQUIRED from its blocking list. It is also
                    // collected in `unbound` so coverage debt stays
                    // queryable even when the outcome is a FAIL.
                    if check.required {
                        failures.push(check.gate_id.clone());
                        unbound.push(check.gate_id.clone());
                    } else {
                        unbound.push(format!("optional:{}", check.gate_id));
                    }
                }
                GateStatus::ReviewRequired => {
                    if check.required {
                        pending_reviews.push(check.gate_id.clone());
                    } else {
                        // Optional reviews are still tracked for callers
                        // that want them, but they never block.
                        pending_reviews.push(format!("optional:{}", check.gate_id));
                    }
                }
                GateStatus::NotApplicable => {
                    not_applicable.push(check.gate_id.clone());
                    if check.required && !check.exempt {
                        // Required-but-inapplicable is missing coverage.
                        // A declared exemption is the one case where the
                        // absence is acknowledged on the record instead.
                        pending_reviews.push(check.gate_id.clone());
                    }
                }
                GateStatus::Pass => {}
            },
        }
    }

    failures.sort();
    failures.dedup();
    pending_reviews.sort();
    pending_reviews.dedup();
    not_applicable.sort();
    not_applicable.dedup();
    unbound.sort();
    unbound.dedup();

    // Strip the `optional:` marker for deterministic output ordering
    // but keep the underlying gate id visible. Optional entries never
    // affect `blocked` or `status`.
    let blocking_reviews: Vec<String> = pending_reviews
        .iter()
        .filter(|id| !id.starts_with("optional:"))
        .cloned()
        .collect();

    if !failures.is_empty() {
        return AggregateOutcome {
            status: GateStatus::Fail,
            blocked: true,
            failures,
            pending_reviews,
            not_applicable,
            unbound,
        };
    }
    if !blocking_reviews.is_empty() && plan.policy.review_required_blocks {
        return AggregateOutcome {
            status: GateStatus::ReviewRequired,
            blocked: true,
            failures,
            pending_reviews,
            not_applicable,
            unbound,
        };
    }
    if !blocking_reviews.is_empty() {
        // Fail-closed is off: PASS but reviews stay explicit.
        return AggregateOutcome {
            status: GateStatus::Pass,
            blocked: false,
            failures,
            pending_reviews,
            not_applicable,
            unbound,
        };
    }
    AggregateOutcome {
        status: GateStatus::Pass,
        blocked: false,
        failures,
        pending_reviews,
        not_applicable,
        unbound,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gate::types::{GateResult, GateSeverity, GateStatus};

    fn result(id: &str, status: GateStatus) -> GateResult {
        GateResult {
            gate_id: id.into(),
            source: "test".into(),
            status,
            severity: GateSeverity::Warning,
            findings: vec![],
            evidence: vec![],
            missing_evidence: vec![],
            diagnostic: None,
            remediation: None,
        }
    }

    fn plan(checks: Vec<(&str, bool)>, review_blocks: bool) -> GatePlan {
        plan_ex(checks, review_blocks, &[])
    }

    fn plan_ex(checks: Vec<(&str, bool)>, review_blocks: bool, exempt: &[&str]) -> GatePlan {
        GatePlan {
            version: 1,
            checks: checks
                .into_iter()
                .map(|(gate_id, required)| PlannedCheck {
                    gate_id: gate_id.into(),
                    source: "test".into(),
                    required,
                    exempt: exempt.contains(&gate_id),
                })
                .collect(),
            policy: BlockingPolicy {
                review_required_blocks: review_blocks,
            },
        }
    }

    #[test]
    fn blocking_failure_wins() {
        let p = plan(vec![("a", true), ("b", true)], true);
        let out = aggregate(
            &p,
            &[result("a", GateStatus::Pass), result("b", GateStatus::Fail)],
        );
        assert_eq!(out.status, GateStatus::Fail);
        assert!(out.blocked);
        assert_eq!(out.failures, vec!["b".to_string()]);
    }

    #[test]
    fn blocking_review_blocks_when_configured() {
        let p = plan(vec![("a", true)], true);
        let out = aggregate(&p, &[result("a", GateStatus::ReviewRequired)]);
        assert_eq!(out.status, GateStatus::ReviewRequired);
        assert!(out.blocked);
    }

    #[test]
    fn non_blocking_review_stays_explicit_pass() {
        let p = plan(vec![("a", true)], false);
        let out = aggregate(&p, &[result("a", GateStatus::ReviewRequired)]);
        assert_eq!(out.status, GateStatus::Pass);
        assert!(!out.blocked);
        assert_eq!(out.pending_reviews, vec!["a".to_string()]);
    }

    #[test]
    fn required_not_applicable_does_not_pass() {
        let p = plan(vec![("a", true)], true);
        let out = aggregate(&p, &[result("a", GateStatus::NotApplicable)]);
        assert_ne!(out.status, GateStatus::Pass);
        assert!(out.blocked);
        assert!(out.not_applicable.contains(&"a".to_string()));
    }

    #[test]
    fn optional_not_applicable_passes_with_record() {
        let p = plan(vec![("req", true), ("opt", false)], true);
        let out = aggregate(
            &p,
            &[
                result("req", GateStatus::Pass),
                result("opt", GateStatus::NotApplicable),
            ],
        );
        assert_eq!(out.status, GateStatus::Pass);
        assert!(!out.blocked);
        assert_eq!(out.not_applicable, vec!["opt".to_string()]);
    }

    /// Inverts `characterisation_required_unbound_blocks_via_review_policy`
    /// (task 1.2): a required `UNBOUND` is a failure, not a review.
    #[test]
    fn required_unbound_blocks_via_finding_not_review_policy() {
        let p = plan(vec![("a", true)], true);
        let out = aggregate(&p, &[result("a", GateStatus::Unbound)]);
        assert_eq!(out.status, GateStatus::Fail);
        assert!(out.blocked);
        assert_eq!(out.failures, vec!["a".to_string()]);
        assert_eq!(out.unbound, vec!["a".to_string()]);
        assert!(out.pending_reviews.is_empty());
    }

    /// Inverts `characterisation_required_unbound_fail_open_without_review_required`
    /// (task 1.2): the project's `blocking:` list can no longer make a
    /// never-attempted required check pass.
    #[test]
    fn required_unbound_blocks_even_when_review_required_does_not() {
        let p = plan(vec![("a", true)], false);
        let out = aggregate(&p, &[result("a", GateStatus::Unbound)]);
        assert_eq!(out.status, GateStatus::Fail);
        assert!(out.blocked);
        assert_eq!(out.failures, vec!["a".to_string()]);
        assert_eq!(out.unbound, vec!["a".to_string()]);
    }

    #[test]
    fn optional_unbound_never_blocks_and_is_kept_separate() {
        let p = plan(vec![("req", true), ("opt", false)], true);
        let out = aggregate(
            &p,
            &[
                result("req", GateStatus::Pass),
                result("opt", GateStatus::Unbound),
            ],
        );
        assert_eq!(out.status, GateStatus::Pass);
        assert!(!out.blocked);
        assert!(out.unbound.contains(&"optional:opt".to_string()));
        assert!(out.not_applicable.is_empty());
    }

    #[test]
    fn unbound_and_not_applicable_stay_in_separate_collections() {
        let p = plan_ex(vec![("proven", true), ("never", true)], true, &["proven"]);
        let out = aggregate(
            &p,
            &[
                result("proven", GateStatus::NotApplicable),
                result("never", GateStatus::Unbound),
            ],
        );
        assert_eq!(out.not_applicable, vec!["proven".to_string()]);
        assert_eq!(out.unbound, vec!["never".to_string()]);
        assert_eq!(out.status, GateStatus::Fail);
        assert_eq!(out.failures, vec!["never".to_string()]);
    }

    /// A declared exemption is the acknowledged counterpart of a
    /// proven exemption: still reported, but not missing coverage.
    #[test]
    fn exempt_required_check_does_not_block_and_stays_recorded() {
        let p = plan_ex(vec![("proven", true), ("exempt", true)], true, &["exempt"]);
        let out = aggregate(
            &p,
            &[
                result("proven", GateStatus::NotApplicable),
                result("exempt", GateStatus::NotApplicable),
            ],
        );
        assert_eq!(out.not_applicable, vec!["exempt", "proven"]);
        assert!(out.unbound.is_empty());
        assert_eq!(out.pending_reviews, vec!["proven".to_string()]);
        assert_eq!(out.status, GateStatus::ReviewRequired);
    }

    /// Fail-closed: a required check the manifest both exempts and
    /// leaves unbound still fails, so an exemption can never be used
    /// to launder a `UNBOUND` verdict.
    #[test]
    fn required_unbound_blocks_even_when_the_plan_marks_it_exempt() {
        let p = plan_ex(vec![("a", true)], true, &["a"]);
        let out = aggregate(&p, &[result("a", GateStatus::Unbound)]);
        assert_eq!(out.status, GateStatus::Fail);
        assert_eq!(out.unbound, vec!["a".to_string()]);
    }

    #[test]
    fn optional_fail_does_not_block() {
        let p = plan(vec![("req", true), ("opt", false)], true);
        let out = aggregate(
            &p,
            &[
                result("req", GateStatus::Pass),
                result("opt", GateStatus::Fail),
            ],
        );
        assert_eq!(out.status, GateStatus::Pass);
        assert!(!out.blocked);
        assert!(out.failures.is_empty());
    }
}
