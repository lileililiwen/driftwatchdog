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
//! Seven concern IDs are registered today, organised in three vocabularies:
//!
//! ## Product-quality vocabulary
//!
//! * `product-code-boundary` — a project command verifies that test
//!   code stays out of product source. A pass report means the boundary
//!   holds; a fail or review report means the project must either
//!   tighten the rule or accept review.
//! * `placeholder-threshold` — a project command enforces an explicit
//!   threshold on placeholder debt (TODO/FIXME density, missing
//!   implementations, etc.). A pass report keeps the threshold; a
//!   fail/review report names the offending surface in the findings.
//! * `source-file-size` — a **built-in** concern (no project command):
//!   Driftwatchdog discovers repository-owned source files, counts raw
//!   newline bytes like `wc -l`, and fails a required Gate when an
//!   included file exceeds the configured (default 1,000) physical-line
//!   maximum. The scanner lives in [`crate::gate::source_size`] and
//!   never parses a language.
//!
//! ## Release-gate vocabulary
//!
//! * `capability-conformance` — a project command reports declared,
//!   configured, verified, and unverified capabilities. A pass report
//!   means every declared capability is verified by a concrete check
//!   in the resolved plan; a fail or review report names the
//!   unverified capability. Driftwatchdog does not embed a capability
//!   scanner; it normalises the project's own report.
//! * `release-evidence` — a project command reports source revision,
//!   release version, artifacts, integrity, SBOM, provenance, and
//!   publication state. A pass report means the evidence is current
//!   and complete; a fail or review report names the missing or
//!   stale evidence. Driftwatchdog does not become a release
//!   publisher, signer, SBOM generator, or deployment executor.
//!
//! ## Deployable-project vocabulary
//!
//! * `compose-contract` — a project-owned command validates the repository's
//!   Compose contract.
//! * `ci-contract` — a project-owned command validates the repeatable local CI
//!   contract that remote CI invokes.
//!
//! ## Wire format
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

/// Stable concern id: enforce a per-file source-size boundary. This is
/// a **built-in** concern executed by [`crate::gate::source_size`]; it
/// does not require a project command and counts raw newline bytes,
/// never parsing a language.
pub const SOURCE_FILE_SIZE: &str = "source-file-size";

/// Every product-quality concern id the Gate recognises. The list is
/// sorted so iteration and diagnostic output stay deterministic.
pub const PRODUCT_QUALITY_CONCERNS: &[&str] = &[
    PLACEHOLDER_THRESHOLD,
    PRODUCT_CODE_BOUNDARY,
    SOURCE_FILE_SIZE,
];

/// True when `id` is a recognised product-quality concern id. The
/// check is exact (no fuzzy match): the vocabulary is closed and
/// projects that want other ids use the existing `[[checks]]` table.
pub fn is_product_quality_concern(id: &str) -> bool {
    PRODUCT_QUALITY_CONCERNS.contains(&id)
}

/// True when `id` is the built-in `source-file-size` concern id. The
/// scanner is dispatched in-process; it never consumes a project
/// command.
pub fn is_source_file_size_concern(id: &str) -> bool {
    id == SOURCE_FILE_SIZE
}

/// Stable concern id: verify a project-owned capability-conformance
/// report. The project command emits a versioned JSON envelope that
/// lists declared, configured, verified, and unverified capabilities;
/// Driftwatchdog executes the command and normalises the result. It
/// never inspects Cargo, NuGet, npm, Docker, or Jenkins internals.
pub const CAPABILITY_CONFORMANCE: &str = "capability-conformance";

/// Stable concern id: verify a project-owned release-evidence report.
/// The project command emits a versioned JSON envelope that records
/// source revision, release version, artifacts, integrity, SBOM,
/// provenance, and publication state; Driftwatchdog executes the
/// command, applies exit-code authority, and refuses a stale revision
/// or a missing required-evidence claim. It does not become a release
/// publisher, signer, SBOM generator, or deployment executor.
pub const RELEASE_EVIDENCE: &str = "release-evidence";

/// Every release-gate concern id the Gate recognises. The list is
/// sorted so iteration and diagnostic output stay deterministic.
pub const RELEASE_GATE_CONCERNS: &[&str] = &[CAPABILITY_CONFORMANCE, RELEASE_EVIDENCE];

/// True when `id` is a recognised release-gate concern id.
pub fn is_release_gate_concern(id: &str) -> bool {
    RELEASE_GATE_CONCERNS.contains(&id)
}

