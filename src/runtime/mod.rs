//! Child-process runtime: cross-platform command runner with bounded
//! stream capture. Public types are re-exported for use by command
//! orchestration.

pub mod runner;

pub use runner::{run, CaptureLimits, CapturedStream, CommandSpec, RunOutcome};
