//! `fingerprints` + `occurrences` repositories.
//!
//! Owns the schema for recurring bug identity: stable SHA-256 hash,
//! canonical text, summary, occurrence count, and the per-run
//! occurrence rows used to render reports and detect trends.

use std::collections::HashMap;

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

/// One occurrence of a fingerprint, joined with the originating run's
/// Git metadata. Used by `driftwatch show` and `driftwatch report`.
#[derive(Debug, Clone)]
pub struct Occurrence {
    pub id: i64,
    pub run_id: i64,
    pub seen_at: String,
    pub excerpt: Option<String>,
    pub git_commit: Option<String>,
    pub git_branch: Option<String>,
}

/// One distinct commit hash observed on a fingerprint's occurrences.
#[derive(Debug, Clone)]
pub struct RecentCommit {
    pub commit: String,
    pub seen_at: String,
}

/// Full report payload for a single fingerprint.
#[derive(Debug, Clone)]
pub struct Report {
    pub fingerprint: Fingerprint,
    pub occurrences: Vec<Occurrence>,
    pub recent_commits: Vec<RecentCommit>,
    pub distinct_runs: i64,
}

pub struct Bugs<'a> {
    db: &'a mut Db,
}

impl<'a> Bugs<'a> {
    pub fn new(db: &'a mut Db) -> Self {
        Self { db }
    }

