//! `driftwatch check` orchestration.
//!
//! For each configured checker in declaration order, this command:
//!
//! 1. Spawns the checker subprocess with the configured working dir,
//!    environment, timeout, and output cap.
//! 2. Parses the stdout as the alerts JSON document (see
//!    [`crate::checker::protocol`]).
//! 3. Persists one `check_snapshots` row plus zero or more
//!    `drift_alerts` rows, transactionally.
//! 4. Continues to the next checker, regardless of any failure on
//!    this one (the "isolate checker failures" requirement).
//!
//! The exit code reflects the aggregate outcome:
//! * 0 — every checker produced a non-failure status (`success` or
//!   `empty`), or `--dry-run` is set.
//! * 1 — at least one checker produced a failure status but at least
//!   one also produced a result.
//! * 2 — every configured checker produced a failure status (mirrors
//!   `driftwatch doctor`'s strict-fail exit code).

use std::path::Path;

use crate::checker::{
    parse_alerts_document, run_checker, CheckerOutcome, CheckerSpec, ProtocolError, Status,
};
use crate::cli::CheckArgs;
use crate::error::Error;
use crate::project::{config::Config, git, ProjectRoot};
use crate::repo::alerts::{NewAlert, NewSnapshot};
use crate::util::truncate_char_boundary;

const EXIT_OK: i32 = 0;
const EXIT_PARTIAL: i32 = 1;
const EXIT_ALL_FAILED: i32 = 2;

pub fn check(args: CheckArgs, cwd: &Path) -> Result<i32, Error> {
    let proj = ProjectRoot::discover(cwd)?;
    let mut db = crate::repo::Db::open(&proj.db_path)?;
    let cfg = Config::load(&proj.config_path)?;
    cfg.validate_working_dirs(&proj.config_path, &proj.root)?;

    if cfg.checkers.is_empty() {
        println!("driftwatch check: no checkers configured in driftwatch.toml.");
        return Ok(EXIT_OK);
    }

    // Select the checkers to run. `--only` may be a name or a list of
    // comma-separated names; declaration order is preserved.
    let selected: Vec<&crate::project::config::CheckerEntry> = if args.only.is_empty() {
        cfg.checkers.iter().collect()
    } else {
        let wanted: Vec<String> = args.only.clone();
        let mut out = Vec::new();
        for entry in &cfg.checkers {
            if wanted.iter().any(|w| w == &entry.name) {
                out.push(entry);
            }
        }
        // Reject unknown names so the user gets a clear error rather
        // than a silent no-op.
        for w in &wanted {
            if !cfg.checkers.iter().any(|c| &c.name == w) {
                return Err(Error::Io {
                    path: proj.config_path.clone(),
                    source: std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        format!("unknown checker `{w}` in --only"),
                    ),
                });
            }
        }
        out
    };

    let git_ctx = git::capture(&proj.root);
    let taken_at = chrono::Utc::now().to_rfc3339();

    let mut outcomes: Vec<CheckerOutcome> = Vec::with_capacity(selected.len());
    for entry in &selected {
        let spec = CheckerSpec::from_entry(entry, &proj.root)?;
        let outcome = run_one_checker(
            &mut db,
            &spec,
            &taken_at,
            git_ctx.commit.as_deref(),
            git_ctx.branch.as_deref(),
            args.dry_run,
        )?;
        outcomes.push(outcome);
    }

    print_summary(&outcomes, args.dry_run);

    let had_failure = outcomes.iter().any(|o| o.status.is_failure());
    let all_failed = !outcomes.is_empty() && outcomes.iter().all(|o| o.status.is_failure());

    // Heuristic correlation runs after every successful check so
    // subsequent `report --ai` invocations can read the persisted
    // rows without re-scoring. A correlation failure is a warning,
    // not a hard error: it must not abort the check (the
    // "isolate checker failures" rule applies to correlation too).
    if !args.dry_run && !all_failed {
        if let Err(e) = crate::correlate::run_after_check(&mut db) {
            eprintln!("driftwatch: correlation skipped: {e}");
        }
    }

    if args.dry_run {
        return Ok(EXIT_OK);
    }
    if all_failed {
        Ok(EXIT_ALL_FAILED)
    } else if had_failure {
        Ok(EXIT_PARTIAL)
    } else {
        Ok(EXIT_OK)
    }
}

