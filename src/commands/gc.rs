//! `driftwatch gc`: prune bulky stdout/stderr older than `--days`,
//! plus retained gate artifact files (metadata rows are kept).

use std::path::Path;

use rusqlite::params;

use crate::cli::GcArgs;
use crate::error::Error;
use crate::project::ProjectRoot;
use crate::repo::{bugs::Bugs, evidence::Artifacts, Db};

/// Run the GC command. Always exits 0 on success.
pub fn gc(args: GcArgs, cwd: &Path) -> Result<i32, Error> {
    let proj = ProjectRoot::discover(cwd)?;
    let mut db = Db::open(&proj.db_path)?;
    let cutoff = (chrono::Utc::now() - chrono::Duration::days(args.days)).to_rfc3339();

    if args.dry_run {
        let (rows, bytes): (i64, i64) = db.conn().query_row(
            "SELECT COUNT(*),
                    COALESCE(SUM(COALESCE(LENGTH(stdout_excerpt),0)
                                + COALESCE(LENGTH(stderr_excerpt),0)),0)
             FROM runs
             WHERE started_at < ?1
               AND (stdout_excerpt IS NOT NULL OR stderr_excerpt IS NOT NULL)",
            params![cutoff],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        let (artifacts, artifact_bytes): (i64, i64) = db.conn().query_row(
            "SELECT COUNT(*), COALESCE(SUM(byte_size),0)
             FROM gate_artifacts
             WHERE created_at < ?1 AND available != 0 AND rel_path IS NOT NULL",
            params![cutoff],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        println!(
            "driftwatch gc --dry-run: would prune {} runs (~{} bytes freed) and {} artifacts (~{} bytes freed)",
            rows, bytes, artifacts, artifact_bytes
        );
        return Ok(0);
    }

    let (rows, bytes) = Bugs::new(&mut db).prune_streams(&cutoff)?;
    let (artifacts, artifact_bytes) = Artifacts::new(&db).prune_before(&proj.state_dir, &cutoff)?;
    println!(
        "driftwatch gc: pruned {} runs (~{} bytes freed estimate) and {} artifacts (~{} bytes freed, metadata retained)",
        rows, bytes, artifacts, artifact_bytes
    );
    Ok(0)
}
