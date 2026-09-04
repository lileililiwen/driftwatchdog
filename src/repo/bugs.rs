//! `fingerprints` + `occurrences` repositories.
//!
//! The fingerprinting change will extend these with normalization, upsert,
//! and trend queries. The foundation change provides only what the schema
//! requires: type-level row definitions so later code can refer to them.

use rusqlite::{params, OptionalExtension};

use crate::error::Error;
use crate::repo::Db;

#[derive(Debug, Clone)]
pub struct Fingerprint {
    pub id: i64,
    pub hash: String,
    pub canonical: String,
    pub summary: Option<String>,
    pub first_seen_at: String,
    pub last_seen_at: String,
    pub occurrence_count: i64,
}

pub struct Bugs<'a> {
    db: &'a Db,
}

impl<'a> Bugs<'a> {
    pub fn new(db: &'a Db) -> Self {
        Self { db }
    }

    /// Look up a fingerprint by its hash. Returns `None` when not present.
    /// The full upsert (insert + update last_seen/count) is added by the
    /// `fingerprinting-and-retention` change.
    pub fn find_by_hash(&self, hash: &str) -> Result<Option<Fingerprint>, Error> {
        let row = self
            .db
            .conn()
            .query_row(
                "SELECT id, hash, canonical, summary, first_seen_at, last_seen_at, occurrence_count
                 FROM fingerprints WHERE hash = ?1",
                params![hash],
                map_fp,
            )
            .optional()?;
        Ok(row)
    }
}

fn map_fp(row: &rusqlite::Row<'_>) -> rusqlite::Result<Fingerprint> {
    Ok(Fingerprint {
        id: row.get(0)?,
        hash: row.get(1)?,
        canonical: row.get(2)?,
        summary: row.get(3)?,
        first_seen_at: row.get(4)?,
        last_seen_at: row.get(5)?,
        occurrence_count: row.get(6)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_by_hash_returns_none_for_missing() {
        let db = Db::open_in_memory().unwrap();
        let res = Bugs::new(&db).find_by_hash("nope").unwrap();
        assert!(res.is_none());
    }
}
