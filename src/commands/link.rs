//! `driftwatch link bug:<id> spec:<alert-id> [--note "..."]`
//!
//! Resolves both sides of the link and persists one `manual_links`
//! row. The bug side accepts the `bug:<id>` form, a bare hash
//! prefix (8+ hex chars preferred), or a numeric `fingerprints.id`.
//! The spec side accepts `spec:<id>` or a numeric
//! `drift_alerts.id`. Mismatches return `Error::LinkTarget` with a
//! message identifying which side failed.

use std::path::Path;

use crate::cli::LinkArgs;
use crate::error::Error;
use crate::project::ProjectRoot;
use crate::repo::{bugs::Bugs, links::Links, Db};

const EXIT_OK: i32 = 0;

/// Execute `driftwatch link`. Returns 0 on success.
pub fn link(args: LinkArgs, cwd: &Path) -> Result<i32, Error> {
    let proj = ProjectRoot::discover(cwd)?;
    let mut db = Db::open(&proj.db_path)?;

    let fp_id = resolve_bug(&mut db, &args.bug)?;
    let alert_id = resolve_alert(&db, &args.spec)?;

    let now = chrono::Utc::now().to_rfc3339();
    let link = Links::create(
        &mut db,
        Some(fp_id),
        Some(alert_id),
        args.note.as_deref(),
        &now,
    )?;
    println!(
        "driftwatch: linked bug #{} to alert #{} as manual link #{}",
        fp_id, alert_id, link.id
    );
    if let Some(note) = &link.note {
        println!("  note: {note}");
    }
    Ok(EXIT_OK)
}

fn resolve_bug(db: &mut Db, raw: &str) -> Result<i64, Error> {
    let trimmed = raw.trim();
    let stripped = strip_kind(trimmed, "bug");
    // Explicit `id:<n>` always means the numeric primary key.
    if let Some(rest) = stripped.strip_prefix("id:") {
        let n: i64 = rest.trim().parse().map_err(|_| Error::LinkTarget {
            side: "bug",
            raw: trimmed.to_string(),
        })?;
        let bugs = Bugs::new(db);
        if let Some(fp) = bugs.find_by_id(n)? {
            return Ok(fp.id);
        }
        return Err(Error::LinkTarget {
            side: "bug",
            raw: trimmed.to_string(),
        });
    }
    let bugs = Bugs::new(db);
    // Hash prefix first so digit-only strings never silently resolve
    // to the wrong numeric row. Ambiguity errors propagate.
    match bugs.find_by_hash_prefix(stripped)? {
        Some(fp) => Ok(fp.id),
        None => {
            // Fall back to the numeric id only when no hash matches
            // and the input is all digits.
            if !stripped.is_empty() && stripped.chars().all(|c| c.is_ascii_digit()) {
                if let Ok(n) = stripped.parse::<i64>() {
                    if let Some(fp) = bugs.find_by_id(n)? {
                        return Ok(fp.id);
                    }
                }
            }
            Err(Error::LinkTarget {
                side: "bug",
                raw: trimmed.to_string(),
            })
        }
    }
}

fn resolve_alert(db: &Db, raw: &str) -> Result<i64, Error> {
    let trimmed = raw.trim();
    let stripped = strip_kind(trimmed, "spec");
    let n: i64 = stripped.parse().map_err(|_| Error::LinkTarget {
        side: "spec",
        raw: trimmed.to_string(),
    })?;
    // Confirm the alert exists; otherwise surface a clear error.
    let exists: bool = db
        .conn()
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM drift_alerts WHERE id = ?1)",
            rusqlite::params![n],
            |r| {
                let v: i64 = r.get(0)?;
                Ok(v != 0)
            },
        )
        .map_err(Error::Sqlite)?;
    if exists {
        Ok(n)
    } else {
        Err(Error::LinkTarget {
            side: "spec",
            raw: trimmed.to_string(),
        })
    }
}

