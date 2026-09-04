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
/// current alert set". Returns the number of inserted rows.
pub fn run_after_check(db: &mut Db) -> Result<usize, Error> {
    let now = chrono::Utc::now().to_rfc3339();
    let fingerprints = Bugs::new(db).current_fingerprints()?;
    let alerts = Alerts::new(db).current_alerts()?;
    if fingerprints.is_empty() || alerts.is_empty() {
        // Nothing to correlate. Existing rows are intentionally
        // left in place so a transient empty window (e.g. the
        // first run after a GC) does not silently delete
        // historical context.
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
    // call touches one fingerprint's worth of rows.
    let mut by_fingerprint: HashMap<i64, Vec<(i64, ComponentScores)>> = HashMap::new();
    for (fp_id, alert_id, scores) in pairs {
        by_fingerprint
            .entry(fp_id)
            .or_default()
            .push((alert_id, scores));
    }
    for (fp_id, slice) in &by_fingerprint {
        inserted += Correlations::replace_for_fingerprint(db, *fp_id, slice, &now)?;
    }
    Ok(inserted)
}
