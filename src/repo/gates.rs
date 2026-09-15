//! Gate run history.
//!
//! One row per `driftwatch gate` execution: change/revision identity,
//! manifest and rule-pack provenance, aggregate status, blocking
//! outcome, and the per-gate results as canonical JSON. Rows are
//! append-only and ordered by `taken_at`; repeated gating of the same
//! change at two revisions yields two distinguishable rows.

use crate::error::Error;
use crate::repo::Db;

/// One persisted gate run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GateRunRow {
    pub id: i64,
    pub taken_at: String,
    pub change_id: String,
    pub revision: String,
    pub manifest_digest: String,
    pub rule_pack_version: String,
    pub status: String,
    pub blocked: bool,
    pub results_json: String,
}

/// Fields needed to persist one gate run.
#[derive(Debug, Clone)]
pub struct NewGateRun<'a> {
    pub taken_at: &'a str,
    pub change_id: &'a str,
    pub revision: &'a str,
    pub manifest_digest: &'a str,
    pub rule_pack_version: &'a str,
    pub status: &'a str,
    pub blocked: bool,
    pub results_json: &'a str,
}

/// Typed access to the `gate_runs` table. All reads tolerate a missing
/// table (databases predating migration 0006, or read-only opens that
/// never migrated) by reporting zero rows instead of failing.
pub struct Gates<'a> {
    db: &'a Db,
}

impl<'a> Gates<'a> {
    pub fn new(db: &'a Db) -> Self {
        Self { db }
    }

    fn table_present(conn: &rusqlite::Connection) -> bool {
        conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='gate_runs'",
            [],
            |r| r.get::<_, i64>(0),
        )
        .unwrap_or(0)
            > 0
    }

    /// Insert one run row inside the caller's transaction context and
    /// return its row id.
    pub fn insert(db: &mut Db, run: &NewGateRun<'_>) -> Result<i64, Error> {
        db.conn_mut().execute(
            "INSERT INTO gate_runs (taken_at, change_id, revision, manifest_digest, rule_pack_version, status, blocked, results_json) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            rusqlite::params![
                run.taken_at,
                run.change_id,
                run.revision,
                run.manifest_digest,
                run.rule_pack_version,
                run.status,
                i64::from(run.blocked),
                run.results_json,
            ],
        )?;
        Ok(db.conn().last_insert_rowid())
    }

    /// Number of persisted gate runs (0 when the table is absent).
    pub fn count(&self) -> Result<i64, Error> {
        if !Self::table_present(self.db.conn()) {
            return Ok(0);
        }
        let n: i64 = self
            .db
            .conn()
            .query_row("SELECT COUNT(*) FROM gate_runs", [], |r| r.get(0))?;
        Ok(n)
    }

    /// Latest run, if any.
    pub fn latest(&self) -> Result<Option<GateRunRow>, Error> {
        Ok(self.list(1)?.into_iter().next())
    }

    /// Runs newest-first, capped at `limit`.
    pub fn list(&self, limit: usize) -> Result<Vec<GateRunRow>, Error> {
        if !Self::table_present(self.db.conn()) {
            return Ok(Vec::new());
        }
        let mut stmt = self.db.conn().prepare(
            "SELECT id, taken_at, change_id, revision, manifest_digest, rule_pack_version, status, blocked, results_json FROM gate_runs ORDER BY id DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map(rusqlite::params![limit as i64], |r| {
            Ok(GateRunRow {
                id: r.get(0)?,
                taken_at: r.get(1)?,
                change_id: r.get(2)?,
                revision: r.get(3)?,
                manifest_digest: r.get(4)?,
                rule_pack_version: r.get(5)?,
                status: r.get(6)?,
                blocked: r.get::<_, i64>(7)? != 0,
                results_json: r.get(8)?,
            })
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_and_list_round_trip() {
        let mut db = Db::open_in_memory().unwrap();
        let now = "2026-09-15T00:00:00Z";
        let id = Gates::insert(
            &mut db,
            &NewGateRun {
                taken_at: now,
                change_id: "abc123",
                revision: "abc123",
                manifest_digest: "sha256:deadbeef",
                rule_pack_version: "local",
                status: "PASS",
                blocked: false,
                results_json: "[]",
            },
        )
        .unwrap();
        assert!(id > 0);
        let gates = Gates::new(&db);
        assert_eq!(gates.count().unwrap(), 1);
        let latest = gates.latest().unwrap().unwrap();
        assert_eq!(latest.change_id, "abc123");
        assert_eq!(latest.revision, "abc123");
        assert_eq!(latest.manifest_digest, "sha256:deadbeef");
        assert!(!latest.blocked);
    }

    #[test]
    fn repeated_change_at_two_revisions_stays_distinguishable() {
        let mut db = Db::open_in_memory().unwrap();
        for rev in ["rev-a", "rev-b"] {
            Gates::insert(
                &mut db,
                &NewGateRun {
                    taken_at: rev,
                    change_id: "change-1",
                    revision: rev,
                    manifest_digest: "sha256:x",
                    rule_pack_version: "local",
                    status: "FAIL",
                    blocked: true,
                    results_json: "[]",
                },
            )
            .unwrap();
        }
        let gates = Gates::new(&db);
        let rows = gates.list(10).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].revision, "rev-b");
        assert_eq!(rows[1].revision, "rev-a");
        assert!(rows.iter().all(|r| r.blocked));
    }

    #[test]
    fn missing_table_reads_as_empty() {
        let db = Db::open_in_memory().unwrap();
        db.conn().execute_batch("DROP TABLE gate_runs;").unwrap();
        let gates = Gates::new(&db);
        assert_eq!(gates.count().unwrap(), 0);
        assert!(gates.latest().unwrap().is_none());
        assert!(gates.list(5).unwrap().is_empty());
    }
}
