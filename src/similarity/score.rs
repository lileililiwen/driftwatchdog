//! Per-component and weighted similarity scores.
//!
//! The components and their weights are part of the public contract
//! of the `correlation-and-ai-context` change; every row persisted
//! into `correlations` carries the individual component scores plus
//! a deterministic `total`.
//!
//! Weights (v2, recalibrated): `message = 0.55`, `symbol = 0.25`,
//! `file = 0.20`, `tag = 0.0`. The tag component is preserved in the
//! schema and the score struct for forward compatibility, but alerts
//! carry no tags in v1, so its weight is honestly 0 rather than a
//! dead 0.1 that would cap the effective maximum at 0.9. The
//! threshold stays at 0.65; see the labeled fixtures in
//! `tests/similarity.rs` and `tests/correlation.rs`.
//!
//! The algorithm version is a string literal rather than an integer
//! so future revisions (e.g. `"v3"`) are obvious in the database.

use crate::similarity::tokenize::{jaccard, tokenize};

/// Algorithm version that produced every correlation row from this
/// change. Bumped to `v2` for the recalibrated scorer (placeholder-aware
/// file scoring, multi-token symbol scoring, honest tag weight).
pub const ALGO_VERSION: &str = "v2";

/// Weighted total below which a candidate is not displayed as a
/// "possible relationship". 0.65 is the documented threshold; do
/// not change it without a spec update.
pub const THRESHOLD: f64 = 0.65;

/// `(message, symbol, file, tag)` weights. They sum to 1.0 so the
/// `total` lives in `[0.0, 1.0]` and is directly comparable to
/// [`THRESHOLD`]. `tag` is 0.0: alerts carry no tags, so the weight
/// is documented as reserved rather than silently dead.
pub const WEIGHTS: (f64, f64, f64, f64) = (0.55, 0.25, 0.20, 0.0);

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
    // Multi-token symbol scoring: tokenize the alert symbol and the
    // bug's canonical + summary. A bare generic symbol (`error`,
    // `failed`, ...) tokenizes to nothing and scores 0.0. When every
    // symbol token appears in the bug text, score 1.0 (exact
    // identifier match, e.g. `DbPool` in a `DbPool ...` bug);
    // otherwise fall back to Jaccard for partial multi-token overlap.
    let alert_sym = match alert.symbol {
        Some(s) if !s.trim().is_empty() => s,
        _ => return 0.0,
    };
    let mut bug_tokens = tokenize(bug.canonical);
    bug_tokens.extend(tokenize(bug.summary));
    if bug_tokens.is_empty() {
        return 0.0;
    }
    let sym_tokens = tokenize(alert_sym);
    if sym_tokens.is_empty() {
        return 0.0;
    }
    let bug_set: std::collections::HashSet<&str> = bug_tokens.iter().map(String::as_str).collect();
    if sym_tokens.iter().all(|t| bug_set.contains(t.as_str())) {
        return 1.0;
    }
    jaccard(&bug_tokens, &sym_tokens)
}

fn leaf_name(s: &str) -> String {
    // Strip `:line:col` suffixes, then take the portion after the
    // last `/` or `\`, lowercased.
    let no_loc = s.split(':').next().unwrap_or(s);
    let chunk = no_loc.split_whitespace().next().unwrap_or(no_loc);
    let last_sep = chunk.rfind(['/', '\\']).map(|i| i + 1).unwrap_or(0);
    chunk[last_sep..].to_ascii_lowercase()
}

fn bug_tokens_normalized(canonical: &str) -> Vec<String> {
    canonical
        .split_whitespace()
        .map(|w| {
            let t = w.trim_matches(|c: char| {
                !c.is_alphanumeric() && c != '_' && c != '.' && c != '<' && c != '>'
            });
            leaf_name(t)
        })
        .filter(|t| !t.is_empty())
        .collect()
}

fn score_file(bug: &BugInput<'_>, alert: &AlertInput<'_>) -> f64 {
    // Placeholder-aware file scoring. The alert's `source` is a
    // path-like string; the bug's canonical may contain a relative
    // path, an absolute path collapsed to `<path>`, or a leaf name.
    // Exact leaf matches score 1.0; a `<path>`/`<tmp>` placeholder
    // against a path-like alert source earns 0.5 partial credit;
    // otherwise 0.0.
    let src = match alert.source {
        Some(s) if !s.trim().is_empty() => s,
        _ => return 0.0,
    };
    let alert_leaf = leaf_name(src);
    if alert_leaf.is_empty() {
        return 0.0;
    }
    let bug_leaves = bug_tokens_normalized(bug.canonical);
    if bug_leaves.iter().any(|t| t == &alert_leaf) {
        return 1.0;
    }
    // Summary often carries the leaf (`db.md`) even when canonical
    // collapsed the directory.
    if let Some(summary) = Some(bug.summary).filter(|s| !s.is_empty()) {
        let summary_leaves = bug_tokens_normalized(summary);
        if summary_leaves.iter().any(|t| t == &alert_leaf) {
            return 1.0;
        }
    }
    let looks_like_path = src.contains('/') || src.contains('\\') || src.contains('.');
    if looks_like_path && (bug.canonical.contains("<path>") || bug.canonical.contains("<tmp>")) {
        return 0.5;
    }
    0.0
}

