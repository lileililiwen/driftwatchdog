//! Stable Gate concern IDs.
//!
//! This module owns the data-level vocabulary of concern IDs the Gate
//! recognises as first-class. The IDs are pure data: they never embed
//! a language scanner, a provider SDK, or a Workspace-Governance type
//! in Driftwatchdog. The actual command binding is project-owned (see
//! `gate.toml` / `.ai-gate/gate.yaml`) and runs through the shared
//! project-runtime adapter, so the same command surface that backs
//! every other Gate concern is reused here.
//!
//! Two concern IDs are registered today:
//!
//! * `product-code-boundary` — a project command verifies that test
//!   code stays out of product source. A pass report means the boundary
//!   holds; a fail or review report means the project must either
//!   tighten the rule or accept review.
//! * `placeholder-threshold` — a project command enforces an explicit
//!   threshold on placeholder debt (TODO/FIXME density, missing
//!   implementations, etc.). A pass report keeps the threshold; a
//!   fail/review report names the offending surface in the findings.
//!
//! The JSON envelope emitted by the project command is the same
//! versioned shape parsed by [`crate::gate::adapters`]. Exit codes
//! carry authority: a `PASS` claim with a non-zero exit, a `FAIL`
//! claim with exit 0, or any other contradiction downgrades the
//! result to `REVIEW_REQUIRED`. Malformed output also lands on
//! `REVIEW_REQUIRED` so missing coverage is never silently treated as
//! a pass.

/// Stable concern id: enforce that test code stays out of product
/// source. Bound to a project-owned checker command.
pub const PRODUCT_CODE_BOUNDARY: &str = "product-code-boundary";

/// Stable concern id: enforce an explicit threshold on placeholder
/// debt. Bound to a project-owned checker command.
pub const PLACEHOLDER_THRESHOLD: &str = "placeholder-threshold";

/// Every product-quality concern id the Gate recognises. The list is
/// sorted so iteration and diagnostic output stay deterministic.
pub const PRODUCT_QUALITY_CONCERNS: &[&str] = &[PLACEHOLDER_THRESHOLD, PRODUCT_CODE_BOUNDARY];

/// True when `id` is a recognised product-quality concern id. The
/// check is exact (no fuzzy match): the vocabulary is closed and
/// projects that want other ids use the existing `[[checks]]` table.
pub fn is_product_quality_concern(id: &str) -> bool {
    PRODUCT_QUALITY_CONCERNS.contains(&id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn concern_ids_are_stable_strings() {
        assert_eq!(PRODUCT_CODE_BOUNDARY, "product-code-boundary");
        assert_eq!(PLACEHOLDER_THRESHOLD, "placeholder-threshold");
    }

    #[test]
    fn vocabulary_recognises_known_ids() {
        assert!(is_product_quality_concern(PRODUCT_CODE_BOUNDARY));
        assert!(is_product_quality_concern(PLACEHOLDER_THRESHOLD));
    }

    #[test]
    fn vocabulary_rejects_unknown_ids() {
        assert!(!is_product_quality_concern(""));
        assert!(!is_product_quality_concern("api-contract"));
        assert!(!is_product_quality_concern("a11y"));
        assert!(!is_product_quality_concern("PRODUCT_CODE_BOUNDARY"));
        assert!(!is_product_quality_concern("product-code-boundary "));
    }

    #[test]
    fn vocabulary_is_sorted_for_deterministic_iteration() {
        let mut sorted = PRODUCT_QUALITY_CONCERNS.to_vec();
        sorted.sort();
        assert_eq!(sorted, PRODUCT_QUALITY_CONCERNS.to_vec());
    }
}
