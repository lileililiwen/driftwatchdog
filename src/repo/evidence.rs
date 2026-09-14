//! `gate_artifacts` repository.
//!
//! Populated by the `evidence-and-artifacts` change. Rows are identity
//! records: cleanup deletes bulky files but keeps the row (with
//! `available = 0`), so every reference stays auditable with an
//! unavailable-content marker. Gate result identity (gate id, status,
//! producer, digest) rides on the artifact row until the later
//! `gate-cli-and-memory-integration` change persists full results.

use std::path::Path;

use rusqlite::{params, OptionalExtension};

use crate::error::Error;
use crate::gate::evidence::{ArtifactKind, ArtifactRecord};
use crate::repo::Db;

pub struct Artifacts<'a> {
    db: &'a Db,
}

impl<'a> Artifacts<'a> {
    pub fn new(db: &'a Db) -> Self {
        Self { db }
    }

    /// Number of artifact rows (available or pruned).
    pub fn count(&self) -> Result<i64, Error> {
        let n: i64 = self
            .db
            .conn()
            .query_row("SELECT COUNT(*) FROM gate_artifacts", [], |r| r.get(0))?;
        Ok(n)
    }

    /// Insert `record`, or return the existing row when `key` is already
    /// present (idempotent re-store; concurrent writers serialize on the
    /// `UNIQUE(key)` constraint and read back the winner).
    pub fn insert_or_get(&self, record: &ArtifactRecord) -> Result<ArtifactRecord, Error> {
        let kind = record.kind.as_str();
        let redacted: i64 = i64::from(record.redacted);
        let available: i64 = i64::from(record.available);
        let inserted = self.db.conn().execute(
            "INSERT OR IGNORE INTO gate_artifacts
                 (key, kind, producer, producer_version, created_at, byte_size,
                  digest, media_type, rel_path, redacted, available, preview)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                record.key,
                kind,
                record.producer,
                record.producer_version,
                record.created_at,
                record.byte_size as i64,
                record.digest,
                record.media_type,
                record.rel_path,
                redacted,
                available,
                record.preview,
            ],
        )?;
        if inserted == 1 {
            let id = self.db.conn().last_insert_rowid();
            return self.get_by_id(id).map(|r| r.expect("just inserted"));
        }
        self.get_by_key(&record.key)
            .map(|r| r.expect("key conflict implies row exists"))
    }

    /// Fetch one artifact by key, if present.
    pub fn get_by_key(&self, key: &str) -> Result<Option<ArtifactRecord>, Error> {
        let mut stmt = self.db.conn().prepare(
            "SELECT id, key, kind, producer, producer_version, created_at, byte_size,
                    digest, media_type, rel_path, redacted, available, preview
             FROM gate_artifacts WHERE key = ?1",
        )?;
        let row = stmt.query_row(params![key], row_to_record).optional()?;
        Ok(row)
    }

    fn get_by_id(&self, id: i64) -> Result<Option<ArtifactRecord>, Error> {
        let mut stmt = self.db.conn().prepare(
            "SELECT id, key, kind, producer, producer_version, created_at, byte_size,
                    digest, media_type, rel_path, redacted, available, preview
             FROM gate_artifacts WHERE id = ?1",
        )?;
        let row = stmt.query_row(params![id], row_to_record).optional()?;
        Ok(row)
    }

    /// Every artifact row, oldest first. Used by export.
    pub fn list_all(&self) -> Result<Vec<ArtifactRecord>, Error> {
        self.list_all_with_ids()
            .map(|rows| rows.into_iter().map(|(_, rec)| rec).collect())
    }

    /// Every artifact row with its row id, oldest first. Used by export
    /// where the DTO carries a stable numeric id.
    pub fn list_all_with_ids(&self) -> Result<Vec<(i64, ArtifactRecord)>, Error> {
        let mut stmt = self.db.conn().prepare(
            "SELECT id, key, kind, producer, producer_version, created_at, byte_size,
                    digest, media_type, rel_path, redacted, available, preview
             FROM gate_artifacts ORDER BY id",
        )?;
        let rows = stmt
            .query_map([], |r| {
                let id: i64 = r.get(0)?;
                row_to_record(r).map(|rec| (id, rec))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// Mark one artifact unavailable without deleting its identity row
    /// (used when the file is already gone or was never stored).
    pub fn mark_unavailable(&self, key: &str) -> Result<bool, Error> {
        let n = self.db.conn().execute(
            "UPDATE gate_artifacts SET available = 0, rel_path = NULL, preview = NULL
             WHERE key = ?1",
            params![key],
        )?;
        Ok(n == 1)
    }

    /// Retention cleanup: for rows created before `cutoff` (RFC 3339)
    /// that still claim content, delete the file under `state_dir` and
    /// flip the row to unavailable. Identity rows (key, producer,
    /// digest, preview marker) are never deleted. Returns
    /// `(files_removed, bytes_freed)`; a missing file still flips the
    /// row and counts as removed.
    pub fn prune_before(&self, state_dir: &Path, cutoff: &str) -> Result<(i64, i64), Error> {
        let mut stmt = self.db.conn().prepare(
            "SELECT key, byte_size, rel_path FROM gate_artifacts
             WHERE created_at < ?1 AND available != 0 AND rel_path IS NOT NULL",
        )?;
        let targets: Vec<(String, i64, String)> = stmt
            .query_map(params![cutoff], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, String>(2)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        drop(stmt);
        let mut removed = 0i64;
        let mut bytes = 0i64;
        for (key, byte_size, rel) in targets {
            let path = state_dir.join(&rel);
            // Only delete inside the state dir; a row pointing elsewhere
            // (legacy or corrupt) is flipped without touching the disk.
            let confined = path.starts_with(state_dir);
            if confined {
                match std::fs::remove_file(&path) {
                    Ok(()) => {}
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                    Err(e) => {
                        return Err(Error::io(path.clone(), e));
                    }
                }
            }
            self.mark_unavailable(&key)?;
            removed += 1;
            bytes += byte_size;
        }
        Ok((removed, bytes))
    }
}

fn row_to_record(r: &rusqlite::Row<'_>) -> rusqlite::Result<ArtifactRecord> {
    let kind_str: String = r.get(2)?;
    let kind = ArtifactKind::parse(&kind_str).unwrap_or(ArtifactKind::File);
    let redacted: i64 = r.get(10)?;
    let available: i64 = r.get(11)?;
    Ok(ArtifactRecord {
        key: r.get(1)?,
        kind,
        producer: r.get(3)?,
        producer_version: r.get(4)?,
        created_at: r.get(5)?,
        byte_size: r.get::<_, i64>(6)? as u64,
        digest: r.get(7)?,
        media_type: r.get(8)?,
        rel_path: r.get(9)?,
        redacted: redacted != 0,
        available: available != 0,
        preview: r.get(12)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gate::evidence::{digest_bytes, NewArtifact};

    fn record(key: &str) -> ArtifactRecord {
        ArtifactRecord {
            key: key.into(),
            kind: ArtifactKind::ToolJson,
            producer: "osv".into(),
            producer_version: Some("1.9".into()),
            created_at: "2020-01-01T00:00:00Z".into(),
            byte_size: 12,
            digest: digest_bytes(b"hello world!"),
            media_type: Some("application/json".into()),
            rel_path: Some("artifacts/tool_json-abc.json".into()),
            redacted: true,
            available: true,
            preview: Some("{}".into()),
        }
    }

    #[test]
    fn insert_get_and_list_round_trip() {
        let db = Db::open_in_memory().unwrap();
        let repo = Artifacts::new(&db);
        let back = repo.insert_or_get(&record("a")).unwrap();
        assert_eq!(back, record("a"));
        assert_eq!(repo.count().unwrap(), 1);
        assert_eq!(repo.list_all().unwrap().len(), 1);
        assert!(repo.get_by_key("missing").unwrap().is_none());
    }

    #[test]
    fn insert_same_key_is_idempotent() {
        let db = Db::open_in_memory().unwrap();
        let repo = Artifacts::new(&db);
        let first = repo.insert_or_get(&record("dup")).unwrap();
        let mut changed = record("dup");
        changed.producer = "other".into();
        let second = repo.insert_or_get(&changed).unwrap();
        // First writer wins; the row is unchanged.
        assert_eq!(second, first);
        assert_eq!(repo.count().unwrap(), 1);
    }

    #[test]
    fn unavailable_marker_row_round_trips() {
        let db = Db::open_in_memory().unwrap();
        let repo = Artifacts::new(&db);
        let marker = ArtifactRecord::unavailable(
            "k",
            ArtifactKind::Sarif,
            "semgrep",
            "2020-01-01T00:00:00Z",
        );
        let back = repo.insert_or_get(&marker).unwrap();
        assert_eq!(back, marker);
        assert!(!back.available);
    }

    #[test]
    fn new_artifact_helper_builds_storable_record() {
        // `NewArtifact` (domain input) converts through the shared
        // builder; the repo only sees the resulting record.
        let tmp = tempfile::tempdir().unwrap();
        let state = tmp.path().join(".driftwatch");
        let new = NewArtifact {
            key: "gate/out",
            kind: ArtifactKind::CommandOutput,
            producer: "probe",
            producer_version: None,
            media_type: None,
            sensitive: vec![],
            created_at: Some("2020-01-01T00:00:00Z"),
        };
        let now = "2026-09-14T00:00:00Z";
        let (rec, _) = crate::gate::evidence::build_record(&state, &new, b"hi", now).unwrap();
        let db = Db::open_in_memory().unwrap();
        let back = Artifacts::new(&db).insert_or_get(&rec).unwrap();
        assert_eq!(back.key, "gate/out");
    }

    #[test]
    fn prune_removes_file_but_keeps_identity_row() {
        let tmp = tempfile::tempdir().unwrap();
        let state = tmp.path().join(".driftwatch");
        let rec = crate::gate::evidence::store_bytes(
            &state,
            &NewArtifact {
                key: "old/out",
                kind: ArtifactKind::CommandOutput,
                producer: "probe",
                producer_version: None,
                media_type: None,
                sensitive: vec![],
                created_at: Some("2020-01-01T00:00:00Z"),
            },
            b"old bytes",
        )
        .unwrap();
        let rel = rec.rel_path.clone().unwrap();
        assert!(state.join(&rel).exists());
        let db = Db::open_in_memory().unwrap();
        Artifacts::new(&db).insert_or_get(&rec).unwrap();
        let (removed, bytes) = Artifacts::new(&db)
            .prune_before(&state, "2026-01-01T00:00:00Z")
            .unwrap();
        assert_eq!(removed, 1);
        assert_eq!(bytes, 9);
        assert!(!state.join(&rel).exists());
        // Identity row survives with digest + unavailable marker.
        let back = Artifacts::new(&db).get_by_key("old/out").unwrap().unwrap();
        assert!(!back.available);
        assert!(back.digest.starts_with("sha256:"));
        assert_eq!(back.producer, "probe");
        assert_eq!(
            back.safe_preview(),
            crate::gate::evidence::UNAVAILABLE_PREVIEW
        );
        // Second prune is a no-op.
        let (removed2, _) = Artifacts::new(&db)
            .prune_before(&state, "2026-01-01T00:00:00Z")
            .unwrap();
        assert_eq!(removed2, 0);
    }

    #[test]
    fn prune_skips_recent_rows() {
        let tmp = tempfile::tempdir().unwrap();
        let state = tmp.path().join(".driftwatch");
        let db = Db::open_in_memory().unwrap();
        let mut fresh = record("fresh");
        fresh.created_at = chrono::Utc::now().to_rfc3339();
        Artifacts::new(&db).insert_or_get(&fresh).unwrap();
        let (removed, _) = Artifacts::new(&db)
            .prune_before(&state, "2020-01-01T00:00:00Z")
            .unwrap();
        assert_eq!(removed, 0);
        assert!(
            Artifacts::new(&db)
                .get_by_key("fresh")
                .unwrap()
                .unwrap()
                .available
        );
    }
}
