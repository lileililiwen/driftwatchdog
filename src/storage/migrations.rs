//! Versioned SQL migrations.
//!
//! Each migration has a monotonic integer version. Applied versions are
//! recorded in the `schema_version` table. Running `apply` is idempotent: it
//! only executes migrations whose version is greater than the current
//! `MAX(version)`.

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
pub fn apply(conn: &mut Connection) -> Result<i64, Error> {
    ensure_tracking_table(conn)?;

    let tx = conn.transaction()?;
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
        tx.execute_batch(sql).map_err(|e| Error::Migration {
            version: *version,
            message: e.to_string(),
        })?;
        tx.execute(
            "INSERT INTO schema_version (version, applied_at) VALUES (?1, ?2)",
            rusqlite::params![*version, &now],
        )?;
    }

    tx.commit()?;
    current_max(conn)
}

fn ensure_tracking_table(conn: &mut Connection) -> Result<(), Error> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_version (
             version INTEGER PRIMARY KEY,
             applied_at TEXT NOT NULL
         )",
    )?;
    Ok(())
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
}
