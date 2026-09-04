//! `driftwatch run` orchestration. Reserves a run row, executes the child
//! process via the runtime, finalizes the row, and reports status.

use std::path::Path;

use crate::cli::RunArgs;
use crate::error::Error;
use crate::project::{config::Config, git, ProjectRoot};
use crate::repo::{runs::Runs, Db};
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
    let db = Db::open(&proj.db_path)?;
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

    render_summary(&program, &outcome);
    Ok(outcome.exit_code.unwrap_or(127))
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
