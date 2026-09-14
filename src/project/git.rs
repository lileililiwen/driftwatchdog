//! Best-effort Git metadata collection.
//!
//! All failures are non-fatal: callers receive [`GitContext::missing`] or a
//! partially populated context, plus an optional diagnostic message suitable
//! for a warning. Every `git` invocation is bounded (~2s); a missing
//! binary or a hang yields `dirty: None` (unknown), never a false
//! `Some(false)` clean report. The module never blocks CLI execution
//! when Git is missing.

use std::path::Path;
use std::process::Command;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use chrono::{DateTime, Utc};

/// Per-command budget for `git` invocations.
const GIT_TIMEOUT: Duration = Duration::from_secs(2);

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
/// Git is unavailable or the directory is not a worktree. `dirty` is
/// `None` (unknown) whenever git is missing, slow, or erroring — never
/// a false clean.
pub fn capture(cwd: &Path) -> GitContext {
    let head = run(cwd, &["rev-parse", "HEAD"]);
    let branch = run(cwd, &["rev-parse", "--abbrev-ref", "HEAD"]);
    let dirty_raw = run(cwd, &["status", "--porcelain"]);

    // Only a successful `status` run proves clean/dirty; anything
    // else (missing binary, timeout, non-worktree) is unknown.
    let dirty = match dirty_raw.as_ref() {
        Some(f) if f.error.is_none() => Some(!f.value.trim().is_empty()),
        _ => None,
    };
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
    // Bounded execution: a hung git must not stall the CLI. The child
    // is waited on in a helper thread; on timeout we return `None`
    // (unknown) and let the orphaned thread reap the child.
    let cwd = cwd.to_path_buf();
    let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    let args_dbg = format!("{args:?}");
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let output = Command::new("git").args(&args).current_dir(&cwd).output();
        let _ = tx.send(output);
    });
    let output = match rx.recv_timeout(GIT_TIMEOUT) {
        Ok(o) => o,
        Err(_) => return None,
    };
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
                    format!("git {args_dbg} exited with {:?}", out.status.code())
                } else {
                    stderr
                }),
            })
        }
        Err(e) => Some(GitField {
            value: String::new(),
            error: Some(format!("git {args_dbg} failed to spawn: {e}")),
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

    #[test]
    fn dirty_is_unknown_not_clean_when_git_fails() {
        // In a non-worktree, `status` errors: dirty must be None
        // (unknown), never Some(false).
        let tmp = tempfile::tempdir().unwrap();
        let ctx = capture(tmp.path());
        assert_eq!(ctx.dirty, None);
    }

    #[test]
    fn capture_completes_within_budget() {
        // Even in the worst case (three bounded invocations) capture
        // finishes in well under 3x the per-command budget.
        let tmp = tempfile::tempdir().unwrap();
        let started = std::time::Instant::now();
        let _ = capture(tmp.path());
        assert!(
            started.elapsed() < std::time::Duration::from_secs(10),
            "git capture took too long"
        );
    }
}
