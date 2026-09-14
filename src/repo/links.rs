//! `manual_links` repository: user-asserted relationships between
//! bug fingerprints and drift alerts. The
//! `correlation-and-ai-context` change extends this placeholder
//! with the `create`/`delete`/`find_by_id` helpers used by
//! `driftwatch link`/`driftwatch unlink`; the `identity-resolution`
//! change adds migration 4 (`UNIQUE(fingerprint_id, alert_id)` +
//! dedupe) and the application-level duplicate guard in `create`.
//!
//! The schema keeps the baseline OR `CHECK` (at least one endpoint
//! set): `driftwatch link` always persists both endpoints, so an XOR
//! check would reject every real link row.

use rusqlite::{params, OptionalExtension};

use crate::error::Error;
use crate::repo::Db;

pub struct Links<'a> {
    db: &'a Db,
}

#[derive(Debug, Clone)]
pub struct ManualLink {
    pub id: i64,
    pub fingerprint_id: Option<i64>,
    pub alert_id: Option<i64>,
    pub note: Option<String>,
    pub created_at: String,
}

impl<'a> Links<'a> {
    pub fn new(db: &'a Db) -> Self {
        Self { db }
    }

    /// All manual link rows, oldest first. Returns an empty vec when
    /// the table is empty.
    pub fn list_all(&self) -> Result<Vec<ManualLink>, Error> {
        let mut stmt = self.db.conn().prepare(
            "SELECT id, fingerprint_id, alert_id, note, created_at
             FROM manual_links ORDER BY id",
        )?;
        let rows = stmt
            .query_map(params![], |r| {
                Ok(ManualLink {
                    id: r.get(0)?,
                    fingerprint_id: r.get(1)?,
                    alert_id: r.get(2)?,
                    note: r.get(3)?,
                    created_at: r.get(4)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// Number of manual link rows. Mirrors `Correlations::count` for
    /// the doctor summary line.
    pub fn count(&self) -> Result<i64, Error> {
        let n: i64 = self
            .db
            .conn()
            .query_row("SELECT COUNT(*) FROM manual_links", [], |r| r.get(0))?;
        Ok(n)
    }

    /// Look up a manual link by id. Returns `None` when not present.
    pub fn find_by_id(&self, id: i64) -> Result<Option<ManualLink>, Error> {
        let row = self
            .db
            .conn()
            .query_row(
                "SELECT id, fingerprint_id, alert_id, note, created_at
                 FROM manual_links WHERE id = ?1",
                params![id],
                |r| {
                    Ok(ManualLink {
                        id: r.get(0)?,
                        fingerprint_id: r.get(1)?,
                        alert_id: r.get(2)?,
                        note: r.get(3)?,
                        created_at: r.get(4)?,
                    })
                },
            )
            .optional()?;
        Ok(row)
    }

    /// Persist a new manual link. `driftwatch link` sets both
    /// `fingerprint_id` and `alert_id` (the schema OR `CHECK`
    /// requires at least one endpoint; the pair form is the normal
    /// case). Duplicate `(fingerprint_id, alert_id)` pairs are
    /// rejected idempotently with [`Error::DuplicateLink`];
    /// missing FK targets surface [`Error::LinkTarget`] instead of a
    /// raw SQLite foreign-key error. Returns the persisted row. Takes
    /// `&mut Db` because the underlying SQLite transaction requires
    /// a mutable connection (consistent with other repo write paths).
    pub fn create(
        db: &mut Db,
        fingerprint_id: Option<i64>,
        alert_id: Option<i64>,
        note: Option<&str>,
        now: &str,
    ) -> Result<ManualLink, Error> {
        // Duplicate guard: idempotent error naming the existing row.
        if let (Some(fp), Some(al)) = (fingerprint_id, alert_id) {
            let existing: Option<i64> = db
                .conn()
                .query_row(
                    "SELECT id FROM manual_links WHERE fingerprint_id = ?1 AND alert_id = ?2",
                    params![fp, al],
                    |r| r.get(0),
                )
                .optional()?;
            if let Some(id) = existing {
                return Err(Error::DuplicateLink { id });
            }
            // FK existence diagnostics (clearer than raw FK errors).
            let fp_exists: bool = db.conn().query_row(
                "SELECT EXISTS(SELECT 1 FROM fingerprints WHERE id = ?1)",
                params![fp],
                |r| {
                    let v: i64 = r.get(0)?;
                    Ok(v != 0)
                },
            )?;
            if !fp_exists {
                return Err(Error::LinkTarget {
                    side: "bug",
                    raw: format!("bug id {fp}"),
                });
            }
            let al_exists: bool = db.conn().query_row(
                "SELECT EXISTS(SELECT 1 FROM drift_alerts WHERE id = ?1)",
                params![al],
                |r| {
                    let v: i64 = r.get(0)?;
                    Ok(v != 0)
                },
            )?;
            if !al_exists {
                return Err(Error::LinkTarget {
                    side: "spec",
                    raw: format!("spec id {al}"),
                });
            }
        }
        let tx = db.conn_mut().transaction()?;
        tx.execute(
            "INSERT INTO manual_links (fingerprint_id, alert_id, note, created_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![fingerprint_id, alert_id, note, now],
        )?;
        let id = tx.last_insert_rowid();
        let row = tx
            .query_row(
                "SELECT id, fingerprint_id, alert_id, note, created_at
                 FROM manual_links WHERE id = ?1",
                params![id],
                |r| {
                    Ok(ManualLink {
                        id: r.get(0)?,
                        fingerprint_id: r.get(1)?,
                        alert_id: r.get(2)?,
                        note: r.get(3)?,
                        created_at: r.get(4)?,
                    })
                },
            )
            .map_err(Error::Sqlite)?;
        tx.commit()?;
        Ok(row)
    }

    /// Remove a manual link by id. Returns `true` when a row was
    /// deleted, `false` when the id was not present. Never deletes
    /// more than one row.
    pub fn delete(db: &mut Db, id: i64) -> Result<bool, Error> {
        let n = db
            .conn_mut()
            .execute("DELETE FROM manual_links WHERE id = ?1", params![id])?;
        Ok(n > 0)
    }

    /// Every manual link that touches `fingerprint_id`, oldest first.
    pub fn list_for_fingerprint(&self, fingerprint_id: i64) -> Result<Vec<ManualLink>, Error> {
        let mut stmt = self.db.conn().prepare(
            "SELECT id, fingerprint_id, alert_id, note, created_at
             FROM manual_links WHERE fingerprint_id = ?1 ORDER BY id",
        )?;
        let rows = stmt
            .query_map(params![fingerprint_id], |r| {
                Ok(ManualLink {
                    id: r.get(0)?,
                    fingerprint_id: r.get(1)?,
                    alert_id: r.get(2)?,
                    note: r.get(3)?,
                    created_at: r.get(4)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// Every manual link that touches `alert_id`, oldest first.
    pub fn list_for_alert(&self, alert_id: i64) -> Result<Vec<ManualLink>, Error> {
        let mut stmt = self.db.conn().prepare(
            "SELECT id, fingerprint_id, alert_id, note, created_at
             FROM manual_links WHERE alert_id = ?1 ORDER BY id",
        )?;
        let rows = stmt
            .query_map(params![alert_id], |r| {
                Ok(ManualLink {
                    id: r.get(0)?,
                    fingerprint_id: r.get(1)?,
                    alert_id: r.get(2)?,
                    note: r.get(3)?,
                    created_at: r.get(4)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repo::alerts::{Alerts, NewAlert, NewSnapshot};
    use crate::repo::bugs::Bugs;

    fn db() -> Db {
        Db::open_in_memory().unwrap()
    }

    fn insert_alert(db: &mut Db) -> i64 {
        Alerts::record_run(
            db,
            &NewSnapshot {
                taken_at: "2026-01-01T00:00:00Z",
                checker_name: "spec",
                status: "success",
                diagnostic: None,
                raw_json: None,
                git_commit: None,
                git_branch: None,
            },
            &[NewAlert {
                severity: "warning",
                message: "m",
                source: Some("s"),
                symbol: Some("S"),
            }],
        )
        .unwrap();
        db.conn()
            .query_row("SELECT id FROM drift_alerts LIMIT 1", [], |r| r.get(0))
            .unwrap()
    }

    /// Insert one fingerprint row + one alert, return both ids.
    /// The fingerprint is required because `manual_links.fingerprint_id`
    /// has a foreign key to `fingerprints.id`.
    fn insert_fingerprint_and_alert(db: &mut Db) -> (i64, i64) {
        let fp = Bugs::new(db)
            .upsert_for_occurrence("err A", "A", "2026-01-01T00:00:00Z")
            .unwrap();
        let alert_id = insert_alert(db);
        (fp.id, alert_id)
    }

    #[test]
    fn create_persists_bug_and_alert_pair() {
        let mut db = db();
        let (fp_id, alert_id) = insert_fingerprint_and_alert(&mut db);
        let link = Links::create(
            &mut db,
            Some(fp_id),
            Some(alert_id),
            None,
            "2026-01-01T00:00:00Z",
        )
        .unwrap();
        assert_eq!(link.fingerprint_id, Some(fp_id));
        assert_eq!(link.alert_id, Some(alert_id));
        assert!(link.note.is_none());
        assert!(link.id > 0);
    }

    #[test]
    fn create_rejects_duplicate_pair_idempotently() {
        let mut db = db();
        let (fp_id, alert_id) = insert_fingerprint_and_alert(&mut db);
        let first = Links::create(
            &mut db,
            Some(fp_id),
            Some(alert_id),
            None,
            "2026-01-01T00:00:00Z",
        )
        .unwrap();
        let err = Links::create(
            &mut db,
            Some(fp_id),
            Some(alert_id),
            None,
            "2026-01-02T00:00:00Z",
        )
        .unwrap_err();
        assert!(matches!(err, Error::DuplicateLink { id } if id == first.id));
        assert!(err.to_string().contains("already linked"));
        assert_eq!(Links::new(&db).count().unwrap(), 1);
    }

    #[test]
    fn create_with_note_stores_note() {
        let mut db = db();
        let (fp_id, alert_id) = insert_fingerprint_and_alert(&mut db);
        let link = Links::create(
            &mut db,
            Some(fp_id),
            Some(alert_id),
            Some("see issue #108"),
            "2026-01-01T00:00:00Z",
        )
        .unwrap();
        assert_eq!(link.note.as_deref(), Some("see issue #108"));
    }

    #[test]
    fn find_by_id_returns_none_for_missing() {
        let db = db();
        let res = Links::new(&db).find_by_id(99).unwrap();
        assert!(res.is_none());
    }

    #[test]
    fn delete_returns_true_when_present_and_false_when_missing() {
        let mut db = db();
        let (fp_id, alert_id) = insert_fingerprint_and_alert(&mut db);
        let link = Links::create(
            &mut db,
            Some(fp_id),
            Some(alert_id),
            None,
            "2026-01-01T00:00:00Z",
        )
        .unwrap();
        assert!(Links::delete(&mut db, link.id).unwrap());
        assert!(!Links::delete(&mut db, link.id).unwrap());
        assert!(!Links::delete(&mut db, 9999).unwrap());
    }

    #[test]
    fn list_for_fingerprint_returns_only_matching() {
        let mut db = db();
        let alert_id = insert_alert(&mut db);
        // Two distinct fingerprints, each with one link to the same alert.
        let fp1 = Bugs::new(&mut db)
            .upsert_for_occurrence("err 1", "1", "2026-01-01T00:00:00Z")
            .unwrap();
        let fp2 = Bugs::new(&mut db)
            .upsert_for_occurrence("err 2", "2", "2026-01-02T00:00:00Z")
            .unwrap();
        let a = Links::create(
            &mut db,
            Some(fp1.id),
            Some(alert_id),
            None,
            "2026-01-01T00:00:00Z",
        )
        .unwrap();
        let _ = Links::create(
            &mut db,
            Some(fp2.id),
            Some(alert_id),
            None,
            "2026-01-01T00:00:00Z",
        )
        .unwrap();
        let rows = Links::new(&db).list_for_fingerprint(fp1.id).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, a.id);
    }

    #[test]
    fn schema_check_rejects_both_targets_null() {
        // Both fingerprint_id and alert_id NULL violates the CHECK
        // constraint added in the baseline migration. Construct a
        // raw insert that the higher-level `create` API rejects at
        // the FK layer before reaching CHECK; this test asserts the
        // CHECK layer instead.
        let mut db = db();
        let err = db
            .conn_mut()
            .execute(
                "INSERT INTO manual_links (note, created_at) VALUES (?1, ?2)",
                rusqlite::params!["orphan", "2026-01-01T00:00:00Z"],
            )
            .unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.to_lowercase().contains("check"),
            "expected CHECK constraint failure, got: {msg}"
        );
    }
}
