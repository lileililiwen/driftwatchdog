//! Candidate generation: cross product of current fingerprints and
//! current alerts, scored, thresholded, and persisted.
//!
//! The cross product is `O(N×M)`. In the common case both N and M are
//! small (single-digit fingerprints, dozens of alerts). The
//! [`MAX_PAIRS`] cap guards against pathological cases (e.g. a
//! checker that emits thousands of alerts per snapshot); when the
//! cross product exceeds the cap, the newest alerts (highest `id`)
//! are retained so fresh drift is never dropped in favor of stale
//! rows. Output order is stable (fingerprint `id` ascending, then
//! alert `id` ascending) and the total never exceeds [`MAX_PAIRS`].

use std::collections::HashMap;

use crate::repo::{alerts::Alert, bugs::Fingerprint};

use super::score::{score_pair, AlertInput, BugInput, ComponentScores, ALGO_VERSION, THRESHOLD};

/// Maximum number of (fingerprint, alert) pairs considered per
/// generation. Sized to keep a worst-case generation under a
/// fraction of a second on commodity hardware. When exceeded, the
/// newest alerts are retained and the final output is hard-truncated
/// to this bound.
pub const MAX_PAIRS: usize = 5_000;

/// Result of [`generate`]: each entry is a (fingerprint_id, alert_id,
/// scores) triple that is at or above the score threshold and is
/// therefore eligible to be persisted.
pub type Candidate = (i64, i64, ComponentScores);

/// Score every (fingerprint, alert) pair, drop the ones below
/// [`THRESHOLD`], and return the survivors in a stable order:
/// fingerprint `id` ascending, then alert `id` ascending. The
/// returned vec never exceeds [`MAX_PAIRS`].
pub fn generate(
    bugs: &[Fingerprint],
    alerts: &[Alert],
    bug_tags: &HashMap<i64, Vec<String>>,
) -> Vec<Candidate> {
    if bugs.is_empty() || alerts.is_empty() {
        return Vec::new();
    }
    // Cap newest-first: sort a copy by `id` descending and take the
    // first `MAX_PAIRS / len(bugs)` entries. The `max(1)` prevents
    // underflow when `bugs.len()` > MAX_PAIRS.
    let alerts_to_score: Vec<&Alert> = if bugs.len() * alerts.len() > MAX_PAIRS {
        let mut sorted: Vec<&Alert> = alerts.iter().collect();
        sorted.sort_by_key(|a| std::cmp::Reverse(a.id));
        let per_bug = (MAX_PAIRS / bugs.len()).max(1);
        sorted.into_iter().take(per_bug).collect()
    } else {
        alerts.iter().collect()
    };
    let mut out = generate_with(bugs, &alerts_to_score, bug_tags);
    // True global bound: the per-bug slice keeps the inner loop
    // bounded, but truncate the output so callers never persist more
    // than MAX_PAIRS rows.
    out.truncate(MAX_PAIRS);
    out
}

fn generate_with(
    bugs: &[Fingerprint],
    alerts: &[&Alert],
    bug_tags: &HashMap<i64, Vec<String>>,
) -> Vec<Candidate> {
    let mut out: Vec<Candidate> = Vec::new();
    let mut sorted_bugs: Vec<&Fingerprint> = bugs.iter().collect();
    sorted_bugs.sort_by_key(|f| f.id);
    let mut sorted_alerts: Vec<&&Alert> = alerts.iter().collect();
    sorted_alerts.sort_by_key(|a| a.id);
    for bug in sorted_bugs {
        let tags: &[String] = bug_tags.get(&bug.id).map(Vec::as_slice).unwrap_or(&[]);
        let bi = BugInput {
            canonical: &bug.canonical,
            summary: bug.summary.as_deref().unwrap_or(""),
            tags,
        };
        for &&alert in &sorted_alerts {
            let ai = AlertInput {
                message: &alert.message,
                symbol: alert.symbol.as_deref(),
                source: alert.source.as_deref(),
            };
            let scores = score_pair(&bi, &ai);
            if scores.total >= THRESHOLD {
                out.push((bug.id, alert.id, scores));
            }
        }
    }
    out
}

