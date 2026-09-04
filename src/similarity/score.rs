//! Per-component and weighted similarity scores.
//!
//! The four components and their weights are part of the public
//! contract of the `correlation-and-ai-context` change; every row
//! persisted into `correlations` carries the individual component
//! scores plus a deterministic `total`.
//!
//! The algorithm version is a string literal rather than an integer
//! so future revisions (e.g. `"v2"`) are obvious in the database.

use crate::similarity::tokenize::{jaccard, tokenize};

/// Algorithm version that produced every correlation row from this
/// change. Bumping this value (and adding a new migration column if
/// needed) is how a future revision is recorded; existing rows
/// preserve the version that produced them.
pub const ALGO_VERSION: &str = "v1";

/// Weighted total below which a candidate is not displayed as a
/// "possible relationship". 0.65 is the documented threshold; do
/// not change it without a spec update.
pub const THRESHOLD: f64 = 0.65;

/// `(message, symbol, file, tag)` weights. They sum to 1.0 so the
/// `total` lives in `[0.0, 1.0]` and is directly comparable to
/// [`THRESHOLD`].
pub const WEIGHTS: (f64, f64, f64, f64) = (0.50, 0.20, 0.20, 0.10);

/// Per-component scores plus the deterministic total. All fields
/// are in `[0.0, 1.0]`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ComponentScores {
    pub message: f64,
    pub symbol: f64,
    pub file: f64,
    pub tag: f64,
    pub total: f64,
}

/// Bug side of the correlation. The canonical text is the same
/// normalized form stored in `fingerprints.canonical`; the summary
/// is the short human-readable line stored alongside it. Tags are
/// the union of tags from each occurrence's originating run.
#[derive(Debug, Clone)]
pub struct BugInput<'a> {
    pub canonical: &'a str,
    pub summary: &'a str,
    pub tags: &'a [String],
}

/// Alert side of the correlation. The message, symbol, and source
/// are exactly the fields emitted by the external checker protocol.
#[derive(Debug, Clone)]
pub struct AlertInput<'a> {
    pub message: &'a str,
    pub symbol: Option<&'a str>,
    pub source: Option<&'a str>,
}

/// Compute the per-component scores and weighted total for one bug +
/// alert pair. Never panics; missing fields produce a 0.0 component.
pub fn score_pair(bug: &BugInput<'_>, alert: &AlertInput<'_>) -> ComponentScores {
    let (wm, ws, wf, wt) = WEIGHTS;
    let message = score_message(bug, alert);
    let symbol = score_symbol(bug, alert);
    let file = score_file(bug, alert);
    let tag = score_tag(bug, alert);
    let total = wm * message + ws * symbol + wf * file + wt * tag;
    ComponentScores {
        message,
        symbol,
        file,
        tag,
        total,
    }
}

fn score_message(bug: &BugInput<'_>, alert: &AlertInput<'_>) -> f64 {
    // Concatenate canonical + summary before tokenizing so a short
    // canonical that misses a key phrase can still match via the
    // summary (and vice versa). The order is irrelevant for Jaccard.
    let mut bug_tokens = tokenize(bug.canonical);
    bug_tokens.extend(tokenize(bug.summary));
    let alert_tokens = tokenize(alert.message);
    jaccard(&bug_tokens, &alert_tokens)
}

fn score_symbol(bug: &BugInput<'_>, alert: &AlertInput<'_>) -> f64 {
    // Treat the alert's `symbol` as an identifier and compare it
    // against the first non-empty whitespace-delimited token of the
    // bug's canonical. We deliberately do not compare against the
    // `summary` because summaries may be truncated prose rather
    // than a clean identifier.
    let alert_sym = match alert.symbol {
        Some(s) if !s.is_empty() => s,
        _ => return 0.0,
    };
    let bug_sym = first_identifier_token(bug.canonical);
    match bug_sym {
        Some(t) if t.eq_ignore_ascii_case(alert_sym) => 1.0,
        _ => 0.0,
    }
}

fn score_file(bug: &BugInput<'_>, alert: &AlertInput<'_>) -> f64 {
    // The alert's `source` is a path-like string. The bug's
    // canonical may contain a relative path or a `<path>`
    // placeholder from the normalizer; we extract the first
    // identifier-like token from the alert source and look for a
    // matching token in the bug canonical. A direct string
    // containment check is a useful, conservative match: alerts
    // that mention a specific file tend to be highly diagnostic.
    let src = match alert.source {
        Some(s) if !s.is_empty() => s,
        _ => return 0.0,
    };
    let src_token = first_path_token(src);
    match src_token {
        Some(t) if bug.canonical.split_whitespace().any(|w| w == t) => 1.0,
        _ => 0.0,
    }
}

fn score_tag(bug: &BugInput<'_>, _alert: &AlertInput<'_>) -> f64 {
    // In v1, alerts carry no tags. The component is preserved in
    // the schema and the score function so future alert-side tags
    // can be added without a schema change. The bug side still
    // contributes a Jaccard against an empty alert set, which
    // collapses to 0.0.
    if bug.tags.is_empty() {
        return 0.0;
    }
    // Defensive: if a future caller populates alert tags (via the
    // extension path), we score them here. Today this branch is
    // unreachable because `_alert` has no tags accessor, but the
    // placeholder keeps the shape symmetric.
    0.0
}

/// First whitespace-delimited, non-empty token of `s` that starts
/// with an alphabetic or underscore character (i.e. a plausible
/// identifier). Used for symbol matching.
fn first_identifier_token(s: &str) -> Option<&str> {
    for tok in s.split_whitespace() {
        let cleaned = tok.trim_matches(|c: char| !c.is_alphanumeric() && c != '_');
        if cleaned.is_empty() {
            continue;
        }
        let first = cleaned.chars().next().unwrap();
        if first.is_ascii_alphabetic() || first == '_' {
            return Some(cleaned);
        }
    }
    None
}

