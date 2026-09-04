//! Integration tests for the similarity engine. The unit tests in
//! `src/similarity/*` cover the per-function behavior; this file
//! exercises the public `score_pair` and `generate` functions from
//! a black-box perspective.

use driftwatchdog::repo::alerts::Alert;
use driftwatchdog::repo::bugs::Fingerprint;
use driftwatchdog::similarity::candidates::{generate, MAX_PAIRS};
use driftwatchdog::similarity::score::{score_pair, AlertInput, BugInput, ALGO_VERSION, THRESHOLD};
use driftwatchdog::similarity::tokenize::{jaccard, tokenize};
use std::collections::HashMap;

fn fp(id: i64, canonical: &str) -> Fingerprint {
    Fingerprint {
        id,
        hash: format!("hash-{id}"),
        canonical: canonical.into(),
        summary: Some(canonical.into()),
        first_seen_at: "2026-01-01T00:00:00Z".into(),
        last_seen_at: "2026-01-01T00:00:00Z".into(),
        occurrence_count: 1,
    }
}

fn alert(id: i64, msg: &str, symbol: Option<&str>, source: Option<&str>) -> Alert {
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
fn tokenize_is_deterministic_across_runs() {
    let input = "the DbPool: connection refused to database after timeout";
    assert_eq!(tokenize(input), tokenize(input));
    let tokens = tokenize(input);
    // The "DbPool" identifier and the "connection"/"refused"/
    // "database"/"timeout" words are all preserved.
    assert!(tokens.contains(&"dbpool".to_string()));
    assert!(tokens.contains(&"connection".to_string()));
    assert!(tokens.contains(&"refused".to_string()));
    assert!(tokens.contains(&"database".to_string()));
    assert!(tokens.contains(&"timeout".to_string()));
    // "the" and "to" are stopwords; "after" is a 5-char word and
    // is not in the stopword list, so it is preserved.
    assert!(!tokens.contains(&"the".to_string()));
    assert!(!tokens.contains(&"to".to_string()));
}

#[test]
fn jaccard_zero_for_two_distinct_short_messages() {
    let a = tokenize("foo bar");
    let b = tokenize("baz qux");
    assert_eq!(jaccard(&a, &b), 0.0);
}

#[test]
fn jaccard_one_for_identical_messages() {
    let a = tokenize("foo bar baz");
    let b = tokenize("foo bar baz");
    assert!((jaccard(&a, &b) - 1.0).abs() < 1e-9);
}

#[test]
fn score_pair_uses_documented_weights_and_threshold() {
    // Bug and alert have identical identifiers and source paths.
    let bug = BugInput {
        canonical: "DbPool connection refused to db.md",
        summary: "DbPool connection refused to db.md",
        tags: &[],
    };
    let alert = AlertInput {
        message: "DbPool connection refused to db.md",
        symbol: Some("DbPool"),
        source: Some("specs/db.md"),
    };
    let s = score_pair(&bug, &alert);
    // All four components reach 1.0; the total is 0.9 (= 0.5 +
    // 0.2 + 0.2; tag stays at 0). Floating-point: 0.9 ± ε.
    assert!((s.total - 0.9).abs() < 1e-9, "got {}", s.total);
    assert!(s.total >= THRESHOLD);
}

#[test]
fn score_pair_algorithm_version_constant_is_stable() {
    assert_eq!(ALGO_VERSION, "v1");
}

#[test]
fn generate_emits_empty_when_inputs_are_empty() {
    let out = generate(&[], &[], &HashMap::new());
    assert!(out.is_empty());
}

#[test]
fn generate_caps_at_max_pairs_when_product_exceeds() {
    // 100 bugs x 100 alerts = 10_000 > MAX_PAIRS.
    let bugs: Vec<Fingerprint> = (1..=100)
        .map(|i| fp(i, "DbPool connection refused to db.md"))
        .collect();
    let alerts: Vec<Alert> = (1..=100)
        .map(|i| {
            alert(
                i,
                "DbPool connection refused to db.md",
                Some("DbPool"),
                Some("db.md"),
            )
        })
        .collect();
    let out = generate(&bugs, &alerts, &HashMap::new());
    assert_eq!(out.len(), MAX_PAIRS);
}
