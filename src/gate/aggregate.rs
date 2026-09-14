//! Deterministic aggregation with an explicit blocking policy.
//!
//! Rules (in evaluation order):
//!
//! * any **required** `FAIL` blocks → aggregate `FAIL`;
//! * a **required** `NOT_APPLICABLE` never passes the gate — it is
//!   reported as missing coverage (`REVIEW_REQUIRED` when blocking,
//!   otherwise tracked explicitly without silently becoming `PASS`);
//! * a **required** `REVIEW_REQUIRED` blocks only when
//!   [`BlockingPolicy::review_required_blocks`] is set; otherwise it is
//!   still listed in `pending_reviews` so it is never silently dropped;
//! * optional results never block, but their ids are preserved in the
//!   outcome for auditability.

use crate::gate::types::{GateResult, GateStatus};
use serde::{Deserialize, Serialize};

/// One check selected for a gate run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannedCheck {
    pub gate_id: String,
    pub source: String,
    pub required: bool,
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
    pub not_applicable: Vec<String>,
}

pub fn aggregate(plan: &GatePlan, results: &[GateResult]) -> AggregateOutcome {
    let mut by_id = std::collections::BTreeMap::<&str, &GateResult>::new();
    for r in results {
        by_id.insert(r.gate_id.as_str(), r);
    }

    let mut failures: Vec<String> = Vec::new();
    let mut pending_reviews: Vec<String> = Vec::new();
    let mut not_applicable: Vec<String> = Vec::new();

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
                    if check.required {
                        // Required-but-inapplicable is missing coverage.
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
        };
    }
    if !blocking_reviews.is_empty() && plan.policy.review_required_blocks {
        return AggregateOutcome {
            status: GateStatus::ReviewRequired,
            blocked: true,
            failures,
            pending_reviews,
            not_applicable,
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
        };
    }
    AggregateOutcome {
        status: GateStatus::Pass,
        blocked: false,
        failures,
        pending_reviews,
        not_applicable,
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
        GatePlan {
            version: 1,
            checks: checks
                .into_iter()
                .map(|(gate_id, required)| PlannedCheck {
                    gate_id: gate_id.into(),
                    source: "test".into(),
                    required,
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