/// First path-like token of `s`: the substring after the last
/// `/` or `\` in the first whitespace-delimited chunk. Falls back
/// to the first chunk if no separator is present.
fn first_path_token(s: &str) -> Option<&str> {
    let chunk = s.split_whitespace().next()?;
    if chunk.is_empty() {
        return None;
    }
    // Take the file name component (after the last separator).
    let last_sep = chunk.rfind(['/', '\\']).map(|i| i + 1).unwrap_or(0);
    let leaf = &chunk[last_sep..];
    if leaf.is_empty() {
        None
    } else {
        Some(leaf)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_bug() -> BugInput<'static> {
        BugInput {
            canonical: "",
            summary: "",
            tags: &[],
        }
    }

    fn empty_alert() -> AlertInput<'static> {
        AlertInput {
            message: "",
            symbol: None,
            source: None,
        }
    }

    #[test]
    fn algorithm_version_constant_is_v1() {
        assert_eq!(ALGO_VERSION, "v1");
    }

    #[test]
    fn threshold_constant_is_0_65() {
        assert!((THRESHOLD - 0.65).abs() < 1e-9);
    }

    #[test]
    fn weights_sum_to_one() {
        let (m, s, f, t) = WEIGHTS;
        assert!(((m + s + f + t) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn score_message_dominates_when_text_overlaps() {
        let bug = BugInput {
            canonical: "connection refused to database on port 5432",
            summary: "connection refused",
            tags: &[],
        };
        let alert = AlertInput {
            message: "connection refused to database on port 5432",
            symbol: None,
            source: None,
        };
        let s = score_pair(&bug, &alert);
        assert!(s.message > 0.9, "got message={}", s.message);
        assert!(s.total > 0.4, "got total={}", s.total);
    }

    #[test]
    fn score_symbol_exact_match_returns_one() {
        let bug = BugInput {
            canonical: "DbPool: timeout exceeded",
            summary: "DbPool timeout",
            tags: &[],
        };
        let alert = AlertInput {
            message: "unrelated message",
            symbol: Some("DbPool"),
            source: None,
        };
        let s = score_pair(&bug, &alert);
        assert!((s.symbol - 1.0).abs() < 1e-9);
    }

    #[test]
    fn score_symbol_mismatch_returns_zero() {
        let bug = BugInput {
            canonical: "DbPool: timeout",
            summary: "DbPool timeout",
            tags: &[],
        };
        let alert = AlertInput {
            message: "unrelated",
            symbol: Some("OtherPool"),
            source: None,
        };
        let s = score_pair(&bug, &alert);
        assert!((s.symbol - 0.0).abs() < 1e-9);
    }

    #[test]
    fn score_file_match_when_path_leaf_in_canonical() {
        let bug = BugInput {
            canonical: "error at db.md while reading pool",
            summary: "db.md",
            tags: &[],
        };
        let alert = AlertInput {
            message: "spec violation",
            symbol: None,
            source: Some("specs/db.md"),
        };
        let s = score_pair(&bug, &alert);
        assert!((s.file - 1.0).abs() < 1e-9, "got file={}", s.file);
    }

    #[test]
    fn score_file_no_match_when_leaf_absent() {
        let bug = BugInput {
            canonical: "error at src/main.rs",
            summary: "main.rs",
            tags: &[],
        };
        let alert = AlertInput {
            message: "spec violation",
            symbol: None,
            source: Some("specs/database.md"),
        };
        let s = score_pair(&bug, &alert);
        assert!((s.file - 0.0).abs() < 1e-9);
    }

    #[test]
    fn score_tag_zero_when_alert_has_no_tags() {
        let tags = vec!["auth".to_string(), "db".to_string()];
        let bug = BugInput {
            canonical: "x",
            summary: "y",
            tags: &tags,
        };
        let s = score_pair(&bug, &empty_alert());
        assert!((s.tag - 0.0).abs() < 1e-9);
    }

    #[test]
    fn total_score_zero_on_empty_inputs() {
        let s = score_pair(&empty_bug(), &empty_alert());
        assert_eq!(s.total, 0.0);
        assert_eq!(s.message, 0.0);
        assert_eq!(s.symbol, 0.0);
        assert_eq!(s.file, 0.0);
        assert_eq!(s.tag, 0.0);
    }

    #[test]
    fn score_pair_handles_missing_summary_via_constructor() {
        // Construct a BugInput directly with empty summary. The
        // `candidates::generate` pipeline is responsible for
        // materializing a summary when the stored one is `None`;
        // here we just confirm the scoring path tolerates an empty
        // summary without panicking.
        let s = score_pair(
            &BugInput {
                canonical: "connection refused to database",
                summary: "",
                tags: &[],
            },
            &AlertInput {
                message: "connection refused to database",
                symbol: None,
                source: None,
            },
        );
        assert!(s.message > 0.9, "got message={}", s.message);
    }

    #[test]
    fn first_identifier_token_skips_punctuation() {
        assert_eq!(
            first_identifier_token(": leading colon ok"),
            Some("leading")
        );
        assert_eq!(first_identifier_token("DbPool timeout"), Some("DbPool"));
        assert_eq!(first_identifier_token("123 nope"), Some("nope"));
        assert_eq!(first_identifier_token("---"), None);
    }

    #[test]
    fn first_path_token_returns_leaf() {
        assert_eq!(first_path_token("specs/db.md"), Some("db.md"));
        assert_eq!(first_path_token("a/b/c.rs:10"), Some("c.rs:10"));
        assert_eq!(first_path_token("plain"), Some("plain"));
        assert_eq!(first_path_token(""), None);
    }
}
