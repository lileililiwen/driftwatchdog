//! Shared small utilities for display-safe handling of untrusted text.

/// Truncate `s` to at most `limit` bytes, cutting only at a `char` boundary.
///
/// Returns `s` unchanged when `s.len() <= limit`. Otherwise cuts back to the
/// nearest char boundary at or below `limit` and appends `…` (U+2026).
/// Never panics; the result is always valid UTF-8 with
/// `len <= limit + 3`.
///
/// Byte (not char-count) semantics match the historical `&s[..N]` call sites
/// this replaces (`check` diagnostics, `list` commands, `top` summaries).
pub fn truncate_char_boundary(s: &str, limit: usize) -> String {
    if s.len() <= limit {
        return s.to_string();
    }
    if s.is_empty() {
        return String::new();
    }
    let mut cut = limit.min(s.len());
    while cut > 0 && !s.is_char_boundary(cut) {
        cut -= 1;
    }
    let mut out = String::with_capacity(cut + 3);
    out.push_str(&s[..cut]);
    out.push('…');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_strings_unchanged() {
        assert_eq!(truncate_char_boundary("hi", 10), "hi");
        assert_eq!(truncate_char_boundary("", 0), "");
        assert_eq!(truncate_char_boundary("abc", 3), "abc");
    }

    #[test]
    fn ascii_truncates_with_ellipsis() {
        let t = truncate_char_boundary(&"a".repeat(100), 10);
        assert!(t.ends_with('…'));
        assert!(t.len() <= 13, "got {} bytes", t.len());
    }

    #[test]
    fn cjk_at_cut_point_does_not_panic() {
        // Each CJK char is 3 bytes; limit 200 lands mid-char for 300 chars.
        let s = "汉".repeat(300);
        let t = truncate_char_boundary(&s, 200);
        assert!(t.ends_with('…'));
        assert!(t.len() <= 203, "got {} bytes", t.len());
        // str is valid UTF-8 by construction; char-boundary cut keeps it so.
        assert!(t.is_char_boundary(t.len() - 3));
    }

    #[test]
    fn emoji_straddling_cut_point() {
        let s = format!("{}🎉{}", "a".repeat(78), "b".repeat(10));
        let t = truncate_char_boundary(&s, 79);
        assert!(t.ends_with('…'));
        assert!(t.len() <= 82, "got {} bytes", t.len());
    }

    #[test]
    fn limit_zero_yields_ellipsis_for_nonempty() {
        assert_eq!(truncate_char_boundary("hi", 0), "…");
        assert_eq!(truncate_char_boundary("", 0), "");
    }

    #[test]
    fn property_no_panic_all_limits() {
        let s = "aé汉🎉".repeat(75); // mixed 1/2/3/4-byte chars, 300 chars
        for limit in 0..300 {
            let t = truncate_char_boundary(&s, limit);
            assert!(t.len() <= limit + 3, "limit {limit}: len {}", t.len());
            // Must remain valid UTF-8 (it is a String, but also must not
            // have split a char: re-slicing at every boundary must hold).
            assert!(t.is_char_boundary(t.len()));
        }
    }
}
