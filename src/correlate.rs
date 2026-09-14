//! Correlate after a `driftwatch check` run.
//!
//! Loads the current fingerprint and alert sets, runs the
//! [`similarity::candidates::generate`] pipeline, and persists the
//! surviving pairs via [`Correlations::replace_for_fingerprint`].
//!
//! Errors are surfaced as warnings (returned through `Err`) but
//! callers (notably `commands/check`) must not let a correlation
//! failure abort the check. The function is intentionally small
//! and side-effect-isolated so a future change can move the
//! trigger to a separate subcommand without rewiring callers.

use std::collections::HashMap;

use crate::error::Error;
use crate::repo::{alerts::Alerts, bugs::Bugs, correlations::Correlations, Db};
use crate::similarity::{candidates, score::ComponentScores};

/// Recompute heuristic correlations for every fingerprint. Replaces
/// the existing correlation rows so the persisted set always
/// reflects "what currently passes the threshold against the
/// current alert set". Stale rows are cleared even when pairs stop
/// passing or the alert set is empty; manual links (a separate
/// table) are never touched. Returns the number of inserted rows.
pub fn run_after_check(db: &mut Db) -> Result<usize, Error> {
    let now = chrono::Utc::now().to_rfc3339();
    let fingerprints = Bugs::new(db).current_fingerprints()?;
    if fingerprints.is_empty() {
        return Ok(0);
    }
    let alerts = Alerts::new(db).current_alerts()?;
    if alerts.is_empty() {
        // No alerts: clear every heuristic row so `report --ai`
        // stops showing stale relationships. Manual links live in
        // `manual_links` and are preserved.
        let mut cleared = 0usize;
        for fp in &fingerprints {
            cleared += Correlations::replace_for_fingerprint(db, fp.id, &[], &now)?;
        }
        let _ = cleared;
        return Ok(0);
    }
    // Build the `fingerprint_id -> tags` map once. The inner
    // `Bugs::new` borrows `db` mutably; do this in two phases so
    // the immutable borrow is released before we call mutating
    // methods below.
    let bug_tags: HashMap<i64, Vec<String>> = {
        let bugs = Bugs::new(db);
        let mut map = HashMap::new();
        for fp in &fingerprints {
            map.insert(fp.id, bugs.tags_for(fp.id)?);
        }
        map
    };
    let pairs = candidates::generate(&fingerprints, &alerts, &bug_tags);
    let mut inserted = 0usize;
    // Group pairs by fingerprint id so each `replace_for_fingerprint`
    // call touches one fingerprint's worth of rows. Every
    // fingerprint gets a clear-then-write (possibly empty) so stale
    // rows disappear when pairs stop passing.
    let mut by_fingerprint: HashMap<i64, Vec<(i64, ComponentScores)>> = HashMap::new();
    for (fp_id, alert_id, scores) in pairs {
        by_fingerprint
            .entry(fp_id)
            .or_default()
            .push((alert_id, scores));
    }
    for fp in &fingerprints {
        let empty: Vec<(i64, ComponentScores)> = Vec::new();
        let slice = by_fingerprint.get(&fp.id).unwrap_or(&empty);
        inserted += Correlations::replace_for_fingerprint(db, fp.id, slice, &now)?;
    }
    Ok(inserted)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repo::alerts::{Alerts, NewAlert, NewSnapshot};
    use crate::repo::bugs::Bugs;
    use crate::repo::links::Links;

    fn seed_fp(db: &mut Db, canonical: &str) -> i64 {
        Bugs::new(db)
            .upsert_for_occurrence(canonical, canonical, "2026-01-01T00:00:00Z")
            .unwrap()
            .id
    }

    fn seed_alert(db: &mut Db, message: &str, symbol: Option<&str>, source: Option<&str>) -> i64 {
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
                message,
                source,
                symbol,
            }],
        )
        .unwrap();
        db.conn()
            .query_row(
                "SELECT id FROM drift_alerts ORDER BY id DESC LIMIT 1",
                [],
                |r| r.get(0),
            )
            .unwrap()
    }

    #[test]
    fn stale_rows_clear_when_pairs_stop_passing() {
        let mut db = Db::open_in_memory().unwrap();
        let fp1 = seed_fp(
            &mut db,
            "DbPool connection refused to db.md while reading pool",
        );
        let fp2 = seed_fp(&mut db, "fonts render slowly on tuesdays with_gamma");
        let alert_id = seed_alert(
            &mut db,
            "DbPool connection refused to db.md while reading pool",
            Some("DbPool"),
            Some("specs/db.md"),
        );
        // Stale row for fp2 that no longer passes (unrelated text).
        Correlations::new(&db)
            .upsert(
                fp2,
                alert_id,
                crate::similarity::score::ComponentScores {
                    message: 0.1,
                    symbol: 0.0,
                    file: 0.0,
                    tag: 0.0,
                    total: 0.1,
                },
                "2026-01-01T00:00:00Z",
            )
            .unwrap();
        assert_eq!(Correlations::new(&db).count().unwrap(), 1);
        let n = run_after_check(&mut db).unwrap();
        assert!(n >= 1, "expected a correlation, got {n}");
        let all = Correlations::new(&db).list_all().unwrap();
        // fp1's fresh pair persists; fp2's stale row is gone.
        assert!(all.iter().any(|c| c.fingerprint_id == fp1));
        assert!(
            all.iter().all(|c| c.fingerprint_id != fp2),
            "stale rows must clear when pairs stop passing: {all:?}"
        );
    }

    #[test]
    fn empty_alerts_clear_heuristic_rows_but_preserve_manual_links() {
        let mut db = Db::open_in_memory().unwrap();
        let fp_id = seed_fp(
            &mut db,
            "DbPool connection refused to db.md while reading pool",
        );
        let alert_id = seed_alert(
            &mut db,
            "DbPool connection refused to db.md while reading pool",
            Some("DbPool"),
            Some("specs/db.md"),
        );
        assert!(run_after_check(&mut db).unwrap() >= 1);
        assert_eq!(Correlations::new(&db).count().unwrap(), 1);
        Links::create(
            &mut db,
            Some(fp_id),
            Some(alert_id),
            Some("keep me"),
            "2026-01-01T00:00:00Z",
        )
        .unwrap();

        // Empty the alert set without cascading (FK off) so the
        // manual link survives; the heuristic rows must still clear.
        db.conn_mut()
            .execute("PRAGMA foreign_keys=OFF", [])
            .unwrap();
        db.conn_mut()
            .execute("DELETE FROM drift_alerts", [])
            .unwrap();
        db.conn_mut()
            .execute("DELETE FROM check_snapshots", [])
            .unwrap();
        db.conn_mut().execute("PRAGMA foreign_keys=ON", []).unwrap();
        assert_eq!(run_after_check(&mut db).unwrap(), 0);
        assert_eq!(Correlations::new(&db).count().unwrap(), 0);
        assert_eq!(
            Links::new(&db).count().unwrap(),
            1,
            "manual links must survive heuristic clearing"
        );
    }
}
