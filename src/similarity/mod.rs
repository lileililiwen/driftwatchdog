//! Heuristic similarity engine for the `correlation-and-ai-context`
//! change.
//!
//! Inputs are a `BugInput` (fingerprint canonical + summary + tags) and
//! an `AlertInput` (alert message + symbol + source). The engine
//! produces a [`score::ComponentScores`] record with per-component
//! values and a deterministic weighted total. The total is the basis
//! for the "possible relationship" threshold defined in
//! [`score::THRESHOLD`].
//!
//! The engine never embeds anything, never calls the network, and
//! makes no language-specific assumptions. It is intentionally simple
//! so the scores can be reproduced from the same inputs and reviewed
//! by humans.

pub mod candidates;
pub mod score;
pub mod tokenize;
