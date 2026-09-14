//! Doctor check primitives.
//!
//! Each check produces a [`Check`] with a stable `id`, a human-readable
//! `name`, a [`Status`], an optional `detail`, and an optional
//! `remediation` hint. Checks must never panic on bad input; they
//! always produce a result, even if that result is `Fail`.

use std::fmt;

/// Result of a single diagnostic check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    /// Required check passed.
    Pass,
    /// Unconfigured-but-ok state (e.g. no checkers yet). Informational:
    /// never fails the run.
    Info,
    /// Optional check did not pass but does not block the user.
    Warn,
    /// Required check failed; the user must address it.
    Fail,
}

impl fmt::Display for Status {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Status::Pass => write!(f, "PASS"),
            Status::Info => write!(f, "INFO"),
            Status::Warn => write!(f, "WARN"),
            Status::Fail => write!(f, "FAIL"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Check {
    /// Stable identifier (e.g. `"db.open"`). Used by automation that
    /// wants to assert on a specific check without parsing prose.
    pub id: &'static str,
    /// Human-readable name shown to the user. May include the target
    /// (e.g. `Checker "foo" is not on PATH`).
    pub name: String,
    pub status: Status,
    /// One-line detail; safe to print. Never include secrets here.
    pub detail: Option<String>,
    /// Actionable next step the user can take. Optional even for
    /// failures, since some failures are self-explanatory.
    pub remediation: Option<String>,
}

impl Check {
    pub fn pass(id: &'static str, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            status: Status::Pass,
            detail: None,
            remediation: None,
        }
    }

    pub fn info(id: &'static str, name: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            status: Status::Info,
            detail: Some(detail.into()),
            remediation: None,
        }
    }

    pub fn warn(id: &'static str, name: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            status: Status::Warn,
            detail: Some(detail.into()),
            remediation: None,
        }
    }

    pub fn fail(id: &'static str, name: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            status: Status::Fail,
            detail: Some(detail.into()),
            remediation: None,
        }
    }

    pub fn with_remediation(mut self, text: impl Into<String>) -> Self {
        self.remediation = Some(text.into());
        self
    }
}
