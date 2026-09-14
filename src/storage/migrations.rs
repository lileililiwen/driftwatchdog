//! Versioned SQL migrations.
//!
//! Each migration has a monotonic integer version. Applied versions are
//! recorded in the `schema_version` table. Running `apply` is idempotent:
//! it only executes migrations whose version is greater than the current
//! `MAX(version)`, and every migration's DDL is rerunnable (column and
//! index creation guard on `PRAGMA table_info` / `sqlite_master`) so a
//! crash mid-migration or a concurrent first-run completes cleanly on
//! the next open instead of failing with `duplicate column`.

use rusqlite::Connection;

use crate::error::Error;
use crate::storage::schema;

/// All known migrations in order. New entries must be appended; never reorder
/// or rewrite a published migration.
const MIGRATIONS: &[(i64, &str)] = &[
    (1, schema::MIGRATION_0001_BASELINE),
    (2, schema::MIGRATION_0002_SNAPSHOT_GIT),
    (3, schema::MIGRATION_0003_CORRELATION_DETAIL),
    (4, schema::MIGRATION_0004_LINK_IDENTITY),
];

/// Apply all unapplied migrations and return the current schema version.
/// The tracking-table creation, version read, DDL, and version write
/// all happen inside a single `IMMEDIATE` transaction so two processes
/// opening a fresh DB concurrently serialize: exactly one migration
/// set applies and neither process errors.
pub fn apply(conn: &mut Connection) -> Result<i64, Error> {
    let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    tx.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_version (
             version INTEGER PRIMARY KEY,
             applied_at TEXT NOT NULL
         )",
    )?;
    let current: i64 = tx
        .query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_version",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);

    let now = chrono::Utc::now().to_rfc3339();
    for (version, sql) in MIGRATIONS.iter() {
        if *version <= current {
            continue;
        }
        apply_one(&tx, *version, sql)?;
        tx.execute(
            "INSERT OR IGNORE INTO schema_version (version, applied_at) VALUES (?1, ?2)",
            rusqlite::params![*version, &now],
        )?;
    }

    tx.commit()?;
    current_max(conn)
}

/// Execute one migration's batch, skipping DDL that is already in
/// effect so reruns after a crash are safe. `CREATE TABLE` / `CREATE
/// INDEX` statements already carry `IF NOT EXISTS`; `ALTER TABLE ADD
/// COLUMN` does not exist in that form in SQLite, so those lines are
/// guarded via `PRAGMA table_info`.
fn apply_one(tx: &rusqlite::Transaction<'_>, version: i64, sql: &str) -> Result<(), Error> {
    for stmt in sql.split(';') {
        let stmt = stmt.trim();
        if stmt.is_empty() {
            continue;
        }
        if let Some((table, column)) = parse_add_column(stmt) {
            if column_exists(tx, &table, &column)? {
                continue;
            }
        }
        tx.execute_batch(stmt).map_err(|e| Error::Migration {
            version,
            message: e.to_string(),
        })?;
    }
    Ok(())
}

/// Parse `ALTER TABLE <table> ADD COLUMN <column> ...` (case-insensitive),
/// returning `(table, column)` when the statement has that shape.
fn parse_add_column(stmt: &str) -> Option<(String, String)> {
    let upper = stmt.to_ascii_uppercase();
    let alter = upper.find("ALTER TABLE")?;
    let add = upper.find("ADD COLUMN")?;
    if add < alter {
        return None;
    }
    let table_part = stmt[alter + "ALTER TABLE".len()..add].trim();
    let table = table_part
        .trim_matches('"')
        .split_whitespace()
        .next()?
        .to_string();
    let after = stmt[add + "ADD COLUMN".len()..].trim();
    let column = after
        .trim_matches('"')
        .split_whitespace()
        .next()?
        .trim_matches('"')
        .to_string();
    if table.is_empty() || column.is_empty() {
        return None;
    }
    Some((table, column))
}