fn strip_kind<'a>(raw: &'a str, kind: &str) -> &'a str {
    let prefix = format!("{kind}:");
    if let Some(rest) = raw.strip_prefix(&prefix) {
        rest
    } else {
        raw
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repo::alerts::{Alerts, NewAlert, NewSnapshot};

    fn db() -> Db {
        Db::open_in_memory().unwrap()
    }

    fn insert_fingerprint_and_alert(db: &mut Db) -> (i64, i64) {
        use crate::repo::bugs::Bugs;
        let fp = Bugs::new(db)
            .upsert_for_occurrence("err A", "A", "2026-01-01T00:00:00Z")
            .unwrap();
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
        let alert_id: i64 = db
            .conn()
            .query_row("SELECT id FROM drift_alerts LIMIT 1", [], |r| r.get(0))
            .unwrap();
        (fp.id, alert_id)
    }

    #[test]
    fn strip_kind_handles_known_and_unknown_prefix() {
        assert_eq!(strip_kind("bug:abc", "bug"), "abc");
        assert_eq!(strip_kind("abc", "bug"), "abc");
        assert_eq!(strip_kind("spec:42", "bug"), "spec:42");
    }

    #[test]
    fn resolve_bug_accepts_bare_hash_prefix() {
        let mut db = db();
        let (fp_id, _alert_id) = insert_fingerprint_and_alert(&mut db);
        // Use the actual hash of the fingerprint row.
        let hash: String = db
            .conn()
            .query_row(
                "SELECT hash FROM fingerprints WHERE id = ?1",
                rusqlite::params![fp_id],
                |r| r.get(0),
            )
            .unwrap();
        let prefix8 = &hash[..8];
        let resolved = resolve_bug(&mut db, prefix8).unwrap();
        assert_eq!(resolved, fp_id);
    }

    #[test]
    fn resolve_bug_rejects_unknown_id() {
        let mut db = db();
        let err = resolve_bug(&mut db, "nonexistent").unwrap_err();
        assert!(matches!(err, Error::LinkTarget { side: "bug", .. }));
    }

    #[test]
    fn resolve_alert_accepts_numeric_id() {
        let mut db = db();
        let (_fp_id, alert_id) = insert_fingerprint_and_alert(&mut db);
        let resolved = resolve_alert(&db, &alert_id.to_string()).unwrap();
        assert_eq!(resolved, alert_id);
    }

    #[test]
    fn resolve_alert_strips_spec_prefix() {
        let mut db = db();
        let (_fp_id, alert_id) = insert_fingerprint_and_alert(&mut db);
        let resolved = resolve_alert(&db, &format!("spec:{alert_id}")).unwrap();
        assert_eq!(resolved, alert_id);
    }

    #[test]
    fn resolve_alert_rejects_missing_id() {
        let db = db();
        let err = resolve_alert(&db, "9999").unwrap_err();
        assert!(matches!(err, Error::LinkTarget { side: "spec", .. }));
    }

    #[test]
    fn resolve_bug_prefers_hash_over_numeric_id_for_digit_strings() {
        // Craft two fingerprints: id=1 with a non-numeric hash, and a
        // second row whose hash starts with digits. A bare digit-only
        // ref that matches the hash prefix must resolve to the hash
        // row, never silently to the numeric id.
        let mut db = db();
        let fp1 = crate::repo::bugs::Bugs::new(&mut db)
            .upsert_for_occurrence("err one", "one", "2026-01-01T00:00:00Z")
            .unwrap();
        // Insert a row with a digit-leading hash directly.
        db.conn_mut()
            .execute(
                "INSERT INTO fingerprints (hash, canonical, summary, first_seen_at, last_seen_at, occurrence_count)
                 VALUES ('12345678abcdef12345678abcdef12345678abcdef12345678abcdef1234', 'err two', 'two', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z', 1)",
                [],
            )
            .unwrap();
        let fp2_id: i64 = db
            .conn()
            .query_row(
                "SELECT id FROM fingerprints WHERE canonical = 'err two'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_ne!(fp1.id, fp2_id);
        // "12345678" is all digits AND the hash prefix of fp2.
        let resolved = resolve_bug(&mut db, "12345678").unwrap();
        assert_eq!(resolved, fp2_id);
        // Explicit `id:` still reaches the numeric row.
        let via_id = resolve_bug(&mut db, &format!("id:{}", fp1.id)).unwrap();
        assert_eq!(via_id, fp1.id);
    }
}
