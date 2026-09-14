//! `check_snapshots` + `drift_alerts` repositories.
//!
//! Populated by the `checker-and-drift-alerts` change. The foundation change
//! only defines the row types and provides a count helper used by `doctor`
//! (added later) and by future tests.

use rusqlite::{params, Connection, OptionalExtension};

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
    pub git_commit: Option<String>,
    pub git_branch: Option<String>,
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

/// Fields needed to record a new check snapshot. `raw_json` is the
/// raw text the checker wrote to stdout (used for diagnostics when
/// parsing fails; `None` for non-protocol failures).
#[derive(Debug, Clone)]
pub struct NewSnapshot<'a> {
    pub taken_at: &'a str,
    pub checker_name: &'a str,
    pub status: &'a str,
    pub diagnostic: Option<&'a str>,
    pub raw_json: Option<&'a str>,
    pub git_commit: Option<&'a str>,
    pub git_branch: Option<&'a str>,
}

/// Row to be inserted into `drift_alerts`. The repository assigns the id.
#[derive(Debug, Clone)]
pub struct NewAlert<'a> {
    pub severity: &'a str,
    pub message: &'a str,
    pub source: Option<&'a str>,
    pub symbol: Option<&'a str>,
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

    /// Convenience: every alert in the database, oldest first. Used
    /// by the `correlation-and-ai-context` change to enumerate the
    /// alert set without re-querying SQL.
    pub fn current_alerts(&self) -> Result<Vec<Alert>, Error> {
        self.list_alerts()
    }

    /// All check snapshots, oldest first. Used by export.
    pub fn list_snapshots(&self) -> Result<Vec<Snapshot>, Error> {
        let mut stmt = self.db.conn().prepare(
            "SELECT id, taken_at, checker_name, status, diagnostic, git_commit, git_branch
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
                    git_commit: r.get(5)?,
                    git_branch: r.get(6)?,
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

    /// Most-recent snapshot for a given checker name, if any. Used by
    /// `doctor` to surface a Warn when a configured checker has been
    /// failing on the last invocation.
    pub fn latest_snapshot_for(&self, checker_name: &str) -> Result<Option<Snapshot>, Error> {
        let mut stmt = self.db.conn().prepare(
            "SELECT id, taken_at, checker_name, status, diagnostic, git_commit, git_branch
             FROM check_snapshots
             WHERE checker_name = ?1
             ORDER BY id DESC LIMIT 1",
        )?;
        let row = stmt
            .query_row(params![checker_name], |r| {
                Ok(Snapshot {
                    id: r.get(0)?,
                    taken_at: r.get(1)?,
                    checker_name: r.get(2)?,
                    status: r.get(3)?,
                    diagnostic: r.get(4)?,
                    git_commit: r.get(5)?,
                    git_branch: r.get(6)?,
                })
            })
            .optional()?;
        Ok(row)
    }

    /// Persist a new check snapshot row. Returns the assigned snapshot id.
    /// Callers typically pair this with [`Self::insert_alerts`] in the same
    /// transaction (see [`Self::record_run`]).
    pub fn insert_snapshot(&self, snap: &NewSnapshot<'_>) -> Result<i64, Error> {
        self.db.conn().execute(
            "INSERT INTO check_snapshots
                (taken_at, checker_name, status, diagnostic, raw_json, git_commit, git_branch)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                snap.taken_at,
                snap.checker_name,
                snap.status,
                snap.diagnostic,
                snap.raw_json,
                snap.git_commit,
                snap.git_branch,
            ],
        )?;
        Ok(self.db.conn().last_insert_rowid())
    }

    /// Bulk-insert alerts attached to a snapshot. Returns the number of
    /// rows written.
    pub fn insert_alerts(&self, snapshot_id: i64, alerts: &[NewAlert<'_>]) -> Result<usize, Error> {
        if alerts.is_empty() {
            return Ok(0);
        }
        let mut stmt = self.db.conn().prepare(
            "INSERT INTO drift_alerts (snapshot_id, severity, message, source, symbol)
             VALUES (?1, ?2, ?3, ?4, ?5)",
        )?;
        let mut count = 0usize;
        for a in alerts {
            stmt.execute(params![
                snapshot_id,
                a.severity,
                a.message,
                a.source,
                a.symbol
            ])?;
            count += 1;
        }
        Ok(count)
    }

    /// Convenience helper: insert a snapshot and its alerts in a single
    /// transaction. Returns the snapshot id and the number of alerts
    /// written. Both writes roll back together on failure. Takes
    /// `&mut Db` directly because it opens a transaction on the shared
    /// connection.
    pub fn record_run(
        db: &mut Db,
        snap: &NewSnapshot<'_>,
        alerts: &[NewAlert<'_>],
    ) -> Result<(i64, usize), Error> {
        let tx = db.conn_mut().transaction()?;
        tx.execute(
            "INSERT INTO check_snapshots
                (taken_at, checker_name, status, diagnostic, raw_json, git_commit, git_branch)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                snap.taken_at,
                snap.checker_name,
                snap.status,
                snap.diagnostic,
                snap.raw_json,
                snap.git_commit,
                snap.git_branch,
            ],
        )?;
        let snapshot_id = tx.last_insert_rowid();
        let mut alert_count = 0usize;
        if !alerts.is_empty() {
            let mut stmt = tx.prepare(
                "INSERT INTO drift_alerts (snapshot_id, severity, message, source, symbol)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
            )?;
            for a in alerts {
                stmt.execute(params![
                    snapshot_id,
                    a.severity,
                    a.message,
                    a.source,
                    a.symbol
                ])?;
                alert_count += 1;
            }
        }
        tx.commit()?;
        Ok((snapshot_id, alert_count))
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
    fn schema_version_reports_five_after_init() {
        let db = Db::open_in_memory().unwrap();
        assert_eq!(schema_version(db.conn()).unwrap(), 5);
    }

    #[test]
    fn foundation_tables_present_after_init() {
        let db = Db::open_in_memory().unwrap();
        assert!(foundation_tables_present(db.conn()).unwrap());
    }

    #[test]
    fn record_run_persists_snapshot_and_alerts_in_one_tx() {
        let mut db = Db::open_in_memory().unwrap();
        let (snap_id, n) = Alerts::record_run(
            &mut db,
            &NewSnapshot {
                taken_at: "2026-01-01T00:00:00Z",
                checker_name: "arch",
                status: "success",
                diagnostic: None,
                raw_json: Some(r#"{"alerts":[]}"#),
                git_commit: Some("abc123"),
                git_branch: Some("main"),
            },
            &[
                NewAlert {
                    severity: "warning",
                    message: "m1",
                    source: Some("s1"),
                    symbol: Some("S1"),
                },
                NewAlert {
                    severity: "error",
                    message: "m2",
                    source: None,
                    symbol: None,
                },
            ],
        )
        .unwrap();
        assert!(snap_id > 0);
        assert_eq!(n, 2);
        let snaps = Alerts::new(&db).list_snapshots().unwrap();
        assert_eq!(snaps.len(), 1);
        assert_eq!(snaps[0].status, "success");
        assert_eq!(snaps[0].git_commit.as_deref(), Some("abc123"));
        let alerts = Alerts::new(&db).list_alerts().unwrap();
        assert_eq!(alerts.len(), 2);
        assert_eq!(alerts[0].snapshot_id, snap_id);
        assert_eq!(alerts[1].severity, "error");
    }

    #[test]
    fn record_run_with_no_alerts_persists_only_snapshot() {
        let mut db = Db::open_in_memory().unwrap();
        let (snap_id, n) = Alerts::record_run(
            &mut db,
            &NewSnapshot {
                taken_at: "2026-01-01T00:00:00Z",
                checker_name: "arch",
                status: "empty",
                diagnostic: None,
                raw_json: Some(r#"{"alerts":[]}"#),
                git_commit: None,
                git_branch: None,
            },
            &[],
        )
        .unwrap();
        assert!(snap_id > 0);
        assert_eq!(n, 0);
        assert_eq!(Alerts::new(&db).alert_count().unwrap(), 0);
        assert_eq!(Alerts::new(&db).snapshot_count().unwrap(), 1);
    }

    #[test]
    fn latest_snapshot_for_returns_most_recent() {
        let mut db = Db::open_in_memory().unwrap();
        Alerts::record_run(
            &mut db,
            &NewSnapshot {
                taken_at: "2026-01-01T00:00:00Z",
                checker_name: "arch",
                status: "success",
                diagnostic: None,
                raw_json: None,
                git_commit: None,
                git_branch: None,
            },
            &[],
        )
        .unwrap();
        Alerts::record_run(
            &mut db,
            &NewSnapshot {
                taken_at: "2026-01-02T00:00:00Z",
                checker_name: "arch",
                status: "failed",
                diagnostic: Some("exit 1"),
                raw_json: None,
                git_commit: None,
                git_branch: None,
            },
            &[],
        )
        .unwrap();
        let snap = Alerts::new(&db)
            .latest_snapshot_for("arch")
            .unwrap()
            .unwrap();
        assert_eq!(snap.status, "failed");
        assert_eq!(snap.diagnostic.as_deref(), Some("exit 1"));
    }

    #[test]
    fn latest_snapshot_for_unknown_checker_is_none() {
        let db = Db::open_in_memory().unwrap();
        let snap = Alerts::new(&db).latest_snapshot_for("nope").unwrap();
        assert!(snap.is_none());
    }
}