fn column_exists(tx: &rusqlite::Transaction<'_>, table: &str, column: &str) -> Result<bool, Error> {
    // Identifiers come from our own migration constants, so inline
    // quoting with doubled `"` is sufficient.
    let table_q = table.replace('"', "\"\"");
    let mut stmt = tx.prepare(&format!("PRAGMA table_info(\"{table_q}\")"))?;
    let rows = stmt.query_map([], |r| r.get::<_, String>(1))?;
    for row in rows {
        if row? == column {
            return Ok(true);
        }
    }
    Ok(false)
}

fn current_max(conn: &Connection) -> Result<i64, Error> {
    let v: i64 = conn
        .query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_version",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);
    Ok(v)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn applies_baseline_once() {
        let tmp = tempdir().unwrap();
        let p = tmp.path().join("state.db");
        let mut conn = crate::storage::open(&p).unwrap();
        let v = apply(&mut conn).unwrap();
        assert_eq!(v, 4);
        // Second call is a no-op and still reports the same version.
        let v2 = apply(&mut conn).unwrap();
        assert_eq!(v2, 4);
    }

    #[test]
    fn migration_0004_enforces_unique_pair() {
        let tmp = tempdir().unwrap();
        let p = tmp.path().join("state.db");
        let mut conn = crate::storage::open(&p).unwrap();
        apply(&mut conn).unwrap();
        // UNIQUE index on (fingerprint_id, alert_id) exists.
        let idx: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name='idx_manual_links_pair'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(idx, 1, "missing unique pair index");
        // OR check preserved: both NULL still rejected.
        let err = conn
            .execute(
                "INSERT INTO manual_links (note, created_at) VALUES (?1, ?2)",
                rusqlite::params!["orphan", "2026-01-01T00:00:00Z"],
            )
            .unwrap_err();
        assert!(err.to_string().to_lowercase().contains("check"));
    }

    #[test]
    fn rerun_after_partial_migration_0002_is_safe() {
        // Simulate a crash mid-migration: baseline applied, one of
        // migration 2's columns present, version row missing.
        let tmp = tempdir().unwrap();
        let p = tmp.path().join("state.db");
        let mut conn = crate::storage::open(&p).unwrap();
        conn.execute_batch(schema::MIGRATION_0001_BASELINE).unwrap();
        conn.execute_batch("ALTER TABLE check_snapshots ADD COLUMN git_commit TEXT;")
            .unwrap();
        // Full apply must complete without `duplicate column` errors.
        let v = apply(&mut conn).unwrap();
        assert_eq!(v, 4);
        // Both git columns present exactly once.
        let n: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('check_snapshots') WHERE name IN ('git_commit','git_branch')",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 2);
    }

    #[test]
    fn rerun_after_partial_migration_0003_is_safe() {
        let tmp = tempdir().unwrap();
        let p = tmp.path().join("state.db");
        let mut conn = crate::storage::open(&p).unwrap();
        conn.execute_batch(schema::MIGRATION_0001_BASELINE).unwrap();
        conn.execute_batch(schema::MIGRATION_0002_SNAPSHOT_GIT)
            .unwrap();
        conn.execute_batch("ALTER TABLE correlations ADD COLUMN score_message REAL;")
            .unwrap();
        let v = apply(&mut conn).unwrap();
        assert_eq!(v, 4);
        let n: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('correlations') WHERE name LIKE 'score_%' OR name = 'algorithm_version'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 5);
    }

    #[test]
    fn concurrent_first_run_applies_migrations_once() {
        use std::sync::{Arc, Barrier};
        use std::thread;
        let tmp = tempdir().unwrap();
        let p = tmp.path().join("state.db");
        let barrier = Arc::new(Barrier::new(4));
        let mut handles = Vec::new();
        for _ in 0..4 {
            let path = p.clone();
            let b = barrier.clone();
            handles.push(thread::spawn(move || {
                b.wait();
                let mut conn = crate::storage::open(&path).unwrap();
                apply(&mut conn)
            }));
        }
        let mut versions = Vec::new();
        for h in handles {
            versions.push(h.join().expect("thread panicked").expect("apply failed"));
        }
        assert!(versions.iter().all(|&v| v == 4), "got {versions:?}");
        let conn = crate::storage::open(&p).unwrap();
        let recorded: i64 = conn
            .query_row("SELECT MAX(version) FROM schema_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(recorded, 4);
    }
}
