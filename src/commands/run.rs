//! `driftwatch run` orchestration. Reserves a run row, executes the child
//! process via the runtime, finalizes the row, and reports status.
//!
//! For `Failed`, `StartFailed`, `Signalled`, and `Timeout` runs, the captured stderr (or stdout
//! as a fallback) is normalized and attached as a fingerprint with a
//! single occurrence row. Successful runs are not fingerprinted.

use std::path::Path;

use crate::cli::RunArgs;
use crate::error::Error;
use crate::fingerprint::{self, Rules};
use crate::project::{config::Config, git, ProjectRoot};
use crate::repo::{
    bugs::Bugs,
    runs::{RunStatus, Runs},
    Db,
};
use crate::runtime::{self, CaptureLimits, CommandSpec};

const DEFAULT_CAPTURE_BYTES: u64 = 64 * 1024;

/// Execute `driftwatch run`. Returns the child's exit code (or 127 when the
/// process could not be started).
pub fn run(args: RunArgs, cwd: &Path) -> Result<i32, Error> {
    if args.command.is_empty() {
        return Err(Error::Io {
            path: cwd.to_path_buf(),
            source: std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "no program supplied to run",
            ),
        });
    }

    let proj = ProjectRoot::discover(cwd)?;
    let mut db = Db::open(&proj.db_path)?;
    let cfg = Config::load(&proj.config_path)?;
    let limits = capture_limits(&cfg);

    let program = args.command[0].clone();
    let argv: Vec<String> = args.command[1..].to_vec();
    let argv_json = serde_json::to_string(&argv)?;

    let started_at = chrono::Utc::now().to_rfc3339();
    let run_id = Runs::new(&db).reserve(
        &started_at,
        &program,
        &argv_json,
        cwd.to_str().unwrap_or("."),
    )?;

    let spec = CommandSpec {
        program: program.clone(),
        args: argv,
        cwd: cwd.to_path_buf(),
        tags: args.tag.clone(),
        capture_limits: limits,
        timeout_ms: args.timeout_ms,
    };

    let outcome = runtime::run(&spec)?;
    let finished_at = chrono::Utc::now().to_rfc3339();
    let git_ctx = git::capture(cwd);

    Runs::new(&db).insert_full(
        run_id,
        &crate::repo::runs::RunCompletion {
            finished_at: &finished_at,
            duration_ms: outcome.duration_ms,
            exit_code: outcome.exit_code,
            status: outcome.status,
            tags: &args.tag,
            stdout_excerpt: excerpt(&outcome.stdout.text),
            stderr_excerpt: excerpt(&outcome.stderr.text),
            stdout_truncated: outcome.stdout.truncated,
            stderr_truncated: outcome.stderr.truncated,
            git_commit: git_ctx.commit.as_deref(),
            git_branch: git_ctx.branch.as_deref(),
            git_dirty: git_ctx.dirty,
        },
    )?;

    if let Some(bug) = fingerprint_failure(&mut db, run_id, &outcome, &finished_at)? {
        eprintln!(
            "driftwatch: bug {} \"{}\" ({} occurrences)",
            &bug.hash[..8],
            bug.summary.as_deref().unwrap_or("(no summary)"),
            bug.occurrence_count
        );
    }

    render_summary(&program, &outcome);
    Ok(outcome.exit_code.unwrap_or(127))
}

/// Build a fingerprint + occurrence row for a failed run. Returns the
/// updated fingerprint so the caller can print a short summary line.
/// Signalled and timed-out runs are fingerprinted like failures: a
/// recurring kill/timeout is a bug worth remembering.
fn fingerprint_failure(
    db: &mut Db,
    run_id: i64,
    outcome: &runtime::RunOutcome,
    finished_at: &str,
) -> Result<Option<crate::repo::bugs::Fingerprint>, Error> {
    if !matches!(
        outcome.status,
        RunStatus::Failed | RunStatus::StartFailed | RunStatus::Signalled | RunStatus::Timeout
    ) {
        return Ok(None);
    }
    // Prefer stderr; fall back to stdout when stderr is empty so
    // commands like `sh -c 'echo boom; exit 1'` are still
    // fingerprinted. StartFailed runs use the diagnostic in stderr.
    let raw = if !outcome.stderr.text.is_empty() {
        outcome.stderr.text.clone()
    } else {
        outcome.stdout.text.clone()
    };
    if raw.trim().is_empty() {
        return Ok(None);
    }
    let canonical = Rules::generic().normalize(&raw);
    let fp =
        Bugs::new(db).upsert_for_occurrence(&canonical.text, &canonical.summary, finished_at)?;
    let excerpt = fingerprint::bounded_excerpt(&raw);
    Bugs::new(db).insert_occurrence(fp.id, run_id, finished_at, Some(&excerpt))?;
    Ok(Some(fp))
}

fn capture_limits(cfg: &Config) -> CaptureLimits {
    CaptureLimits {
        stdout_bytes: Some(
            cfg.storage
                .max_stdout_bytes
                .unwrap_or(DEFAULT_CAPTURE_BYTES),
        ),
        stderr_bytes: Some(
            cfg.storage
                .max_stderr_bytes
                .unwrap_or(DEFAULT_CAPTURE_BYTES),
        ),
    }
}

fn excerpt(text: &str) -> Option<&str> {
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

fn render_summary(program: &str, outcome: &runtime::RunOutcome) {
    if let Some(diag) = outcome.diagnostic.as_deref() {
        if !diag.is_empty() {
            eprintln!("driftwatch: note: {diag}");
        }
    }
    match outcome.status {
        crate::repo::runs::RunStatus::Success => {
            println!("driftwatch: {} ({}ms)", program, outcome.duration_ms);
        }
        crate::repo::runs::RunStatus::Failed => {
            eprintln!(
                "driftwatch: FAIL exit={:?} duration={}ms",
                outcome.exit_code, outcome.duration_ms
            );
            if !outcome.stderr.text.is_empty() {
                let tail = tail_lines(&outcome.stderr.text, 10);
                eprintln!("--- stderr (last {} lines) ---", tail.lines().count());
                eprint!("{tail}");
            }
        }
        crate::repo::runs::RunStatus::StartFailed => {
            eprintln!(
                "driftwatch: could not start {}: {}",
                program, outcome.stderr.text
            );
        }
        crate::repo::runs::RunStatus::Signalled => {
            eprintln!(
                "driftwatch: {} killed by signal (exit unavailable) duration={}ms",
                program, outcome.duration_ms
            );
        }
        crate::repo::runs::RunStatus::Timeout => {
            eprintln!(
                "driftwatch: {} timed out duration={}ms",
                program, outcome.duration_ms
            );
        }
        crate::repo::runs::RunStatus::Running => {
            // Should not happen; RunOutcome always has a terminal status.
        }
    }
}

fn tail_lines(text: &str, n: usize) -> String {
    let lines: Vec<&str> = text.lines().collect();
    if lines.len() <= n {
        text.to_string()
    } else {
        let start = lines.len() - n;
        lines[start..].join("\n")
    }
}