fn run_one_checker(
    db: &mut crate::repo::Db,
    spec: &CheckerSpec,
    taken_at: &str,
    git_commit: Option<&str>,
    git_branch: Option<&str>,
    dry_run: bool,
) -> Result<CheckerOutcome, Error> {
    let run = run_checker(spec);

    // Branch 1: the child could not be spawned at all.
    if let Some(spawn_err) = run.spawn_error {
        let diagnostic = format!("could not start checker: {spawn_err}");
        if !dry_run {
            crate::repo::alerts::Alerts::record_run(
                db,
                &NewSnapshot {
                    taken_at,
                    checker_name: &spec.name,
                    status: Status::StartFailed.as_str(),
                    diagnostic: Some(&diagnostic),
                    raw_json: None,
                    git_commit,
                    git_branch,
                },
                &[],
            )?;
        }
        return Ok(CheckerOutcome {
            name: spec.name.clone(),
            status: Status::StartFailed,
            alert_count: 0,
            diagnostic: Some(diagnostic),
            alerts: vec![],
            duration_ms: run.duration_ms,
        });
    }

    // Branch 2: the child timed out.
    if run.timed_out {
        let diagnostic = format!(
            "checker exceeded timeout of {} ms",
            spec.timeout.as_millis()
        );
        if !dry_run {
            crate::repo::alerts::Alerts::record_run(
                db,
                &NewSnapshot {
                    taken_at,
                    checker_name: &spec.name,
                    status: Status::Timeout.as_str(),
                    diagnostic: Some(&diagnostic),
                    raw_json: Some(run.stdout.as_str()),
                    git_commit,
                    git_branch,
                },
                &[],
            )?;
        }
        return Ok(CheckerOutcome {
            name: spec.name.clone(),
            status: Status::Timeout,
            alert_count: 0,
            diagnostic: Some(diagnostic),
            alerts: vec![],
            duration_ms: run.duration_ms,
        });
    }

    // Branch 3: signal-killed (exit code None, not a timeout, spawned OK).
    // Must precede the nonzero-exit branch so kills are never Failed.
    if run.signalled {
        let mut diagnostic = "checker killed by signal (exit code unavailable)".to_string();
        if let Some(cap) = run.capture_error.as_deref() {
            if !cap.is_empty() {
                diagnostic.push_str("; ");
                diagnostic.push_str(&truncate_diagnostic(cap));
            }
        }
        if !dry_run {
            crate::repo::alerts::Alerts::record_run(
                db,
                &NewSnapshot {
                    taken_at,
                    checker_name: &spec.name,
                    status: Status::Unknown.as_str(),
                    diagnostic: Some(&diagnostic),
                    raw_json: Some(run.stdout.as_str()),
                    git_commit,
                    git_branch,
                },
                &[],
            )?;
        }
        return Ok(CheckerOutcome {
            name: spec.name.clone(),
            status: Status::Unknown,
            alert_count: 0,
            diagnostic: Some(diagnostic),
            alerts: vec![],
            duration_ms: run.duration_ms,
        });
    }

    // Branch 4: the child exited nonzero.
    if !matches!(run.exit_code, Some(0)) {
        let mut diagnostic = format!(
            "checker exited with status {:?}; stderr: {}",
            run.exit_code,
            truncate_diagnostic(&run.stderr)
        );
        if let Some(cap) = run.capture_error.as_deref() {
            if !cap.is_empty() {
                diagnostic.push_str("; capture: ");
                diagnostic.push_str(&truncate_diagnostic(cap));
            }
        }
        if !dry_run {
            crate::repo::alerts::Alerts::record_run(
                db,
                &NewSnapshot {
                    taken_at,
                    checker_name: &spec.name,
                    status: Status::Failed.as_str(),
                    diagnostic: Some(&diagnostic),
                    raw_json: Some(run.stdout.as_str()),
                    git_commit,
                    git_branch,
                },
                &[],
            )?;
        }
        return Ok(CheckerOutcome {
            name: spec.name.clone(),
            status: Status::Failed,
            alert_count: 0,
            diagnostic: Some(diagnostic),
            alerts: vec![],
            duration_ms: run.duration_ms,
        });
    }

    // Branch 5: the child exited zero. Try to parse the protocol.
    match parse_alerts_document(run.stdout.as_bytes()) {
        Ok(doc) => {
            let alerts = doc.alerts_list().to_vec();
            let status = if alerts.is_empty() {
                Status::Empty
            } else {
                Status::Success
            };
            let alert_count = alerts.len();
            // Surface capture-pipeline notes without changing the status.
            // Every outcome carries a diagnostic: fall back to a
            // human-readable note so dry-run rows are self-explanatory.
            let capture_note = run
                .capture_error
                .clone()
                .filter(|s| !s.is_empty())
                .or_else(|| {
                    Some(if alert_count == 0 {
                        "parsed 0 alerts".to_string()
                    } else {
                        format!("parsed {alert_count} alert(s)")
                    })
                });
            if !dry_run {
                let new_alerts: Vec<NewAlert<'_>> = alerts
                    .iter()
                    .map(|a| NewAlert {
                        severity: a.severity.as_deref().unwrap_or(""),
                        message: a.message.as_deref().unwrap_or(""),
                        source: a.source.as_deref(),
                        symbol: a.symbol.as_deref(),
                    })
                    .collect();
                crate::repo::alerts::Alerts::record_run(
                    db,
                    &NewSnapshot {
                        taken_at,
                        checker_name: &spec.name,
                        status: status.as_str(),
                        diagnostic: capture_note.as_deref(),
                        raw_json: Some(run.stdout.as_str()),
                        git_commit,
                        git_branch,
                    },
                    &new_alerts,
                )?;
            }
            Ok(CheckerOutcome {
                name: spec.name.clone(),
                status,
                alert_count,
                diagnostic: capture_note,
                alerts,
                duration_ms: run.duration_ms,
            })
        }
        Err(err) => {
            let diagnostic = format_protocol_error(&err);
            if !dry_run {
                crate::repo::alerts::Alerts::record_run(
                    db,
                    &NewSnapshot {
                        taken_at,
                        checker_name: &spec.name,
                        status: Status::BadJson.as_str(),
                        diagnostic: Some(&diagnostic),
                        raw_json: Some(run.stdout.as_str()),
                        git_commit,
                        git_branch,
                    },
                    &[],
                )?;
            }
            Ok(CheckerOutcome {
                name: spec.name.clone(),
                status: Status::BadJson,
                alert_count: 0,
                diagnostic: Some(diagnostic),
                alerts: vec![],
                duration_ms: run.duration_ms,
            })
        }
    }
}

