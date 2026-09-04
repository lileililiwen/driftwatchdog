//! `manual_links` repository placeholder. Populated by the
//! `correlation-and-ai-context` change.
//!
//! `list_all` is read by the `export-and-doctor` change so the export
//! DTO can include existing manual links without re-querying SQL from
//! the export module.

use rusqlite::params;

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
    /// the table is empty (the common case before the correlation change
    /// lands). Used by `driftwatch export`.
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
}