/// Re-export so callers (i.e. `commands/check`) can reference the
/// algorithm version through one path.
pub fn algorithm_version() -> &'static str {
    ALGO_VERSION
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repo::alerts::Alert;

    fn fp(id: i64, canonical: &str) -> Fingerprint {
        Fingerprint {
            id,
            hash: format!("hash-{id}"),
            canonical: canonical.to_string(),
            summary: Some(canonical.to_string()),
            first_seen_at: "2026-01-01T00:00:00Z".into(),
            last_seen_at: "2026-01-01T00:00:00Z".into(),
            occurrence_count: 1,
        }
    }

    fn alert_with(id: i64, msg: &str, symbol: Option<&str>, source: Option<&str>) -> Alert {
        Alert {
            id,
            snapshot_id: 1,
            severity: "warning".into(),
            message: msg.into(),
            source: source.map(String::from),
            symbol: symbol.map(String::from),
        }
    }

    #[test]
    fn generate_produces_zero_rows_when_inputs_empty() {
        let out = generate(&[], &[alert_with(1, "x", None, None)], &HashMap::new());
        assert!(out.is_empty());
        let out = generate(&[fp(1, "x")], &[], &HashMap::new());
        assert!(out.is_empty());
    }

    #[test]
    fn generate_emits_pair_only_when_total_above_threshold() {
        // Bug fingerprint identifies `DbPool` and references `db.md`.
        // The matching alert has overlapping message text, the same
        // `DbPool` symbol, and the same `db.md` source. Score:
        // message=1.0 * 0.55 + symbol>0 * 0.25 + file=1.0 * 0.20 →
        // above THRESHOLD. The unrelated alert scores 0 on every
        // component and is filtered.
        let bugs = vec![fp(
            1,
            "DbPool connection refused to db.md while reading pool",
        )];
        let alerts = vec![
            alert_with(
                1,
                "DbPool connection refused to db.md while reading pool",
                Some("DbPool"),
                Some("specs/db.md"),
            ),
            alert_with(2, "totally unrelated message about fonts", None, None),
        ];
        let out = generate(&bugs, &alerts, &HashMap::new());
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].1, 1);
        assert!(out[0].2.total >= THRESHOLD);
    }

    #[test]
    fn generate_orders_results_by_fingerprint_then_alert() {
        let bugs = vec![
            fp(2, "DbPool alpha bravo charlie db.md"),
            fp(1, "DbPool alpha bravo charlie db.md"),
        ];
        let alerts = vec![
            alert_with(
                2,
                "DbPool alpha bravo charlie",
                Some("DbPool"),
                Some("db.md"),
            ),
            alert_with(
                1,
                "DbPool alpha bravo charlie",
                Some("DbPool"),
                Some("db.md"),
            ),
        ];
        let out = generate(&bugs, &alerts, &HashMap::new());
        // Expect (fp=1, alert=1), (fp=1, alert=2), (fp=2, alert=1), (fp=2, alert=2).
        let ids: Vec<(i64, i64)> = out.iter().map(|(f, a, _)| (*f, *a)).collect();
        assert_eq!(ids, vec![(1, 1), (1, 2), (2, 1), (2, 2)]);
    }

    #[test]
    fn generate_caps_at_max_pairs_deterministically() {
        // 100 bugs x 100 alerts = 10_000 pairs, exceeds the 5_000
        // cap. Every pair scores >= threshold (full overlap on
        // message + symbol + file). After the cap, only the newest
        // 50 alerts (per_bug = 5_000 / 100 = 50) are scored
        // against every bug.
        let bugs: Vec<Fingerprint> = (1..=100)
            .map(|i| fp(i, "DbPool connection refused to db.md while reading pool"))
            .collect();
        let alerts: Vec<Alert> = (1..=100)
            .map(|i| {
                alert_with(
                    i,
                    "DbPool connection refused to db.md while reading pool",
                    Some("DbPool"),
                    Some("db.md"),
                )
            })
            .collect();
        let out = generate(&bugs, &alerts, &HashMap::new());
        // Every scored pair passes the threshold; expect 100 bugs x
        // 50 alerts = 5_000.
        assert_eq!(out.len(), MAX_PAIRS);
        // Newest-first: all selected alert ids are > 50.
        for (_f, a, _) in &out {
            assert!(*a > 50, "alert id {a} should be in newest 50");
        }
    }

    #[test]
    fn generate_output_never_exceeds_global_cap() {
        // 200 bugs x 100 alerts = 20_000 pairs. per_bug =
        // 5_000/200 = 25 newest alerts each; output truncated to
        // MAX_PAIRS.
        let bugs: Vec<Fingerprint> = (1..=200)
            .map(|i| fp(i, "DbPool connection refused to db.md while reading pool"))
            .collect();
        let alerts: Vec<Alert> = (1..=100)
            .map(|i| {
                alert_with(
                    i,
                    "DbPool connection refused to db.md while reading pool",
                    Some("DbPool"),
                    Some("db.md"),
                )
            })
            .collect();
        let out = generate(&bugs, &alerts, &HashMap::new());
        assert!(out.len() <= MAX_PAIRS, "got {}", out.len());
    }
}
