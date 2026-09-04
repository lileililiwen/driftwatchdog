//! `driftwatch list` orchestration. Prints recent runs in a fixed-width
//! table with filters.

use std::path::Path;

use crate::cli::ListArgs;
use crate::error::Error;
use crate::project::ProjectRoot;
use crate::repo::{
    bugs::Bugs,
    runs::{ListFilter, RunRecord, RunStatus, Runs},
    Db,
};

/// Run the `list` command. Always returns exit 0 on success.
pub fn list(args: ListArgs, cwd: &Path) -> Result<i32, Error> {
    let proj = ProjectRoot::discover(cwd)?;
    let mut db = Db::open(&proj.db_path)?;
    let rows = Runs::new(&db).list(&ListFilter {
        limit: args.limit,
        only_failed: args.failed,
        tag: args.tag.clone(),
    })?;

    if rows.is_empty() {
        println!("No runs match the current filter.");
        return Ok(0);
    }

    // Batch-fetch attached fingerprint hashes so we don't N+1 the
    // database. The map is empty when no row has a fingerprint.
    let ids: Vec<i64> = rows.iter().map(|r| r.id).collect();
    let hashes = Bugs::new(&mut db).hash_for_runs(&ids)?;

    print_table(&rows, &hashes);
    Ok(0)
}

fn print_table(rows: &[RunRecord], hashes: &std::collections::HashMap<i64, String>) {
    println!(
        "{:<20}  {:<7}  {:<10}  {:<8}  {:<10}  COMMAND",
        "STARTED", "STATUS", "EXIT", "DURATION", "HASH"
    );
    for r in rows {
        let hash = hashes
            .get(&r.id)
            .map(|h| h[..8.min(h.len())].to_string())
            .unwrap_or_else(|| "-".to_string());
        println!(
            "{:<20}  {:<7}  {:<10}  {:<8}  {:<10}  {}",
            r.started_at,
            status_label(r.status),
            exit_label(r.exit_code),
            duration_label(r.duration_ms),
            hash,
            format_command(&r.program, &r.argv_json)
        );
    }
}

fn status_label(s: RunStatus) -> &'static str {
    match s {
        RunStatus::Success => "ok",
        RunStatus::Failed => "FAIL",
        RunStatus::StartFailed => "NOEXE",
        RunStatus::Running => "RUN",
    }
}

fn exit_label(code: Option<i32>) -> String {
    match code {
        Some(c) => c.to_string(),
        None => "-".to_string(),
    }
}

fn duration_label(ms: Option<i64>) -> String {
    match ms {
        Some(m) => format!("{m}ms"),
        None => "-".to_string(),
    }
}

fn format_command(program: &str, argv_json: &str) -> String {
    let argv: Vec<String> = serde_json::from_str(argv_json).unwrap_or_default();
    let mut parts: Vec<String> = vec![program.to_string()];
    parts.extend(argv);
    let joined = parts.join(" ");
    if joined.len() > 80 {
        format!("{}…", &joined[..79])
    } else {
        joined
    }
}
