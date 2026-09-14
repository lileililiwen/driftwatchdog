//! Deterministic text tokenization and Jaccard similarity.
//!
//! Tokenization rules:
//! * split on any non-alphanumeric character;
//! * lowercase every token;
//! * keep short domain tokens on an allowlist (`db`, `io`, `os`,
//!   `s3`, `ui`, `go`) and otherwise drop tokens shorter than 3
//!   characters (eliminates noise like `s`, `t`, `42`);
//! * drop an English + failure-domain stopword set so generic words
//!   (`the`, `error`, `failed`, ...) do not dominate Jaccard.
//!
//! The tokenizer is intentionally language-agnostic: it does not
//! stem, lemmatize, or recognize compound words. The output is
//! deterministic for a given input.

use std::collections::HashSet;

/// Common English words that appear in so many failure messages that
/// they add noise without discriminating power, plus generic
/// failure-domain words (`error`, `failed`, ...) that would otherwise
/// let any two failures match. Kept short on purpose: driftwatch's
/// job is to surface leads, not to score essays.
pub const STOPWORDS: &[&str] = &[
    "the",
    "and",
    "for",
    "with",
    "this",
    "that",
    "from",
    "into",
    "have",
    "has",
    "had",
    "was",
    "were",
    "are",
    "but",
    "not",
    "you",
    "your",
    "our",
    "their",
    "while",
    "when",
    "then",
    "than",
    "over",
    "under",
    "such",
    "also",
    "error",
    "errors",
    "failed",
    "failure",
    "failures",
    "exception",
    "exceptions",
    "traceback",
    "warning",
    "warnings",
    "invalid",
    "cannot",
    "unable",
    "expected",
    "unexpected",
    "occurred",
];

/// Short tokens that carry domain meaning and must survive the
/// minimum-length filter.
pub const SHORT_TOKEN_ALLOWLIST: &[&str] = &["db", "io", "os", "s3", "ui", "go"];

/// Tokenize `s` into a deterministic list of normalized tokens.
/// The order is preserved; duplicates are kept (the Jaccard
/// implementation deduplicates via sets).
pub fn tokenize(s: &str) -> Vec<String> {
    s.split(|c: char| !c.is_alphanumeric())
        .map(|t| t.to_ascii_lowercase())
        .filter(|t| {
            (t.len() >= 3 || SHORT_TOKEN_ALLOWLIST.contains(&t.as_str()))
                && !STOPWORDS.contains(&t.as_str())
        })
        .map(|t| t.to_string())
        .collect()
}

/// Jaccard similarity between two token lists. Returns a value in
/// `[0.0, 1.0]`. Two empty sets are treated as 0.0 (no evidence of
/// similarity, not perfect similarity).
pub fn jaccard(a: &[String], b: &[String]) -> f64 {
    let sa: HashSet<&str> = a.iter().map(String::as_str).collect();
    let sb: HashSet<&str> = b.iter().map(String::as_str).collect();
    if sa.is_empty() && sb.is_empty() {
        return 0.0;
    }
    let inter = sa.intersection(&sb).count() as f64;
    let union = sa.union(&sb).count() as f64;
    if union == 0.0 {
        0.0
    } else {
        inter / union
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenize_drops_stopwords() {
        let t = tokenize("the quick brown fox and the lazy dog");
        // "the", "and", "the" are stopwords; the rest survive.
        assert!(!t.iter().any(|w| w == "the"));
        assert!(!t.iter().any(|w| w == "and"));
        assert!(t.contains(&"quick".to_string()));
        assert!(t.contains(&"brown".to_string()));
        assert!(t.contains(&"fox".to_string()));
    }

    #[test]
    fn tokenize_lowercases_and_drops_short() {
        let t = tokenize("Error: a b c BadToken xyz");
        // "Error" is a generic failure-domain stopword; "a"/"b"/"c"
        // are too short. "badtoken"/"xyz" survive lowercased.
        assert!(!t.iter().any(|w| w == "error"));
        assert!(t.contains(&"badtoken".to_string()));
        assert!(!t.iter().any(|w| w == "a"));
        assert!(!t.iter().any(|w| w == "b"));
    }

    #[test]
    fn tokenize_keeps_short_domain_tokens() {
        let t = tokenize("s3 db io read failed");
        // "failed" is a stopword; s3/db/io survive via the allowlist.
        assert!(t.contains(&"s3".to_string()));
        assert!(t.contains(&"db".to_string()));
        assert!(t.contains(&"io".to_string()));
        assert!(!t.iter().any(|w| w == "failed"));
    }

    #[test]
    fn tokenize_drops_generic_failure_words() {
        let t = tokenize("error exception failed warning");
        assert!(t.is_empty(), "got: {t:?}");
    }

    #[test]
    fn tokenize_splits_on_punctuation() {
        let t = tokenize("foo,bar.baz;qux");
        assert_eq!(t, vec!["foo", "bar", "baz", "qux"]);
    }

    #[test]
    fn jaccard_identical_is_one() {
        let a = tokenize("connection timeout to database");
        let b = tokenize("database connection timeout");
        assert!((jaccard(&a, &b) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn jaccard_disjoint_is_zero() {
        let a = tokenize("apple banana");
        let b = tokenize("pear grape");
        assert!((jaccard(&a, &b) - 0.0).abs() < 1e-9);
    }

    #[test]
    fn jaccard_handles_empty_inputs() {
        let a: Vec<String> = vec![];
        let b: Vec<String> = vec![];
        assert!((jaccard(&a, &b) - 0.0).abs() < 1e-9);
        let c = tokenize("only words here");
        assert!((jaccard(&a, &c) - 0.0).abs() < 1e-9);
    }

    #[test]
    fn jaccard_partial_overlap() {
        let a = tokenize("connection refused to database");
        let b = tokenize("connection timeout from database");
        // "to" and "from" are stopwords. Tokens: A = {connection, refused, database}, B = {connection, timeout, database}.
        // Intersection = {connection, database} (2). Union = {connection, refused, database, timeout} (4).
        let j = jaccard(&a, &b);
        assert!((j - 0.5).abs() < 1e-9, "got {j}");
    }

    #[test]
    fn jaccard_is_symmetric() {
        let a = tokenize("alpha beta gamma delta");
        let b = tokenize("gamma delta epsilon zeta");
        assert!((jaccard(&a, &b) - jaccard(&b, &a)).abs() < 1e-9);
    }
}
