//! `fingerprints` + `occurrences` repositories.
//!
//! The fingerprinting change will extend these with normalization, upsert,
//! and trend queries. The `runtime-memory` change adds a `top` query that
//! consumes grouped fingerprint data for the `driftwatch top` command.

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

/// One row in the `top` query: a grouped recurring failure.
#[derive(Debug, Clone)]
pub struct TopRow {
    pub hash: String,
    pub summary: Option<String>,
    pub count: i64,
    pub first_seen_at: String,
    pub last_seen_at: String,
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

    /// Return the top recurring failures, ordered by occurrence count then
    /// recency. `cutoff` is an RFC3339 timestamp; rows with `last_seen_at`
    /// older than the cutoff are excluded. `tag_like` is an optional `LIKE`
    /// pattern that filters to fingerprints whose occurrences include a run
    /// whose tags column matches (e.g. `'%"auth"%'`).
    pub fn top(
        &self,
        limit: usize,
        cutoff: &str,
        tag_like: Option<&str>,
    ) -> Result<Vec<TopRow>, Error> {
        // The EXISTS subquery is built inline because the tag filter is
        // optional. When it is absent, the subquery is replaced by a constant
        // `1` so the planner does not have to materialize it.
        let (predicate, params_vec): (&str, Vec<Box<dyn rusqlite::ToSql>>) = match tag_like {
            Some(pattern) => (
                "f.last_seen_at >= ?1 AND EXISTS (
                    SELECT 1 FROM occurrences o JOIN runs r ON o.run_id = r.id
                    WHERE o.fingerprint_id = f.id AND r.tags LIKE ?2
                )",
                vec![Box::new(cutoff.to_string()), Box::new(pattern.to_string())],
            ),
            None => ("f.last_seen_at >= ?1", vec![Box::new(cutoff.to_string())]),
        };
        let sql = format!(
            "SELECT f.hash, f.summary, f.occurrence_count, f.first_seen_at, f.last_seen_at
             FROM fingerprints f
             WHERE {predicate}
             ORDER BY f.occurrence_count DESC, f.last_seen_at DESC
             LIMIT {limit}"
        );
        let params_refs: Vec<&dyn rusqlite::ToSql> = params_vec
            .iter()
            .map(|b| b.as_ref() as &dyn rusqlite::ToSql)
            .collect();
        let mut stmt = self.db.conn().prepare(&sql)?;
        let rows = stmt
            .query_map(params_refs.as_slice(), map_top)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
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

fn map_top(row: &rusqlite::Row<'_>) -> rusqlite::Result<TopRow> {
    Ok(TopRow {
        hash: row.get(0)?,
        summary: row.get(1)?,
        count: row.get(2)?,
        first_seen_at: row.get(3)?,
        last_seen_at: row.get(4)?,
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

    #[test]
    fn top_returns_empty_on_fresh_db() {
        let db = Db::open_in_memory().unwrap();
        let rows = Bugs::new(&db)
            .top(20, "2020-01-01T00:00:00Z", None)
            .unwrap();
        assert!(rows.is_empty());
    }
}
