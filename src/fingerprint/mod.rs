//! Generic failure normalization and SHA-256 fingerprinting.
//!
//! The default profile is generic and language-agnostic: it accepts
//! arbitrary text and produces a deterministic canonical form. Optional
//! language-specific profiles are not implemented in v1 (the
//! `FingerprintSection.ignore` config field is advisory only).
//!
//! The fingerprint is the 64-character hex SHA-256 of the canonical text.
//! Hash choice is less important than deterministic canonicalization: the
//! same logical failure always maps to the same hash.

pub mod hash;
pub mod normalizer;

pub use hash::fingerprint;
pub use normalizer::{bounded_excerpt, summary_of, Canonical, Rule, Rules};