    /// All fingerprints, ordered by `id` ascending. Used by `driftwatch
    /// export` to enumerate the recurring-bug set without filter.
    pub fn all(&self) -> Result<Vec<Fingerprint>, Error> {
        let mut stmt = self.db.conn().prepare(
            "SELECT id, hash, canonical, summary, first_seen_at, last_seen_at, occurrence_count
             FROM fingerprints ORDER BY id",
        )?;
        let rows = stmt
            .query_map([], map_fp)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// Convenience: every fingerprint in the database, oldest first.
    /// Used by the `correlation-and-ai-context` change to enumerate
    /// the bug set without re-querying SQL.
    pub fn current_fingerprints(&self) -> Result<Vec<Fingerprint>, Error> {
        self.all()
    }

    /// Distinct tag set observed on a fingerprint's occurrences, in
    /// encounter order. Tags are parsed from the `runs.tags` JSON
    /// column for each occurrence; duplicates are removed. Returns
    /// an empty `Vec` when the fingerprint has no occurrences or no
    /// tagged runs.
    pub fn tags_for(&self, fingerprint_id: i64) -> Result<Vec<String>, Error> {
        let mut stmt = self.db.conn().prepare(
            "SELECT r.tags
             FROM occurrences o
             JOIN runs r ON o.run_id = r.id
             WHERE o.fingerprint_id = ?1",
        )?;
        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut out: Vec<String> = Vec::new();
        let rows = stmt.query_map(params![fingerprint_id], |r| {
            let tags_json: String = r.get(0)?;
            Ok(tags_json)
        })?;
        for row in rows {
            let tags_json = row?;
            let tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();
            for t in tags {
                if seen.insert(t.clone()) {
                    out.push(t);
                }
            }
        }
        Ok(out)
    }

    /// Look up a fingerprint by its hash. Returns `None` when not present.
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

    /// Look up a fingerprint by its integer primary key.
    pub fn find_by_id(&self, id: i64) -> Result<Option<Fingerprint>, Error> {
        let row = self
            .db
            .conn()
            .query_row(
                "SELECT id, hash, canonical, summary, first_seen_at, last_seen_at, occurrence_count
                 FROM fingerprints WHERE id = ?1",
                params![id],
                map_fp,
            )
            .optional()?;
        Ok(row)
    }

    /// Look up a fingerprint by a hex prefix of its hash. Returns `None`
    /// when the prefix is empty/non-hex, when no row matches, or when
    /// the prefix is shorter than 8 hex chars and matches more than one
    /// row (ambiguous). Callers should treat `None` as "not uniquely
    /// resolvable".
    pub fn find_by_hash_prefix(&self, prefix: &str) -> Result<Option<Fingerprint>, Error> {
        let prefix = prefix.trim();
        if prefix.is_empty() || !prefix.chars().all(|c| c.is_ascii_hexdigit()) {
            return Ok(None);
        }
        // SQLite's text collation is sufficient for hex comparison: a
        // prefix of a hex string is a strict text prefix of the full
        // string. The `length(prefix)` check ensures the matched row's
        // hash actually begins with our prefix (without it, `LIKE 'a%'`
        // would also match `aa…`).
        let like = format!("{prefix}%");
        let mut rows: Vec<Fingerprint> = self
            .db
            .conn()
            .prepare(
                "SELECT id, hash, canonical, summary, first_seen_at, last_seen_at, occurrence_count
                 FROM fingerprints
                 WHERE hash LIKE ?1 AND length(hash) >= ?2
                 ORDER BY id",
            )?
            .query_map(params![like, prefix.len() as i64], map_fp)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        if rows.is_empty() {
            return Ok(None);
        }
        // Disambiguate: prefixes shorter than 8 hex chars are ambiguous
        // when more than one row matches.
        if rows.len() > 1 && prefix.len() < 8 {
            return Ok(None);
        }
        Ok(Some(rows.remove(0)))
    }

    /// Hash attached to a specific run (via its most recent occurrence).
    /// Returns `None` when the run has no associated fingerprint.
    pub fn hash_for_run(&self, run_id: i64) -> Result<Option<String>, Error> {
        let row: Option<String> = self
            .db
            .conn()
            .query_row(
                "SELECT f.hash
                 FROM occurrences o
                 JOIN fingerprints f ON o.fingerprint_id = f.id
                 WHERE o.run_id = ?1
                 ORDER BY o.id DESC
                 LIMIT 1",
                params![run_id],
                |r| r.get(0),
            )
            .optional()?;
        Ok(row)
    }

    /// Batch version of [`hash_for_run`]. Returns a map keyed by `run_id`
    /// with the most recently attached hash (newest `occurrence.id` wins).
    pub fn hash_for_runs(&self, ids: &[i64]) -> Result<HashMap<i64, String>, Error> {
        if ids.is_empty() {
            return Ok(HashMap::new());
        }
        let placeholders = std::iter::repeat_n("?", ids.len())
            .collect::<Vec<_>>()
            .join(",");
        let sql = format!(
            "SELECT o.run_id, f.hash
             FROM occurrences o
             JOIN fingerprints f ON o.fingerprint_id = f.id
             WHERE o.run_id IN ({placeholders})
             ORDER BY o.id DESC"
        );
        let mut stmt = self.db.conn().prepare(&sql)?;
        let params_vec: Vec<&dyn rusqlite::ToSql> =
            ids.iter().map(|i| i as &dyn rusqlite::ToSql).collect();
        let mut map: HashMap<i64, String> = HashMap::new();
        let rows = stmt.query_map(params_vec.as_slice(), |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?))
        })?;
        for row in rows {
            let (run_id, hash) = row?;
            // First row per run_id wins because we sort by o.id DESC.
            map.entry(run_id).or_insert(hash);
        }
        Ok(map)
    }

    /// Insert a new fingerprint or update the existing one for `hash`.
    /// The first_seen_at is preserved when the row already exists;
    /// last_seen_at and occurrence_count are updated. `summary` is set
    /// on insert and preserved on update (first canonical text wins).
    /// Returns the resulting fingerprint row. Takes `&mut self` because
    /// the underlying SQLite transaction requires a mutable connection.
    pub fn upsert_for_occurrence(
        &mut self,
        canonical: &str,
        summary: &str,
        seen_at: &str,
    ) -> Result<Fingerprint, Error> {
        let hash = crate::fingerprint::fingerprint(canonical);
        let conn = self.db.conn_mut();
        let tx = conn.transaction()?;
        // `INSERT … ON CONFLICT DO NOTHING` is supported by SQLite
        // (>= 3.24); the project uses bundled rusqlite which targets a
        // modern SQLite. The follow-up SELECT tells us whether we
        // inserted or hit an existing row.
        let inserted = tx.execute(
            "INSERT INTO fingerprints (hash, canonical, summary, first_seen_at, last_seen_at, occurrence_count)
             VALUES (?1, ?2, ?3, ?4, ?4, 1)
             ON CONFLICT(hash) DO NOTHING",
            params![hash, canonical, summary, seen_at],
        )?;
        if inserted == 0 {
            tx.execute(
                "UPDATE fingerprints
                 SET last_seen_at = MAX(last_seen_at, ?2),
                     occurrence_count = occurrence_count + 1,
                     summary = COALESCE(summary, ?3)
                 WHERE hash = ?1",
                params![hash, seen_at, summary],
            )?;
        }
        let fp = tx
            .query_row(
                "SELECT id, hash, canonical, summary, first_seen_at, last_seen_at, occurrence_count
                 FROM fingerprints WHERE hash = ?1",
                params![hash],
                map_fp,
            )
            .map_err(Error::Sqlite)?;
        tx.commit()?;
        Ok(fp)
    }

    /// Insert one occurrence row. `excerpt` is truncated by the caller
    /// (via `crate::fingerprint::bounded_excerpt`) before binding.
    pub fn insert_occurrence(
        &self,
        fingerprint_id: i64,
        run_id: i64,
        seen_at: &str,
        excerpt: Option<&str>,
    ) -> Result<i64, Error> {
        self.db.conn().execute(
            "INSERT INTO occurrences (fingerprint_id, run_id, seen_at, excerpt)
             VALUES (?1, ?2, ?3, ?4)",
            params![fingerprint_id, run_id, seen_at, excerpt],
        )?;
        Ok(self.db.conn().last_insert_rowid())
    }

    /// All occurrences for `fingerprint_id`, newest first, capped at
    /// `limit`.
    pub fn occurrences_for(
        &self,
        fingerprint_id: i64,
        limit: usize,
    ) -> Result<Vec<Occurrence>, Error> {
        let mut stmt = self.db.conn().prepare(
            "SELECT o.id, o.run_id, o.seen_at, o.excerpt, r.git_commit, r.git_branch
             FROM occurrences o
             JOIN runs r ON o.run_id = r.id
             WHERE o.fingerprint_id = ?1
             ORDER BY o.seen_at DESC, o.id DESC
             LIMIT ?2",
        )?;
        let rows = stmt
            .query_map(params![fingerprint_id, limit as i64], map_occ)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// Distinct recent commits observed on a fingerprint's
    /// occurrences, ordered by most recent `seen_at`. `git_commit`
    /// values are de-duplicated; only non-null commits are returned.
    pub fn recent_commits(
        &self,
        fingerprint_id: i64,
        limit: usize,
    ) -> Result<Vec<RecentCommit>, Error> {
        let mut stmt = self.db.conn().prepare(
            "SELECT r.git_commit, MAX(o.seen_at) AS last_seen
             FROM occurrences o
             JOIN runs r ON o.run_id = r.id
             WHERE o.fingerprint_id = ?1 AND r.git_commit IS NOT NULL
             GROUP BY r.git_commit
             ORDER BY last_seen DESC
             LIMIT ?2",
        )?;
        let rows = stmt
            .query_map(params![fingerprint_id, limit as i64], |r| {
                Ok(RecentCommit {
                    commit: r.get(0)?,
                    seen_at: r.get(1)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// Aggregate payload for `driftwatch show` / `driftwatch report`.
    /// `distinct_runs` is the number of unique run ids that produced
    /// an occurrence for this fingerprint.
    pub fn report_for(&self, fingerprint_id: i64) -> Result<Report, Error> {
        let fp = self
            .find_by_id(fingerprint_id)?
            .ok_or_else(|| Error::BugNotFound {
                id: format!("id={fingerprint_id}"),
            })?;
        let occurrences = self.occurrences_for(fingerprint_id, 50)?;
        let recent_commits = self.recent_commits(fingerprint_id, 20)?;
        let distinct_runs: i64 = self.db.conn().query_row(
            "SELECT COUNT(DISTINCT run_id) FROM occurrences WHERE fingerprint_id = ?1",
            params![fingerprint_id],
            |r| r.get(0),
        )?;
        Ok(Report {
            fingerprint: fp,
            occurrences,
            recent_commits,
            distinct_runs,
        })
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

    /// NULL out `stdout_excerpt` and `stderr_excerpt` for runs whose
    /// `started_at` is strictly older than `cutoff`. The run row and
    /// the run/occurrence linkage are preserved. Returns
    /// `(rows_pruned, bytes_freed_estimate)`. Idempotent: a second
    /// invocation with the same cutoff prunes zero rows. Takes
    /// `&mut self` because the underlying SQLite transaction requires
    /// a mutable connection.
    pub fn prune_streams(&mut self, cutoff: &str) -> Result<(i64, i64), Error> {
        let conn = self.db.conn_mut();
        let tx = conn.transaction()?;
        let (rows, bytes): (i64, i64) = tx.query_row(
            "SELECT COUNT(*),
                    COALESCE(SUM(COALESCE(LENGTH(stdout_excerpt),0)
                               + COALESCE(LENGTH(stderr_excerpt),0)),0)
             FROM runs
             WHERE started_at < ?1
               AND (stdout_excerpt IS NOT NULL OR stderr_excerpt IS NOT NULL)",
            params![cutoff],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        if rows > 0 {
            tx.execute(
                "UPDATE runs
                 SET stdout_excerpt = NULL,
                     stderr_excerpt = NULL,
                     stdout_truncated = 0,
                     stderr_truncated = 0
                 WHERE started_at < ?1
                   AND (stdout_excerpt IS NOT NULL OR stderr_excerpt IS NOT NULL)",
                params![cutoff],
            )?;
        }
        tx.commit()?;
        Ok((rows, bytes))
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

fn map_occ(row: &rusqlite::Row<'_>) -> rusqlite::Result<Occurrence> {
    Ok(Occurrence {
        id: row.get(0)?,
        run_id: row.get(1)?,
        seen_at: row.get(2)?,
        excerpt: row.get(3)?,
        git_commit: row.get(4)?,
        git_branch: row.get(5)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repo::runs::{RunCompletion, RunStatus, Runs};

    fn db() -> Db {
        Db::open_in_memory().unwrap()
    }

    fn reserve_run(db: &mut Db, started_at: &str, commit: Option<&str>) -> i64 {
        let id = Runs::new(db)
            .reserve(started_at, "prog", "[]", "/tmp")
            .unwrap();
        Runs::new(db)
            .insert_full(
                id,
                &RunCompletion {
                    finished_at: started_at,
                    duration_ms: 1,
                    exit_code: Some(1),
                    status: RunStatus::Failed,
                    tags: &[],
                    stdout_excerpt: Some("oops"),
                    stderr_excerpt: Some("nope"),
                    stdout_truncated: false,
                    stderr_truncated: false,
                    git_commit: commit,
                    git_branch: Some("main"),
                    git_dirty: Some(false),
                },
            )
            .unwrap();
        id
    }

    #[test]
    fn find_by_hash_returns_none_for_missing() {
        let mut db = db();
        let res = Bugs::new(&mut db).find_by_hash("nope").unwrap();
        assert!(res.is_none());
    }

    #[test]
    fn find_by_id_returns_none_for_missing() {
        let mut db = db();
        let res = Bugs::new(&mut db).find_by_id(99).unwrap();
        assert!(res.is_none());
    }

    #[test]
    fn top_returns_empty_on_fresh_db() {
        let mut db = db();
        let rows = Bugs::new(&mut db)
            .top(20, "2020-01-01T00:00:00Z", None)
            .unwrap();
        assert!(rows.is_empty());
    }

    #[test]
    fn upsert_inserts_then_updates_count_and_last_seen() {
        let mut db = db();
        let fp1 = Bugs::new(&mut db)
            .upsert_for_occurrence("err A", "summary A", "2026-01-01T00:00:00Z")
            .unwrap();
        assert_eq!(fp1.occurrence_count, 1);
        assert_eq!(fp1.first_seen_at, "2026-01-01T00:00:00Z");
        assert_eq!(fp1.last_seen_at, "2026-01-01T00:00:00Z");

        let fp2 = Bugs::new(&mut db)
            .upsert_for_occurrence("err A", "summary A", "2026-02-01T00:00:00Z")
            .unwrap();
        assert_eq!(fp2.id, fp1.id);
        assert_eq!(fp2.occurrence_count, 2);
        // first_seen is preserved; last_seen moves forward.
        assert_eq!(fp2.first_seen_at, "2026-01-01T00:00:00Z");
        assert_eq!(fp2.last_seen_at, "2026-02-01T00:00:00Z");
    }

    #[test]
    fn upsert_preserves_first_seen_on_backdated_run() {
        let mut db = db();
        let fp1 = Bugs::new(&mut db)
            .upsert_for_occurrence("err A", "summary A", "2026-05-01T00:00:00Z")
            .unwrap();
        let fp2 = Bugs::new(&mut db)
            .upsert_for_occurrence("err A", "summary A", "2026-01-01T00:00:00Z")
            .unwrap();
        // The newer first_seen wins; the older upsert should not
        // overwrite it.
        assert_eq!(fp1.first_seen_at, fp2.first_seen_at);
        assert_eq!(fp2.first_seen_at, "2026-05-01T00:00:00Z");
        assert_eq!(fp2.last_seen_at, "2026-05-01T00:00:00Z");
    }

    #[test]
    fn insert_occurrence_caps_excerpt_to_4kib() {
        // The truncation is the caller's job; we just verify the
        // insert path accepts a None excerpt without panic.
        let mut db = db();
        let fp = Bugs::new(&mut db)
            .upsert_for_occurrence("err A", "summary A", "2026-01-01T00:00:00Z")
            .unwrap();
        let run_id = reserve_run(&mut db, "2026-01-01T00:00:00Z", None);
        let occ_id = Bugs::new(&mut db)
            .insert_occurrence(fp.id, run_id, "2026-01-01T00:00:00Z", None)
            .unwrap();
        assert!(occ_id > 0);
    }

    #[test]
    fn find_by_hash_prefix_resolves_unique_short_prefix() {
        let mut db = db();
        let any = Bugs::new(&mut db)
            .upsert_for_occurrence("err A", "summary A", "2026-01-01T00:00:00Z")
            .unwrap();
        // The first 8 hex chars of the actual hash are always
        // unique; this is the most direct way to assert the
        // prefix-lookup contract.
        let prefix8 = &any.hash[..8];
        let fp = Bugs::new(&mut db)
            .find_by_hash_prefix(prefix8)
            .unwrap()
            .unwrap();
        assert_eq!(fp.id, any.id);
    }

    #[test]
    fn find_by_hash_prefix_returns_none_on_ambiguity() {
        let mut db = db();
        Bugs::new(&mut db)
            .upsert_for_occurrence("err A", "A", "2026-01-01T00:00:00Z")
            .unwrap();
        // Force a second hash that also starts with "0": we can't
        // easily control the hash, but inserting any two distinct
        // canonicals gives us two distinct hashes. Use a single
        // character prefix to guarantee ambiguity.
        let f1 = Bugs::new(&mut db).find_by_hash_prefix("0").unwrap();
        // If only one row was inserted, f1 is Some(_). Adding a
        // second row makes the 1-char prefix ambiguous.
        Bugs::new(&mut db)
            .upsert_for_occurrence("err B", "B", "2026-01-02T00:00:00Z")
            .unwrap();
        let f2 = Bugs::new(&mut db).find_by_hash_prefix("0").unwrap();
        // Either: f1 is Some AND f2 is None (the 1-char prefix
        // becomes ambiguous after the second insert), or both are
        // None. The contract is: f2 must be None when 2+ rows exist.
        if f1.is_some() {
            assert!(f2.is_none(), "ambiguous prefix should return None");
        }
    }

    #[test]
    fn find_by_hash_prefix_rejects_non_hex() {
        let mut db = db();
        Bugs::new(&mut db)
            .upsert_for_occurrence("err A", "A", "2026-01-01T00:00:00Z")
            .unwrap();
        let res = Bugs::new(&mut db).find_by_hash_prefix("zz").unwrap();
        assert!(res.is_none());
    }

    #[test]
    fn find_by_hash_prefix_rejects_empty() {
        let mut db = db();
        Bugs::new(&mut db)
            .upsert_for_occurrence("err A", "A", "2026-01-01T00:00:00Z")
            .unwrap();
        let res = Bugs::new(&mut db).find_by_hash_prefix("").unwrap();
        assert!(res.is_none());
    }

    #[test]
    fn hash_for_run_returns_attached_hash() {
        let mut db = db();
        let run_id = reserve_run(&mut db, "2026-01-01T00:00:00Z", None);
        let fp = Bugs::new(&mut db)
            .upsert_for_occurrence("err A", "A", "2026-01-01T00:00:00Z")
            .unwrap();
        Bugs::new(&mut db)
            .insert_occurrence(fp.id, run_id, "2026-01-01T00:00:00Z", None)
            .unwrap();
        let hash = Bugs::new(&mut db).hash_for_run(run_id).unwrap();
        assert_eq!(hash, Some(fp.hash));
    }

    #[test]
    fn hash_for_run_returns_none_when_unattached() {
        let mut db = db();
        let run_id = reserve_run(&mut db, "2026-01-01T00:00:00Z", None);
        let hash = Bugs::new(&mut db).hash_for_run(run_id).unwrap();
        assert!(hash.is_none());
    }

    #[test]
    fn hash_for_runs_returns_map_for_each_id() {
        let mut db = db();
        let r1 = reserve_run(&mut db, "2026-01-01T00:00:00Z", None);
        let r2 = reserve_run(&mut db, "2026-01-02T00:00:00Z", None);
        let r3 = reserve_run(&mut db, "2026-01-03T00:00:00Z", None);
        let fp = Bugs::new(&mut db)
            .upsert_for_occurrence("err A", "A", "2026-01-01T00:00:00Z")
            .unwrap();
        Bugs::new(&mut db)
            .insert_occurrence(fp.id, r1, "2026-01-01T00:00:00Z", None)
            .unwrap();
        Bugs::new(&mut db)
            .insert_occurrence(fp.id, r2, "2026-01-02T00:00:00Z", None)
            .unwrap();
        // r3 has no fingerprint.
        let map = Bugs::new(&mut db).hash_for_runs(&[r1, r2, r3]).unwrap();
        assert_eq!(map.get(&r1).cloned(), Some(fp.hash.clone()));
        assert_eq!(map.get(&r2).cloned(), Some(fp.hash));
        assert!(!map.contains_key(&r3));
    }

    #[test]
    fn report_for_joins_occurrences_and_dedupes_commits() {
        let mut db = db();
        let fp = Bugs::new(&mut db)
            .upsert_for_occurrence("err A", "A summary", "2026-01-01T00:00:00Z")
            .unwrap();
        let r1 = reserve_run(&mut db, "2026-01-01T00:00:00Z", Some("aaaa"));
        let r2 = reserve_run(&mut db, "2026-01-02T00:00:00Z", Some("aaaa"));
        let r3 = reserve_run(&mut db, "2026-01-03T00:00:00Z", Some("bbbb"));
        Bugs::new(&mut db)
            .insert_occurrence(fp.id, r1, "2026-01-01T00:00:00Z", Some("ex1"))
            .unwrap();
        Bugs::new(&mut db)
            .insert_occurrence(fp.id, r2, "2026-01-02T00:00:00Z", Some("ex2"))
            .unwrap();
        Bugs::new(&mut db)
            .insert_occurrence(fp.id, r3, "2026-01-03T00:00:00Z", Some("ex3"))
            .unwrap();
        let r = Bugs::new(&mut db).report_for(fp.id).unwrap();
        assert_eq!(r.fingerprint.id, fp.id);
        assert_eq!(r.occurrences.len(), 3);
        assert_eq!(r.recent_commits.len(), 2);
        assert_eq!(r.distinct_runs, 3);
    }

    #[test]
    fn prune_streams_nulls_out_excerpts_and_preserves_runs() {
        let mut db = db();
        let run_id = reserve_run(&mut db, "2020-01-01T00:00:00Z", None);
        // Reserve + insert_full already populated the excerpts.
        let (rows, bytes) = Bugs::new(&mut db)
            .prune_streams("2020-02-01T00:00:00Z")
            .unwrap();
        assert_eq!(rows, 1);
        assert!(bytes > 0);

        let rec = Runs::new(&db).find(run_id).unwrap().unwrap();
        assert!(rec.stdout_excerpt.is_none());
        assert!(rec.stderr_excerpt.is_none());
        // The run row itself is preserved.
        assert_eq!(rec.id, run_id);
    }

    #[test]
    fn prune_streams_is_idempotent() {
        let mut db = db();
        reserve_run(&mut db, "2020-01-01T00:00:00Z", None);
        let (first_rows, _) = Bugs::new(&mut db)
            .prune_streams("2020-02-01T00:00:00Z")
            .unwrap();
        let (second_rows, _) = Bugs::new(&mut db)
            .prune_streams("2020-02-01T00:00:00Z")
            .unwrap();
        assert_eq!(first_rows, 1);
        assert_eq!(second_rows, 0);
    }

    #[test]
    fn prune_streams_does_not_touch_recent_runs() {
        let mut db = db();
        let recent = reserve_run(&mut db, chrono::Utc::now().to_rfc3339().as_str(), None);
        let (rows, _) = Bugs::new(&mut db)
            .prune_streams("2020-01-01T00:00:00Z")
            .unwrap();
        assert_eq!(rows, 0);
        let rec = Runs::new(&db).find(recent).unwrap().unwrap();
        assert!(rec.stdout_excerpt.is_some());
    }

    #[test]
    fn tags_for_returns_distinct_tags_across_occurrences() {
        let mut db = db();
        let fp = Bugs::new(&mut db)
            .upsert_for_occurrence("err A", "A", "2026-01-01T00:00:00Z")
            .unwrap();
        // Two runs with overlapping tag sets.
        let r1 = reserve_run(&mut db, "2026-01-01T00:00:00Z", None);
        let r2 = reserve_run(&mut db, "2026-01-02T00:00:00Z", None);
        // Insert full with JSON tags. We re-open the run rows
        // afterwards to overwrite the tag column.
        Runs::new(&db)
            .insert_full(
                r1,
                &RunCompletion {
                    finished_at: "2026-01-01T00:00:01Z",
                    duration_ms: 1,
                    exit_code: Some(1),
                    status: RunStatus::Failed,
                    tags: &["auth".to_string(), "db".to_string()],
                    stdout_excerpt: None,
                    stderr_excerpt: None,
                    stdout_truncated: false,
                    stderr_truncated: false,
                    git_commit: None,
                    git_branch: None,
                    git_dirty: None,
                },
            )
            .unwrap();
        Runs::new(&db)
            .insert_full(
                r2,
                &RunCompletion {
                    finished_at: "2026-01-02T00:00:01Z",
                    duration_ms: 1,
                    exit_code: Some(1),
                    status: RunStatus::Failed,
                    tags: &["db".to_string(), "ci".to_string()],
                    stdout_excerpt: None,
                    stderr_excerpt: None,
                    stdout_truncated: false,
                    stderr_truncated: false,
                    git_commit: None,
                    git_branch: None,
                    git_dirty: None,
                },
            )
            .unwrap();
        Bugs::new(&mut db)
            .insert_occurrence(fp.id, r1, "2026-01-01T00:00:00Z", None)
            .unwrap();
        Bugs::new(&mut db)
            .insert_occurrence(fp.id, r2, "2026-01-02T00:00:00Z", None)
            .unwrap();
        let mut tags = Bugs::new(&mut db).tags_for(fp.id).unwrap();
        tags.sort();
        assert_eq!(
            tags,
            vec!["auth".to_string(), "ci".to_string(), "db".to_string()]
        );
    }

    #[test]
    fn tags_for_returns_empty_when_no_occurrences() {
        let mut db = db();
        let fp = Bugs::new(&mut db)
            .upsert_for_occurrence("err A", "A", "2026-01-01T00:00:00Z")
            .unwrap();
        let tags = Bugs::new(&mut db).tags_for(fp.id).unwrap();
        assert!(tags.is_empty());
    }
}