fn format_protocol_error(err: &ProtocolError) -> String {
    err.to_string()
}

fn truncate_diagnostic(s: &str) -> String {
    truncate_char_boundary(s, 200)
}

fn print_summary(outcomes: &[CheckerOutcome], dry_run: bool) {
    println!("driftwatch check: {} checker(s) run", outcomes.len());
    if dry_run {
        println!("(dry-run; nothing was persisted)");
    }
    println!();
    println!(
        "{:<24}  {:<12}  {:<7}  DIAGNOSTIC",
        "CHECKER", "STATUS", "ALERTS"
    );
    for o in outcomes {
        let diag = o.diagnostic.as_deref().unwrap_or("-");
        println!(
            "{:<24}  {:<12}  {:<7}  {}",
            truncate(&o.name, 24),
            o.status.as_str(),
            o.alert_count,
            truncate(diag, 80)
        );
    }
    let total_alerts: usize = outcomes.iter().map(|o| o.alert_count).sum();
    println!();
    println!("Total alerts: {total_alerts}");
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
        out.push('…');
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_handles_short_strings() {
        assert_eq!(truncate("hi", 10), "hi");
    }

    #[test]
    fn truncate_handles_long_strings() {
        let s = "a".repeat(100);
        let t = truncate(&s, 10);
        assert_eq!(t.chars().count(), 10);
        assert!(t.ends_with('…'));
    }

    #[test]
    fn truncate_handles_unicode_boundary() {
        // "é" is two bytes, so a naive byte slice would panic. The
        // function operates on chars and so it is safe.
        let s = "ééééé";
        let t = truncate(s, 3);
        assert!(t.chars().count() <= 3);
    }

    #[test]
    fn truncate_diagnostic_keeps_short_text_intact() {
        assert_eq!(truncate_diagnostic("hello"), "hello");
    }

    #[test]
    fn truncate_diagnostic_truncates_long_text() {
        let s = "x".repeat(500);
        let t = truncate_diagnostic(&s);
        assert!(t.ends_with('…'));
        // 200 chars + 3 bytes for the ellipsis = 203 bytes total.
        assert!(t.len() <= 203, "got {} bytes", t.len());
        // The visible content (excluding the ellipsis) is exactly LIMIT
        // ASCII chars.
        assert!(t.chars().count() <= 201);
    }
}
