//! `correlations` repository placeholder. Populated by the
//! `correlation-and-ai-context` change.
//!
//! `list_all` is read by the `export-and-doctor` change so the export
//! DTO can include existing correlation rows without re-querying SQL
//! from the export module.

use rusqlite::params;

use crate::error::Error;
use crate::repo::Db;

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
}

impl<'a> Correlations<'a> {
    pub fn new(db: &'a Db) -> Self {
        Self { db }
    }

    /// All correlation rows, oldest first. Returns an empty vec when the
    /// table is empty (the common case before the correlation change
    /// lands). Used by `driftwatch export`.
    pub fn list_all(&self) -> Result<Vec<Correlation>, Error> {
        let mut stmt = self.db.conn().prepare(
            "SELECT id, fingerprint_id, alert_id, score, label, created_at
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
}