/// True when `id` is the `release-evidence` concern id.
pub fn is_release_evidence_concern(id: &str) -> bool {
    id == RELEASE_EVIDENCE
}

/// True when `id` is the `capability-conformance` concern id.
pub fn is_capability_conformance_concern(id: &str) -> bool {
    id == CAPABILITY_CONFORMANCE
}

/// Stable concern id: validate a deployable project's Compose contract.
pub const COMPOSE_CONTRACT: &str = "compose-contract";

/// Stable concern id: validate a deployable project's local CI contract.
pub const CI_CONTRACT: &str = "ci-contract";

/// Every deployable-project concern id, sorted for deterministic plans.
pub const DEPLOYABLE_CONCERNS: &[&str] = &[CI_CONTRACT, COMPOSE_CONTRACT];

/// True when `id` is a deployable-project concern.
pub fn is_deployable_concern(id: &str) -> bool {
    DEPLOYABLE_CONCERNS.contains(&id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn concern_ids_are_stable_strings() {
        assert_eq!(PRODUCT_CODE_BOUNDARY, "product-code-boundary");
        assert_eq!(PLACEHOLDER_THRESHOLD, "placeholder-threshold");
        assert_eq!(SOURCE_FILE_SIZE, "source-file-size");
        assert_eq!(CAPABILITY_CONFORMANCE, "capability-conformance");
        assert_eq!(RELEASE_EVIDENCE, "release-evidence");
        assert_eq!(COMPOSE_CONTRACT, "compose-contract");
        assert_eq!(CI_CONTRACT, "ci-contract");
    }

    #[test]
    fn vocabulary_recognises_known_ids() {
        assert!(is_product_quality_concern(PRODUCT_CODE_BOUNDARY));
        assert!(is_product_quality_concern(PLACEHOLDER_THRESHOLD));
        assert!(is_product_quality_concern(SOURCE_FILE_SIZE));
        assert!(is_source_file_size_concern(SOURCE_FILE_SIZE));
        assert!(is_release_gate_concern(CAPABILITY_CONFORMANCE));
        assert!(is_release_gate_concern(RELEASE_EVIDENCE));
        assert!(is_release_evidence_concern(RELEASE_EVIDENCE));
        assert!(is_capability_conformance_concern(CAPABILITY_CONFORMANCE));
        assert!(is_deployable_concern(COMPOSE_CONTRACT));
        assert!(is_deployable_concern(CI_CONTRACT));
    }

    #[test]
    fn vocabulary_rejects_unknown_ids() {
        assert!(!is_product_quality_concern(""));
        assert!(!is_product_quality_concern("api-contract"));
        assert!(!is_product_quality_concern("a11y"));
        assert!(!is_product_quality_concern("PRODUCT_CODE_BOUNDARY"));
        assert!(!is_product_quality_concern("product-code-boundary "));
        assert!(!is_source_file_size_concern(PRODUCT_CODE_BOUNDARY));
        assert!(!is_source_file_size_concern("SOURCE_FILE_SIZE"));
        assert!(!is_release_gate_concern(""));
        assert!(!is_release_gate_concern("api-contract"));
        assert!(!is_release_gate_concern("RELEASE_EVIDENCE"));
        // Cross-vocabulary recognition stays negative: a product-quality
        // id is not a release-gate id and vice versa.
        assert!(!is_release_gate_concern(PRODUCT_CODE_BOUNDARY));
        assert!(!is_release_gate_concern(PLACEHOLDER_THRESHOLD));
        assert!(!is_product_quality_concern(CAPABILITY_CONFORMANCE));
        assert!(!is_product_quality_concern(RELEASE_EVIDENCE));
        assert!(!is_release_evidence_concern(CAPABILITY_CONFORMANCE));
        assert!(!is_capability_conformance_concern(RELEASE_EVIDENCE));
        assert!(!is_deployable_concern("docker"));
    }

    #[test]
    fn vocabulary_is_sorted_for_deterministic_iteration() {
        for (label, slice) in [
            ("product-quality", PRODUCT_QUALITY_CONCERNS),
            ("release-gate", RELEASE_GATE_CONCERNS),
            ("deployable", DEPLOYABLE_CONCERNS),
        ] {
            let mut sorted = slice.to_vec();
            sorted.sort();
            assert_eq!(sorted, slice.to_vec(), "{label} slice not sorted");
        }
    }
}
