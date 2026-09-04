//! Best-effort Git metadata collection.
//!
//! All failures are non-fatal: callers receive [`GitContext::missing`] or a
//! partially populated context, plus an optional diagnostic message suitable
//! for a warning. The module never blocks CLI execution when Git is missing.

use std::path::Path;
use std::process::Command;

use chrono::{DateTime, Utc};

/// Git metadata captured at a point in time. Any field may be `None`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GitContext {
    pub commit: Option<String>,
    pub branch: Option<String>,
    pub dirty: Option<bool>,
    /// Human-readable explanation when a field is unavailable.
    pub diagnostic: Option<String>,
}

impl GitContext {
    pub fn missing(diagnostic: impl Into<String>) -> Self {
        Self {
            commit: None,
            branch: None,
            dirty: None,
            diagnostic: Some(diagnostic.into()),
        }
    }
}

/// Capture the current Git state for `cwd`. Returns a [`GitContext`] with
/// whatever information could be obtained, or [`GitContext::missing`] when
/// Git is unavailable or the directory is not a worktree.
pub fn capture(cwd: &Path) -> GitContext {
    let head = run(cwd, &["rev-parse", "HEAD"]);
    let branch = run(cwd, &["rev-parse", "--abbrev-ref", "HEAD"]);
    let dirty_raw = run(cwd, &["status", "--porcelain"]);

    let dirty = dirty_raw.as_ref().map(|f| !f.value.trim().is_empty());
    let diagnostic = build_diagnostic(&head, &branch, &dirty_raw);

    if head.is_none() && branch.is_none() && dirty_raw.is_none() {
        return GitContext::missing(diagnostic.unwrap_or_else(|| "git unavailable".into()));
    }

    GitContext {
        commit: head.and_then(|f| nonempty(f.value)),
        branch: branch.and_then(|f| nonempty(f.value)),
        dirty,
        diagnostic,
    }
}

fn nonempty(s: String) -> Option<String> {
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

/// Marker for the time at which Git metadata was captured. Recorded alongside
/// run rows; the wall-clock is sufficient for human/AI reports.
pub fn now() -> DateTime<Utc> {
    Utc::now()
}

#[derive(Debug)]
struct GitField {
    value: String,
    error: Option<String>,
}

fn run(cwd: &Path, args: &[&str]) -> Option<GitField> {
    let output = Command::new("git").args(args).current_dir(cwd).output();
    match output {
        Ok(out) if out.status.success() => {
            let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
            Some(GitField {
                value: text,
                error: None,
            })
        }
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
            Some(GitField {
                value: String::new(),
                error: Some(if stderr.is_empty() {
                    format!("git {:?} exited with {:?}", args, out.status.code())
                } else {
                    stderr
                }),
            })
        }
        Err(e) => Some(GitField {
            value: String::new(),
            error: Some(format!("git {:?} failed to spawn: {}", args, e)),
        }),
    }
}

fn build_diagnostic(
    head: &Option<GitField>,
    branch: &Option<GitField>,
    dirty: &Option<GitField>,
) -> Option<String> {
    let mut parts: Vec<String> = Vec::new();
    for (label, field) in [("commit", head), ("branch", branch), ("status", dirty)] {
        if let Some(f) = field {
            if let Some(err) = &f.error {
                parts.push(format!("{}: {}", label, err));
            }
        }
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("; "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_context_carries_diagnostic() {
        let ctx = GitContext::missing("git not installed");
        assert!(ctx.commit.is_none());
        assert_eq!(ctx.diagnostic.as_deref(), Some("git not installed"));
    }

    #[test]
    fn capture_in_non_git_dir_yields_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let ctx = capture(tmp.path());
        // Non-Git dir produces no usable values; diagnostic is present.
        assert!(ctx.commit.is_none());
        assert!(ctx.diagnostic.is_some());
    }
}
