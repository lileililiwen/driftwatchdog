//! `driftwatch gc`: prune bulky stdout/stderr older than `--days`.

use std::path::Path;

use rusqlite::params;

use crate::cli::GcArgs;
use crate::error::Error;
use crate::project::ProjectRoot;
use crate::repo::{bugs::Bugs, Db};

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
        println!(
            "driftwatch gc --dry-run: would prune {} runs (~{} bytes freed)",
            rows, bytes
        );
        return Ok(0);
    }

    let (rows, bytes) = Bugs::new(&mut db).prune_streams(&cutoff)?;
    println!(
        "driftwatch gc: pruned {} runs (~{} bytes freed estimate)",
        rows, bytes
    );
    Ok(0)
}
