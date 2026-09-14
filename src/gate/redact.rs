//! Secret-safe, bounded diagnostics and evidence previews.
//!
//! Raw checker output can contain tokens, passwords, or private keys.
//! Nothing in the gate contract embeds raw unbounded output: diagnostics
//! and previews pass through [`redact_secrets`] then a char-boundary
//! truncation before they are stored or serialized.

use crate::util::truncate_char_boundary;
use regex::Regex;
use std::sync::OnceLock;

fn patterns() -> &'static Vec<(Regex, &'static str)> {
    static CELL: OnceLock<Vec<(Regex, &'static str)>> = OnceLock::new();
    CELL.get_or_init(|| {
        let kv = Regex::new(
            r"(?i)(aws_secret_access_key|aws_session_token|password|passwd|secret|api[_-]?key|auth[_-]?token|access[_-]?token|github[_-]?token|bearer)\s*[:=]\s*\S+",
        )
        .expect("static regex");
        let github = Regex::new(r"\b(ghp_[A-Za-z0-9_]{10,}|gho_[A-Za-z0-9_]{10,}|github_pat_[A-Za-z0-9_]{10,})\b")
            .expect("static regex");
        let aws_key = Regex::new(r"\bAKIA[0-9A-Z]{16}\b").expect("static regex");
        let slack = Regex::new(r"\bxox[abpr]-([A-Za-z0-9-]{10,})\b").expect("static regex");
        let private_key =
            Regex::new(r"-----BEGIN (?:RSA )?PRIVATE KEY-----").expect("static regex");
        vec![
            (kv, "$1=[REDACTED]"),
            (github, "[REDACTED]"),
            (aws_key, "[REDACTED]"),
            (slack, "[REDACTED]"),
            (private_key, "[REDACTED PRIVATE KEY]"),
        ]
    })
}

/// Replace likely secret values with `[REDACTED]`. Best-effort and
/// deterministic; unknown secret shapes may still pass through, so
/// callers must also bound the output length.
pub fn redact_secrets(s: &str) -> String {
    let mut out = s.to_string();
    for (re, replacement) in patterns().iter() {
        out = re.replace_all(&out, *replacement).into_owned();
    }
    out
}

/// Like [`redact_secrets`], but also redacts caller-configured
/// sensitive values (e.g. tokens from the project environment) with a
/// plain substring match. Values are applied longest-first so overlapping
/// tokens redact deterministically. Empty values are ignored.
pub fn redact_secrets_with_extra(s: &str, extra: &[&str]) -> String {
    let mut out = redact_secrets(s);
    let mut ordered: Vec<&str> = extra.iter().copied().filter(|v| !v.is_empty()).collect();
    ordered.sort_by_key(|a| std::cmp::Reverse(a.len()));
    for secret in ordered {
        if out.contains(secret) {
            out = out.replace(secret, "[REDACTED]");
        }
    }
    out
}

/// Redact (with configured extra values) then truncate to `limit`
/// bytes at a char boundary. Always returns valid UTF-8; never panics.
pub fn bound_text_with_extra(s: &str, extra: &[&str], limit: usize) -> String {
    truncate_char_boundary(&redact_secrets_with_extra(s, extra), limit)
}

/// Redact then truncate to `limit` bytes at a char boundary.
/// Always returns valid UTF-8; never panics.
pub fn bound_text(s: &str, limit: usize) -> String {
    truncate_char_boundary(&redact_secrets(s), limit)
}

/// Bound a diagnostic line for persistence and display.
pub fn bounded_diagnostic(s: &str, limit: usize) -> String {
    bound_text(s, limit)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_key_value_secrets() {
        let out = redact_secrets("api_key=supersecret123 and password: hunter2");
        assert!(!out.contains("supersecret123"));
        assert!(!out.contains("hunter2"));
        assert!(out.contains("[REDACTED]"));
    }

    #[test]
    fn redacts_github_and_aws_shapes() {
        assert!(!redact_secrets("token ghp_abcdefghij1234567890 done").contains("ghp_"));
        assert!(!redact_secrets("key AKIAIOSFODNN7EXAMPLE end").contains("AKIA"));
    }

    #[test]
    fn redacts_private_key_header() {
        let out = redact_secrets("-----BEGIN RSA PRIVATE KEY-----\nabc");
        assert!(out.contains("[REDACTED PRIVATE KEY]"));
    }

    #[test]
    fn bound_text_truncates_at_char_boundary() {
        let s = format!("password=secret {}", "汉".repeat(500));
        let out = bound_text(&s, 64);
        assert!(out.len() <= 67);
        assert!(!out.contains("secret"));
    }

    #[test]
    fn redacts_configured_extra_values_longest_first() {
        let out = redact_secrets_with_extra("token abc123 (abc)", &["abc", "abc123"]);
        assert!(!out.contains("abc123"));
        assert!(!out.contains("(abc)"));
        assert!(out.contains("[REDACTED]"));
    }

    #[test]
    fn extra_redaction_ignores_empty_values() {
        let out = redact_secrets_with_extra("plain text", &[""]);
        assert_eq!(out, "plain text");
    }
}
