//! Deterministic text tokenization and Jaccard similarity.
//!
//! Tokenization rules:
//! * split on any non-alphanumeric character;
//! * lowercase every token;
//! * drop tokens shorter than 3 characters (eliminates noise like
//!   `s`, `t`, `42` from line/column);
//! * drop a small English stopword set so generic words do not
//!   dominate the Jaccard intersection.
//!
//! The tokenizer is intentionally language-agnostic: it does not
//! stem, lemmatize, or recognize compound words. The output is
//! deterministic for a given input.

use std::collections::HashSet;

/// Common English words that appear in so many failure messages that
/// they add noise without discriminating power. The list is short on
/// purpose: driftwatch's job is to surface leads, not to score essays.
pub const STOPWORDS: &[&str] = &[
    "the", "and", "for", "with", "this", "that", "from", "into", "have", "has", "had", "was",
    "were", "are", "but", "not", "you", "your", "our", "their", "while", "when", "then", "than",
    "over", "under", "such", "also",
];

/// Tokenize `s` into a deterministic list of normalized tokens.
/// The order is preserved; duplicates are kept (the Jaccard
/// implementation deduplicates via sets).
pub fn tokenize(s: &str) -> Vec<String> {
    s.split(|c: char| !c.is_alphanumeric())
        .map(|t| t.to_ascii_lowercase())
        .filter(|t| t.len() >= 3 && !STOPWORDS.contains(&t.as_str()))
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
        // "a", "b", "c", "BadToken" stay after lowercasing and length
        // check. We assert the lowercased form survived.
        assert!(t.contains(&"errortoken".to_string()) || t.contains(&"badtoken".to_string()));
        // short tokens are dropped.
        assert!(!t.iter().any(|w| w == "a"));
        assert!(!t.iter().any(|w| w == "b"));
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
