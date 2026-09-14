//! `correlations` repository: persisted heuristic relationships
//! between bug fingerprints and drift alerts.
//!
//! `list_all` is read by `driftwatch export` so the export DTO
//! reflects the persisted correlation table. The
//! `correlation-and-ai-context` change extends the schema with
//! per-component score columns (`score_message`, `score_symbol`,
//! `score_file`, `score_tag`) and an `algorithm_version` column
//! (always `"v1"` for rows from this change).

use rusqlite::{params, OptionalExtension};

use crate::error::Error;
use crate::repo::Db;
use crate::similarity::score::{ComponentScores, ALGO_VERSION};

pub struct Correlations<'a> {
    db: &'a Db,
}

#[derive(Debug, Clone)]
pub struct Correlation {
    pub id: i64,
    pub fingerprint_id: i64,
    pub alert_id: i64,
    pub score: f64,
    pub label: String,
    pub created_at: String,
    pub score_message: Option<f64>,
    pub score_symbol: Option<f64>,
    pub score_file: Option<f64>,
    pub score_tag: Option<f64>,
    pub algorithm_version: String,
}

impl<'a> Correlations<'a> {
    pub fn new(db: &'a Db) -> Self {
        Self { db }
    }

    /// All correlation rows, oldest first. Returns an empty vec when the
    /// table is empty.
    pub fn list_all(&self) -> Result<Vec<Correlation>, Error> {
        let mut stmt = self.db.conn().prepare(
            "SELECT id, fingerprint_id, alert_id, score, label, created_at,
                    score_message, score_symbol, score_file, score_tag,
                    algorithm_version
             FROM correlations ORDER BY id",
        )?;
        let rows = stmt
            .query_map(params![], |r| {
                Ok(Correlation {
                    id: r.get(0)?,
                    fingerprint_id: r.get(1)?,
                    alert_id: r.get(2)?,
                    score: r.get(3)?,
                    label: r.get(4)?,
                    created_at: r.get(5)?,
                    score_message: r.get(6)?,
                    score_symbol: r.get(7)?,
                    score_file: r.get(8)?,
                    score_tag: r.get(9)?,
                    algorithm_version: r.get(10)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// Number of correlation rows. Mirrors `Alerts::alert_count` for
    /// the doctor summary line.
    pub fn count(&self) -> Result<i64, Error> {
        let n: i64 = self
            .db
            .conn()
            .query_row("SELECT COUNT(*) FROM correlations", [], |r| r.get(0))?;
        Ok(n)
    }

    /// Insert one correlation row, or update the existing row for
    /// the `(fingerprint_id, alert_id)` pair. The label is derived
    /// from the score using the documented cautious terminology.
    /// Returns the row id (existing or newly assigned).
    pub fn upsert(
        &self,
        fingerprint_id: i64,
        alert_id: i64,
        scores: ComponentScores,
        now: &str,
    ) -> Result<i64, Error> {
        let label = label_for(scores.total);
        self.db.conn().execute(
            "INSERT INTO correlations
                (fingerprint_id, alert_id, score, label, created_at,
                 score_message, score_symbol, score_file, score_tag,
                 algorithm_version)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
             ON CONFLICT(fingerprint_id, alert_id) DO UPDATE SET
                score = excluded.score,
                label = excluded.label,
                score_message = excluded.score_message,
                score_symbol = excluded.score_symbol,
                score_file = excluded.score_file,
                score_tag = excluded.score_tag,
                algorithm_version = excluded.algorithm_version",
            params![
                fingerprint_id,
                alert_id,
                scores.total,
                label,
                now,
                scores.message,
                scores.symbol,
                scores.file,
                scores.tag,
                ALGO_VERSION,
            ],
        )?;
        let id: i64 = self
            .db
            .conn()
            .query_row(
                "SELECT id FROM correlations WHERE fingerprint_id = ?1 AND alert_id = ?2",
                params![fingerprint_id, alert_id],
                |r| r.get(0),
            )
            .optional()?
            .ok_or_else(|| Error::Sqlite(rusqlite::Error::QueryReturnedNoRows))?;
        Ok(id)
    }

    /// Delete every correlation row for `fingerprint_id` and
    /// re-insert the provided `(alert_id, scores)` pairs in a single
    /// transaction. Used by `commands/check` after a fresh batch of
    /// alerts is recorded so the persisted set is always "what the
    /// current candidates scored above the threshold". Returns the
    /// number of inserted rows. Takes `&mut Db` directly because the
    /// transaction requires a mutable connection (the `Correlations`
    /// struct only holds a shared borrow).
    pub fn replace_for_fingerprint(
        db: &mut Db,
        fingerprint_id: i64,
        pairs: &[(i64, ComponentScores)],
        now: &str,
    ) -> Result<usize, Error> {
        let tx = db.conn_mut().transaction()?;
        tx.execute(
            "DELETE FROM correlations WHERE fingerprint_id = ?1",
            params![fingerprint_id],
        )?;
        let mut inserted = 0usize;
        for (alert_id, scores) in pairs {
            let label = label_for(scores.total);
            tx.execute(
                "INSERT INTO correlations
                    (fingerprint_id, alert_id, score, label, created_at,
                     score_message, score_symbol, score_file, score_tag,
                     algorithm_version)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    fingerprint_id,
                    alert_id,
                    scores.total,
                    label,
                    now,
                    scores.message,
                    scores.symbol,
                    scores.file,
                    scores.tag,
                    ALGO_VERSION,
                ],
            )?;
            inserted += 1;
        }
        tx.commit()?;
        Ok(inserted)
    }
}

/// Cautious label derived from the total score. The wording is
/// deliberately qualified; the table heading "Possible
/// relationships" and the per-row label together signal that a
/// similarity number is not a root-cause claim.
pub fn label_for(total: f64) -> String {
    if total >= 0.85 {
        "heuristic correlation (strong)".to_string()
    } else if total >= 0.75 {
        "heuristic correlation".to_string()
    } else {
        "possible relationship".to_string()
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

    /// Insert one snapshot + one alert, return the alert id. The
    /// `&mut Db` requirement of `record_run` is satisfied by the
    /// caller, which then drops the mutable borrow before reading
    /// the alert id with `&Db`.
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

    /// Insert one fingerprint + one alert. The fingerprint is
    /// required because `correlations.fingerprint_id` has a foreign
    /// key to `fingerprints.id`.
    fn insert_fingerprint_and_alert(db: &mut Db) -> (i64, i64) {
        let fp = Bugs::new(db)
            .upsert_for_occurrence("err A", "A", "2026-01-01T00:00:00Z")
            .unwrap();
        let alert_id = insert_alert(db);
        (fp.id, alert_id)
    }

    fn scores(m: f64, s: f64, f: f64, t: f64) -> ComponentScores {
        let total = 0.5 * m + 0.2 * s + 0.2 * f + 0.1 * t;
        ComponentScores {
            message: m,
            symbol: s,
            file: f,
            tag: t,
            total,
        }
    }

    #[test]
    fn upsert_inserts_new_row() {
        let mut db = db();
        let (fp_id, alert_id) = insert_fingerprint_and_alert(&mut db);
        let id = Correlations::new(&db)
            .upsert(
                fp_id,
                alert_id,
                scores(1.0, 1.0, 1.0, 0.0),
                "2026-01-01T00:00:00Z",
            )
            .unwrap();
        assert!(id > 0);
        let all = Correlations::new(&db).list_all().unwrap();
        assert_eq!(all.len(), 1);
    }

    #[test]
    fn upsert_updates_existing_row_and_preserves_id() {
        let mut db = db();
        let (fp_id, alert_id) = insert_fingerprint_and_alert(&mut db);
        // total = 0.5 * 0.5 + 0 + 0 + 0 = 0.25
        let id1 = Correlations::new(&db)
            .upsert(
                fp_id,
                alert_id,
                scores(0.5, 0.0, 0.0, 0.0),
                "2026-01-01T00:00:00Z",
            )
            .unwrap();
        // total = 0.5 * 0.9 + 0 + 0 + 0 = 0.45
        let id2 = Correlations::new(&db)
            .upsert(
                fp_id,
                alert_id,
                scores(0.9, 0.0, 0.0, 0.0),
                "2026-01-01T00:00:00Z",
            )
            .unwrap();
        assert_eq!(id1, id2);
        let all = Correlations::new(&db).list_all().unwrap();
        assert_eq!(all.len(), 1);
        // The second upsert overwrote the score.
        assert!((all[0].score - 0.45).abs() < 1e-9, "got {}", all[0].score);
    }

    #[test]
    fn upsert_writes_algorithm_version_v1() {
        let mut db = db();
        let (fp_id, alert_id) = insert_fingerprint_and_alert(&mut db);
        Correlations::new(&db)
            .upsert(
                fp_id,
                alert_id,
                scores(0.8, 0.0, 0.0, 0.0),
                "2026-01-01T00:00:00Z",
            )
            .unwrap();
        let all = Correlations::new(&db).list_all().unwrap();
        assert_eq!(all[0].algorithm_version, ALGO_VERSION);
    }

    #[test]
    fn list_all_returns_new_fields() {
        let mut db = db();
        let (fp_id, alert_id) = insert_fingerprint_and_alert(&mut db);
        Correlations::new(&db)
            .upsert(
                fp_id,
                alert_id,
                scores(0.5, 0.25, 0.5, 0.0),
                "2026-01-01T00:00:00Z",
            )
            .unwrap();
        let all = Correlations::new(&db).list_all().unwrap();
        assert_eq!(all[0].score_message, Some(0.5));
        assert_eq!(all[0].score_symbol, Some(0.25));
        assert_eq!(all[0].score_file, Some(0.5));
        assert_eq!(all[0].score_tag, Some(0.0));
    }

    #[test]
    fn replace_for_fingerprint_writes_only_passing_pairs() {
        let mut db = db();
        let (fp_id, alert_id) = insert_fingerprint_and_alert(&mut db);
        let n = Correlations::replace_for_fingerprint(
            &mut db,
            fp_id,
            &[(alert_id, scores(1.0, 1.0, 1.0, 0.0))],
            "2026-01-01T00:00:00Z",
        )
        .unwrap();
        assert_eq!(n, 1);
        let all = Correlations::new(&db).list_all().unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].fingerprint_id, fp_id);
    }

    #[test]
    fn replace_for_fingerprint_removes_existing_rows_for_fingerprint() {
        let mut db = db();
        let (fp_id, alert_id) = insert_fingerprint_and_alert(&mut db);
        Correlations::new(&db)
            .upsert(
                fp_id,
                alert_id,
                scores(0.5, 0.0, 0.0, 0.0),
                "2026-01-01T00:00:00Z",
            )
            .unwrap();
        assert_eq!(Correlations::new(&db).count().unwrap(), 1);
        Correlations::replace_for_fingerprint(&mut db, fp_id, &[], "2026-01-01T00:00:00Z").unwrap();
        assert_eq!(Correlations::new(&db).count().unwrap(), 0);
    }

    #[test]
    fn label_for_uses_qualified_wording() {
        assert_eq!(label_for(0.90), "heuristic correlation (strong)");
        assert_eq!(label_for(0.80), "heuristic correlation");
        assert_eq!(label_for(0.70), "possible relationship");
    }
}