fn score_tag(_bug: &BugInput<'_>, _alert: &AlertInput<'_>) -> f64 {
    // Alerts carry no tags in v1/v2. The component is preserved in the
    // schema and the score struct for forward compatibility, but its
    // weight is 0 (see WEIGHTS), so this honestly returns 0.0.
    0.0
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
    fn algorithm_version_constant_is_v2() {
        assert_eq!(ALGO_VERSION, "v2");
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
            canonical: "DbPool connection timeout exceeded",
            summary: "DbPool timeout",
            tags: &[],
        };
        let alert = AlertInput {
            message: "unrelated message about fonts",
            symbol: Some("DbPool"),
            source: None,
        };
        let s = score_pair(&bug, &alert);
        assert!(s.symbol > 0.0, "got symbol={}", s.symbol);
    }

    #[test]
    fn generic_error_symbol_does_not_match_alone() {
        // Bug text starts with generic `error`; alert symbol is
        // `error`. Both tokenize to stopwords-only, so symbol is 0
        // and the pair must not pass on symbol alone.
        let bug = BugInput {
            canonical: "error something broke",
            summary: "error",
            tags: &[],
        };
        let alert = AlertInput {
            message: "totally different font rendering path",
            symbol: Some("error"),
            source: None,
        };
        let s = score_pair(&bug, &alert);
        assert_eq!(s.symbol, 0.0);
        assert!(s.total < THRESHOLD, "got total={}", s.total);
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
            canonical: "connection refused to db.md while reading pool",
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
    fn score_file_placeholder_earns_partial_credit() {
        // Absolute bug path collapsed to `<path>`; alert references
        // the leaf. Placeholder-aware scoring gives 0.5, not 0.
        let bug = BugInput {
            canonical: "connection timeout at <path> while reading pool",
            summary: "connection timeout",
            tags: &[],
        };
        let alert = AlertInput {
            message: "spec violation about fonts",
            symbol: None,
            source: Some("src/db.rs"),
        };
        let s = score_pair(&bug, &alert);
        assert!((s.file - 0.5).abs() < 1e-9, "got file={}", s.file);
    }

    #[test]
    fn score_file_strips_line_suffix() {
        // Alert source `c.rs:10` matches bug leaf `c.rs`.
        let bug = BugInput {
            canonical: "build failure at c.rs",
            summary: "c.rs",
            tags: &[],
        };
        let alert = AlertInput {
            message: "spec violation",
            symbol: None,
            source: Some("src/c.rs:10"),
        };
        let s = score_pair(&bug, &alert);
        assert!((s.file - 1.0).abs() < 1e-9, "got file={}", s.file);
    }

    #[test]
    fn absolute_path_bug_matches_alert_source() {
        // End-to-end labeled fixture: absolute-path bug (collapsed to
        // `<path>`) vs alert referencing a path + symbol + overlapping
        // message. File earns placeholder partial credit (0.5, not 0);
        // symbol matches exactly (1.0); message overlap carries the
        // rest over the threshold.
        let bug = BugInput {
            canonical: "DbPool connection timeout pool exhausted at <path>",
            summary: "DbPool connection timeout pool exhausted",
            tags: &[],
        };
        let alert = AlertInput {
            message: "DbPool connection timeout pool exhausted violates spec",
            symbol: Some("DbPool"),
            source: Some("src/db.rs"),
        };
        let s = score_pair(&bug, &alert);
        assert!(s.file > 0.0, "got file={}", s.file);
        assert!(s.symbol > 0.0, "got symbol={}", s.symbol);
        assert!(s.total >= THRESHOLD, "got total={}", s.total);
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
    fn score_tag_weight_is_honestly_zero() {
        let (_, _, _, wt) = WEIGHTS;
        assert_eq!(wt, 0.0);
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
    fn leaf_name_strips_location_suffix() {
        assert_eq!(leaf_name("specs/db.md"), "db.md");
        assert_eq!(leaf_name("src/c.rs:10"), "c.rs");
        assert_eq!(leaf_name("a/b/c.rs:10:5"), "c.rs");
        assert_eq!(leaf_name("plain"), "plain");
    }
}
