//! `check_snapshots` + `drift_alerts` repositories.
//!
//! Populated by the `checker-and-drift-alerts` change. The foundation change
//! only defines the row types and provides a count helper used by `doctor`
//! (added later) and by future tests.

use rusqlite::{params, Connection};

use crate::error::Error;
use crate::repo::Db;

pub struct Alerts<'a> {
    db: &'a Db,
}

#[derive(Debug, Clone)]
pub struct Snapshot {
    pub id: i64,
    pub taken_at: String,
    pub checker_name: String,
    pub status: String,
    pub diagnostic: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Alert {
    pub id: i64,
    pub snapshot_id: i64,
    pub severity: String,
    pub message: String,
    pub source: Option<String>,
    pub symbol: Option<String>,
}

impl<'a> Alerts<'a> {
    pub fn new(db: &'a Db) -> Self {
        Self { db }
    }

    /// Number of recorded check snapshots. Always defined; useful for tests
    /// and the future `doctor` command.
    pub fn snapshot_count(&self) -> Result<i64, Error> {
        count(self.db.conn(), "check_snapshots")
    }

    /// Number of recorded drift alerts.
    pub fn alert_count(&self) -> Result<i64, Error> {
        count(self.db.conn(), "drift_alerts")
    }

    /// All check snapshots, oldest first. Used by export.
    pub fn list_snapshots(&self) -> Result<Vec<Snapshot>, Error> {
        let mut stmt = self.db.conn().prepare(
            "SELECT id, taken_at, checker_name, status, diagnostic
             FROM check_snapshots ORDER BY id",
        )?;
        let rows = stmt
            .query_map([], |r| {
                Ok(Snapshot {
                    id: r.get(0)?,
                    taken_at: r.get(1)?,
                    checker_name: r.get(2)?,
                    status: r.get(3)?,
                    diagnostic: r.get(4)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// All drift alerts, oldest first. Used by export.
    pub fn list_alerts(&self) -> Result<Vec<Alert>, Error> {
        let mut stmt = self.db.conn().prepare(
            "SELECT id, snapshot_id, severity, message, source, symbol
             FROM drift_alerts ORDER BY id",
        )?;
        let rows = stmt
            .query_map([], |r| {
                Ok(Alert {
                    id: r.get(0)?,
                    snapshot_id: r.get(1)?,
                    severity: r.get(2)?,
                    message: r.get(3)?,
                    source: r.get(4)?,
                    symbol: r.get(5)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }
}

fn count(conn: &Connection, table: &str) -> Result<i64, Error> {
    let q = format!("SELECT COUNT(*) FROM {table}");
    let n: i64 = conn.query_row(&q, [], |r| r.get(0))?;
    Ok(n)
}

/// Schema-version probe used by `doctor`. Returns the value stored in
/// the `schema_version` table, or `0` for an empty database.
pub fn schema_version(conn: &Connection) -> Result<i64, Error> {
    let n: i64 = conn.query_row(
        "SELECT COALESCE(MAX(version), 0) FROM schema_version",
        [],
        |r| r.get(0),
    )?;
    Ok(n)
}

/// A probe that confirms the foundation tables exist. Used by `doctor`
/// to fail loudly when the DB file is present but the schema is missing
/// (e.g. created by an older binary, or corrupted).
pub fn foundation_tables_present(conn: &Connection) -> Result<bool, Error> {
    let mut stmt = conn.prepare(
        "SELECT name FROM sqlite_master WHERE type='table' AND name IN
             ('runs','fingerprints','occurrences','check_snapshots',
              'drift_alerts','correlations','manual_links','schema_version')",
    )?;
    let mut rows = stmt.query(params![])?;
    let mut count = 0;
    while rows.next()?.is_some() {
        count += 1;
    }
    Ok(count == 8)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_count_is_zero_on_fresh_db() {
        let db = Db::open_in_memory().unwrap();
        assert_eq!(Alerts::new(&db).snapshot_count().unwrap(), 0);
    }

    #[test]
    fn alert_count_is_zero_on_fresh_db() {
        let db = Db::open_in_memory().unwrap();
        assert_eq!(Alerts::new(&db).alert_count().unwrap(), 0);
    }

    #[test]
    fn list_snapshots_is_empty_on_fresh_db() {
        let db = Db::open_in_memory().unwrap();
        assert!(Alerts::new(&db).list_snapshots().unwrap().is_empty());
    }

    #[test]
    fn list_alerts_is_empty_on_fresh_db() {
        let db = Db::open_in_memory().unwrap();
        assert!(Alerts::new(&db).list_alerts().unwrap().is_empty());
    }

    #[test]
    fn schema_version_reports_one_after_init() {
        let db = Db::open_in_memory().unwrap();
        assert_eq!(schema_version(db.conn()).unwrap(), 1);
    }

    #[test]
    fn foundation_tables_present_after_init() {
        let db = Db::open_in_memory().unwrap();
        assert!(foundation_tables_present(db.conn()).unwrap());
    }
}
