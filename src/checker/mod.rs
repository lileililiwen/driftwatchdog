//! External checker protocol and execution.
//!
//! A checker is a user-configured child process that emits a JSON document
//! describing spec drift. This module owns:
//!
//! * the wire format ([`protocol`]) — what a valid checker output looks
//!   like and how it is parsed into normalized [`DriftAlert`] records;
//! * the runner ([`runner`]) — how the child process is launched, timed
//!   out, and its output bounded;
//! * a small human-readable status summary ([`report`]) shared with the
//!   `driftwatch check` command.
//!
//! The rest of the crate treats this module as a black box: it gets a
//! `CheckerSpec`, calls [`runner::run_checker`], and feeds the resulting
//! `CheckerRun` into [`protocol::parse_alerts_document`].

pub mod protocol;
pub mod report;
pub mod runner;

pub use protocol::{
    parse_alerts_document, AlertsDocument, DriftAlert, ProtocolError, MAX_ALERTS, MAX_MESSAGE_BYTES,
};
pub use report::{label_for_status, CheckerOutcome, Severity, Status};
pub use runner::{run_checker, CheckerRun, CheckerSpec};
