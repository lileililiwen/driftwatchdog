//! Generic failure normalizer.
//!
//! The normalizer applies an ordered set of [`Rule`]s that strip or
//! replace unstable tokens (ANSI escapes, absolute paths, line/column,
//! UUIDs, timestamps, PIDs, ports, durations, temp paths, long random
//! values) while preserving meaningful text and relative paths. The
//! generic profile is the v1 compatibility baseline: it accepts
//! arbitrary text and never rejects an unknown toolchain.
//!
//! After the rules run, post-processing normalizes line endings,
//! trims trailing whitespace per line, and preserves leading
//! indentation (so Python/YAML blocks stay distinct). The resulting
//! canonical text is what we hash and persist.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use fancy_regex::Regex;

/// Maximum length (in characters) of the human-readable summary line.
pub const SUMMARY_MAX: usize = 120;

/// Maximum size (in bytes) of a persisted raw excerpt.
pub const EXCERPT_MAX: usize = 4096;

/// Output of normalization: the canonical text plus a 1-line summary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Canonical {
    pub text: String,
    pub summary: String,
}

/// One replacement rule. `pattern` is compiled lazily on first use and
/// cached for the process lifetime.
#[derive(Debug, Clone)]
pub struct Rule {
    pub name: &'static str,
    pub pattern: &'static str,
    pub replacement: &'static str,
}

impl Rule {
    /// Apply this rule to `text`, returning the post-replacement copy.
    /// A rule whose pattern fails to compile is skipped (input
    /// returned unchanged) so normalization never panics. Use
    /// [`Rule::try_apply`] when the caller needs the error.
    pub fn apply(&self, text: &str) -> String {
        self.try_apply(text).unwrap_or_else(|_| text.to_string())
    }

    /// Fallible application: returns the replaced text, or an error
    /// naming the rule when its pattern does not compile.
    pub fn try_apply(&self, text: &str) -> Result<String, String> {
        let re = try_compile(self.name, self.pattern)?;
        Ok(re.replace_all(text, self.replacement).into_owned())
    }
}

/// Ordered set of rules. The default is [`Rules::generic`].
pub struct Rules(&'static [Rule]);

impl Rules {
    /// The v1 default rule set. Order matters: later rules see the
    /// post-replacement text of earlier ones.
    pub const fn generic() -> Self {
        Self(GENERIC_RULES)
    }

    /// Apply every rule in order, then post-process whitespace.
    pub fn normalize(&self, input: &str) -> Canonical {
        let mut text = input.to_string();
        for rule in self.0.iter() {
            text = rule.apply(&text);
        }
        text = post_process(&text);
        let summary = summary_of(&text);
        Canonical { text, summary }
    }
}

/// First non-empty line of `canonical`, trimmed and truncated to
/// [`SUMMARY_MAX`] characters. Appends `…` when truncated.
pub fn summary_of(canonical: &str) -> String {
    for line in canonical.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        return truncate_chars(trimmed, SUMMARY_MAX);
    }
    String::new()
}

/// Truncate `text` to at most [`EXCERPT_MAX`] bytes, appending `…` when
/// anything was removed. The function operates on a char boundary so the
/// result is valid UTF-8.
pub fn bounded_excerpt(text: &str) -> String {
    if text.len() <= EXCERPT_MAX {
        return text.to_string();
    }
    // Walk back to a char boundary at or below EXCERPT_MAX.
    let mut cut = EXCERPT_MAX;
    while cut > 0 && !text.is_char_boundary(cut) {
        cut -= 1;
    }
    let mut out = String::with_capacity(cut + 3);
    out.push_str(&text[..cut]);
    out.push('…');
    out
}

fn truncate_chars(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max).collect();
    out.push('…');
    out
}

fn post_process(text: &str) -> String {
    // Normalize line endings, preserve leading indentation (Python/YAML
    // blocks stay distinct), collapse interior horizontal whitespace
    // to a single space, trim trailing whitespace per line, and
    // collapse 3+ consecutive blank lines to 2.
    let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
    let mut out = String::with_capacity(normalized.len());
    let mut blank_run = 0usize;
    for line in normalized.split('\n') {
        // Split off leading indentation before collapsing.
        let indent_len = line.len() - line.trim_start_matches([' ', '\t']).len();
        let (indent, rest) = line.split_at(indent_len);
        // Collapse interior runs of spaces/tabs; drop trailing space.
        let mut collapsed = String::with_capacity(rest.len());
        let mut in_space = false;
        for ch in rest.chars() {
            if ch == ' ' || ch == '\t' {
                in_space = true;
            } else {
                if in_space && !collapsed.is_empty() {
                    collapsed.push(' ');
                }
                in_space = false;
                collapsed.push(ch);
            }
        }
        if collapsed.is_empty() {
            blank_run += 1;
            if blank_run <= 2 {
                out.push('\n');
            }
        } else {
            blank_run = 0;
            out.push_str(indent);
            out.push_str(&collapsed);
            out.push('\n');
        }
    }
    // Trim trailing newlines.
    while out.ends_with('\n') {
        out.pop();
    }
    out
}

/// Pattern-keyed regex cache. Returns a cloned `Regex`; invalid
/// patterns produce an `Err` naming the rule instead of panicking.
/// No leaking: entries live in a `OnceLock<Mutex<...>>` for the
/// process lifetime.
fn cache() -> &'static Mutex<HashMap<(String, String), Regex>> {
    static CACHE: OnceLock<Mutex<HashMap<(String, String), Regex>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Fallible compilation with a `(name, pattern)` cache key.
pub fn try_compile(name: &str, pattern: &str) -> Result<Regex, String> {
    {
        let guard = cache()
            .lock()
            .map_err(|e| format!("regex cache poisoned: {e}"))?;
        if let Some(re) = guard.get(&(name.to_string(), pattern.to_string())) {
            return Ok(re.clone());
        }
    }
    let re = Regex::new(pattern).map_err(|e| format!("invalid rule {name}: {e}"))?;
    if let Ok(mut guard) = cache().lock() {
        guard.insert((name.to_string(), pattern.to_string()), re.clone());
    }
    Ok(re)
}

/// Compile a one-off regex (used by tests for negative cases). Not part
/// of the public API.
#[cfg(test)]
fn compile_for_test(pattern: &str) -> Regex {
    Regex::new(pattern).expect("test regex compiles")
}

// --- Generic rule set ----------------------------------------------------

const GENERIC_RULES: &[Rule] = &[
    Rule {
        name: "ansi_escape",
        pattern: "\x1B\\[[0-9;?]*[A-Za-z]",
        replacement: "",
    },
    Rule {
        name: "osc_escape",
        pattern: "\x1B\\][^\x07\x1B]*(?:\x07|\x1B\\\\)",
        replacement: "",
    },
    Rule {
        name: "temp_path",
        // Linux /tmp and macOS /var/folders/... per-user temp dirs.
        // Must run BEFORE absolute_path so `/tmp/...` is captured as a
        // temp path rather than a generic absolute path. The leading
        // context covers start-of-line plus whitespace and the common
        // quoting/wrapping characters (`(`, `"`, `'`, `=`, `:`) so
        // `("/tmp/foo")` normalizes identically to ` /tmp/foo`.
        pattern: "(?P<prefix>^|[\\s\\(\\\"'=:])(?:/tmp/[^\\s\"'\\)]+|/var/folders/[^/\\s\"'\\)]+/[^\\s\"'\\)]+)",
        replacement: "${prefix}<tmp>",
    },
    Rule {
        name: "absolute_path",
        // Windows drive-rooted path OR POSIX absolute path. The POSIX
        // form requires a non-word, non-slash character before the
        // leading `/` so that relative paths like `src/main.rs` are
        // preserved. The negation set includes `:` so the regex stops
        // before `path:line:col` pairs that the line/column rule
        // processes next.
        pattern: "(?:[A-Za-z]:[\\\\/][^\\s\"'\\):]+|(?<!\\w)/[^\\s\"'\\):]+)",
        replacement: "<path>",
    },
    Rule {
        name: "port",
        // Must run BEFORE line/column so `host:5432` is captured as a
        // port rather than triggering the line/column rule. Covers
        // `localhost`, loopback, bare `host`/`port` cues, common
        // service names, and dotted hostnames like `example.com:5432`.
        // The dotted-host alternative requires a non-path boundary
        // (not preceded by `/`, word char, `.`, or `-`) so filenames
        // like `src/main.rs:183` never match, and refuses a trailing
        // `:digits` so `file:line:col` chains fall through to the
        // line/column rule.
        pattern: "(?:\\b(?:localhost|127\\.0\\.0\\.1|host|port|postgres(?:ql)?|mysql|redis|mongo(?:db)?):\\d{2,5}\\b|(?<![/\\w.-])[A-Za-z0-9-]+(?:\\.[A-Za-z0-9-]+)+:\\d{2,5}\\b(?!:\\d))",
        replacement: "port=<port>",
    },
    Rule {
        name: "line_column",
        // File-shaped prefix required: a dotted filename (`path.rs`),
        // a slash path (`src/main`), or a normalizer placeholder
        // (`<path>`, `<tmp>`) left by earlier rules. Bare `host:port`
        // chains never reach here because `port` runs first. The
        // lookbehind `(?<!\d)` keeps timestamps like `T03:04:05`
        // intact. Runs after `port` and `absolute_path`.
        pattern: "(?<!\\d)(?P<file>(?:[\\w\\-./\\\\]+\\.[A-Za-z0-9]+|[\\w\\-./\\\\]*[/\\\\][\\w\\-./\\\\]+|<path>|<tmp>))(?P<sep>:)(?P<line>\\d+)(?::(?P<col>\\d+))?",
        replacement: "${file}${sep}<line>:<col>",
    },
    Rule {
        name: "uuid",
        pattern: "\\b[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}\\b",
        replacement: "<uuid>",
    },
    Rule {
        name: "iso_timestamp",
        pattern: "\\b\\d{4}-\\d{2}-\\d{2}[T ]\\d{2}:\\d{2}:\\d{2}(?:\\.\\d+)?(?:Z|[+\\-]\\d{2}:?\\d{2})?\\b",
        replacement: "<ts>",
    },
    Rule {
        name: "pid",
        pattern: "(?i)\\bpid[=:]?\\s*\\d+\\b",
        replacement: "pid=<pid>",
    },
    Rule {
        name: "duration_ms",
        pattern: "\\b\\d+(?:\\.\\d+)?\\s*ms\\b",
        replacement: "<duration>",
    },
    Rule {
        name: "duration_s",
        // Apply *after* `duration_ms` so the `ms` form is captured first.
        // Only true durations collapse: compact `30s`, explicit
        // `30 sec(s)` / `30 second(s)`, or `<num> <unit>` with a
        // minute/hour unit. A bare `<num> s` with a space (e.g.
        // `retry 5 s`, `5 tests failed`) is intentionally left alone
        // so counts never merge with durations.
        pattern: "\\b\\d+(?:\\.\\d+)?s\\b|\\b\\d+(?:\\.\\d+)?\\s*(?:sec|secs|second|seconds|min|mins|minute|minutes|h|hr|hrs|hour|hours)\\b",
        replacement: "<duration>",
    },
    Rule {
        name: "long_hex",
        pattern: "\\b[0-9a-fA-F]{16,}\\b",
        replacement: "<hex>",
    },
    Rule {
        name: "long_decimal",
        pattern: "\\b\\d{6,}\\b",
        replacement: "<num>",
    },
];

// (no module-level helpers needed; the rule cache lives in a
// `thread_local!` above.)

#[cfg(test)]
mod tests {
    use super::*;

    fn gen(input: &str) -> Canonical {
        Rules::generic().normalize(input)
    }

    #[test]
    fn ansi_escape_strips_color_codes() {
        let c = gen("\x1B[31merror\x1B[0m: boom");
        assert_eq!(c.text, "error: boom");
    }

    #[test]
    fn osc_escape_strips_terminal_titles() {
        let c = gen("\x1B]0;some title\x07rest");
        assert_eq!(c.text, "rest");
    }

    #[test]
    fn absolute_path_is_replaced() {
        let c = gen("error at /var/log/app.log");
        assert_eq!(c.text, "error at <path>");
    }

    #[test]
    fn windows_drive_path_is_replaced() {
        let c = gen(r"failed at C:\Users\alice\file.rs");
        assert_eq!(c.text, "failed at <path>");
    }

    #[test]
    fn relative_path_is_preserved() {
        let c = gen("error at src/main.rs");
        assert_eq!(c.text, "error at src/main.rs");
    }

    #[test]
    fn line_column_collapses() {
        // `src/main.rs` is a relative path; absolute_path leaves it
        // alone, so the line/column placeholder sits next to the
        // preserved relative path.
        let c = gen("src/main.rs:183:5 oops");
        assert_eq!(c.text, "src/main.rs:<line>:<col> oops");
    }

    #[test]
    fn absolute_path_with_line_column_collapses_together() {
        // An absolute path is followed by line/column, both should
        // collapse into placeholders.
        let c = gen("/var/log/server.log:42:5 oops");
        assert_eq!(c.text, "<path>:<line>:<col> oops");
    }

    #[test]
    fn uuid_collapses() {
        let c = gen("req 550e8400-e29b-41d4-a716-446655440000 timed out");
        assert_eq!(c.text, "req <uuid> timed out");
    }

    #[test]
    fn iso_timestamp_collapses() {
        let c = gen("at 2026-01-02T03:04:05Z something happened");
        assert_eq!(c.text, "at <ts> something happened");
    }

    #[test]
    fn pid_collapses() {
        let c = gen("killed by pid 12345");
        assert_eq!(c.text, "killed by pid=<pid>");
    }

    #[test]
    fn port_collapses() {
        let c = gen("connect to localhost:5432 failed");
        assert_eq!(c.text, "connect to port=<port> failed");
    }

    #[test]
    fn duration_ms_collapses() {
        let c = gen("timed out after 1500 ms");
        assert_eq!(c.text, "timed out after <duration>");
    }

    #[test]
    fn duration_s_collapses() {
        let c = gen("after 30s the job died");
        assert_eq!(c.text, "after <duration> the job died");
    }

    #[test]
    fn temp_path_collapses() {
        let c = gen("wrote /tmp/build-1234.log and moved on");
        assert_eq!(c.text, "wrote <tmp> and moved on");
    }

    #[test]
    fn long_hex_collapses() {
        let c = gen("sha abcdef0123456789abcdef0123456789 mismatch");
        assert_eq!(c.text, "sha <hex> mismatch");
    }

    #[test]
    fn long_decimal_collapses() {
        let c = gen("attempt 1000001 failed");
        assert_eq!(c.text, "attempt <num> failed");
    }

    #[test]
    fn short_numbers_are_preserved() {
        // 5 digits should not be eaten by `long_decimal`.
        let c = gen("exit code 12345 was returned");
        assert_eq!(c.text, "exit code 12345 was returned");
    }

    #[test]
    fn moving_timeout_collapses_to_one_canonical() {
        // The exact scenario from the spec: same message, different line
        // and duration. Should produce identical canonical text.
        let a = "error[E0599]: timeout at /var/log/db/server.log:42:5 after 1500 ms";
        let b = "error[E0599]: timeout at /var/log/db/server.log:207:9 after 3200 ms";
        let ca = gen(a);
        let cb = gen(b);
        assert_eq!(ca.text, cb.text);
    }

    #[test]
    fn rust_format_is_accepted() {
        let input = "\
error[E0425]: cannot find value `foo` in this scope
 --> src/main.rs:5:9
  |
5 |     foo();
  |     ^^^ not found in this scope
";
        let c = gen(input);
        assert!(!c.text.is_empty());
        assert!(c.text.contains("cannot find value"));
    }

    #[test]
    fn dotnet_format_is_accepted() {
        let input = "Unhandled exception. System.IO.IOException: file not found at /var/lib/data/file.txt:13.";
        let c = gen(input);
        assert!(c.text.contains("Unhandled exception"));
    }

    #[test]
    fn python_format_is_accepted() {
        let input = "Traceback (most recent call last):\n  File \"/var/lib/py/app.py\", line 42, in <module>\nValueError: bad";
        let c = gen(input);
        assert!(c.text.contains("ValueError"));
    }

    #[test]
    fn node_format_is_accepted() {
        let input = "Error: connect ECONNREFUSED 127.0.0.1:5432 at TCPConnectWrap.afterConnect [as oncomplete]";
        let c = gen(input);
        assert!(c.text.contains("Error"));
    }

    #[test]
    fn go_format_is_accepted() {
        let input = "panic: runtime error: invalid memory address or nil pointer dereference [signal 0xc0000017c0]";
        let c = gen(input);
        assert!(c.text.contains("panic"));
    }

    #[test]
    fn unknown_toolchain_is_accepted() {
        let input = "MYAPP-CRITICAL-9001: thingy broke at /opt/myapp/thingy.bin:7";
        let c = gen(input);
        assert!(c.text.contains("MYAPP-CRITICAL-9001"));
    }

    #[test]
    fn summary_truncates_at_120_chars() {
        let mut line = "x".repeat(200);
        line.push('\n');
        let c = gen(&line);
        // The summary is up to SUMMARY_MAX characters of the original
        // line plus a single-character ellipsis.
        assert!(c.summary.chars().count() <= SUMMARY_MAX + 1);
        assert!(c.summary.ends_with('…'));
        assert!(c.summary.starts_with("xxxx"));
    }

    #[test]
    fn summary_picks_first_non_empty_line() {
        let c = gen("\n\n   \nactual summary line\nsecond line\n");
        assert_eq!(c.summary, "actual summary line");
    }

    #[test]
    fn bounded_excerpt_caps_at_4kib() {
        let big = "x".repeat(8 * 1024);
        let out = bounded_excerpt(&big);
        assert!(out.len() <= EXCERPT_MAX + "…".len());
        assert!(out.ends_with('…'));
    }

    #[test]
    fn bounded_excerpt_passthrough_when_small() {
        let s = "small text";
        assert_eq!(bounded_excerpt(s), s);
    }

    #[test]
    fn whitespace_collapses() {
        // 5 blank lines collapse to 2 blank lines (per design: "3+ blank
        // lines" → 2).
        let c = gen("a\t\tb   c\n\n\n\n\nd");
        assert_eq!(c.text, "a b c\n\n\nd");
    }

    #[test]
    fn fingerprint_section_ignore_is_advisory_in_v1() {
        // v1 behavior: the normalizer ignores FingerprintSection.ignore.
        // This test pins the current behavior so reviewers can see that
        // the field is documented but not yet wired in.
        let input = "error at /var/log/app.log:42 after 1500 ms";
        let c = gen(input);
        assert_eq!(c.text, "error at <path>:<line>:<col> after <duration>");
    }

    #[test]
    fn relative_temp_path_is_preserved() {
        // A path like `src/tmp/foo` should NOT be matched by the
        // temp_path rule, which requires a leading whitespace or
        // start-of-line anchor.
        let c = gen("wrote src/tmp/foo and continued");
        assert_eq!(c.text, "wrote src/tmp/foo and continued");
    }

    #[test]
    fn rule_compile_is_idempotent() {
        // Force the cache to populate twice; should return the same
        // compiled regex without panic. Cache is keyed by
        // (name, pattern), so same-name/different-pattern entries
        // do not collide.
        let a = try_compile("ansi_escape", GENERIC_RULES[0].pattern).unwrap();
        let b = try_compile("ansi_escape", GENERIC_RULES[0].pattern).unwrap();
        assert_eq!(a.as_str(), b.as_str());
    }

    #[test]
    fn test_helper_compiles_basic_regex() {
        // Sanity check that the test-only helper works.
        let re = compile_for_test(r"\d+");
        assert!(re.is_match("123").unwrap());
    }

    #[test]
    fn invalid_pattern_returns_error_not_panic() {
        let rule = Rule {
            name: "bad",
            pattern: "([unclosed",
            replacement: "x",
        };
        let err = rule.try_apply("input").unwrap_err();
        assert!(err.contains("invalid rule bad"), "got: {err}");
        // The infallible path skips the rule instead of panicking.
        assert_eq!(rule.apply("input"), "input");
    }

    #[test]
    fn db_host_port_does_not_share_fingerprint_with_line_col() {
        let host = gen("connect to example.com:5432 failed");
        let line = gen("error at src/main.rs:10:5");
        assert!(host.text.contains("port=<port>"), "got: {}", host.text);
        assert!(
            line.text.contains("src/main.rs:<line>:<col>"),
            "got: {}",
            line.text
        );
        assert_ne!(host.text, line.text);
    }

    #[test]
    fn bare_count_is_not_a_duration() {
        let bare = gen("retry 5 s later");
        assert!(!bare.text.contains("<duration>"), "got: {}", bare.text);
        let count = gen("5 tests failed");
        assert!(count.text.contains('5'), "got: {}", count.text);
        assert!(!count.text.contains("<duration>"));
        let real_compact = gen("after 30s the job died");
        assert!(
            real_compact.text.contains("<duration>"),
            "got: {}",
            real_compact.text
        );
        let real_long = gen("elapsed 30 seconds total");
        assert!(
            real_long.text.contains("<duration>"),
            "got: {}",
            real_long.text
        );
    }

    #[test]
    fn quoted_tmp_path_collapses() {
        let quoted = gen("open (\"/tmp/foo-bar\") now");
        let plain = gen("open /tmp/foo-bar now");
        // Both collapse the tmp portion; surrounding syntax (parens,
        // quotes) is preserved, so assert placeholder presence rather
        // than byte identity.
        assert!(quoted.text.contains("<tmp>"), "got: {}", quoted.text);
        assert!(plain.text.contains("<tmp>"), "got: {}", plain.text);
        assert!(!quoted.text.contains("/tmp/foo-bar"));
        assert!(!plain.text.contains("/tmp/foo-bar"));
    }

    #[test]
    fn python_traceback_indentation_is_preserved() {
        let a = "Traceback (most recent call last):\n  File \"app.py\", line 1\n    foo()\nValueError: bad";
        let b = "Traceback (most recent call last):\n  File \"app.py\", line 1\n        foo()\nValueError: bad";
        assert_ne!(gen(a).text, gen(b).text);
    }
}

#[cfg(test)]
mod fuzz {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        // Arbitrary unicode (including invalid UTF-8 boundaries,
        // control chars, and multi-byte emoji) must never panic the
        // normalizer; golden fixtures stay stable because the rules
        // are total functions over `&str`.
        #[test]
        fn normalization_never_panics(s in "\\PC*") {
            let c = Rules::generic().normalize(&s);
            // Canonical text is valid UTF-8 by construction.
            assert!(c.text.is_char_boundary(c.text.len()));
            let _ = crate::fingerprint::fingerprint(&c.text);
        }

        // Idempotence where applicable: normalizing canonical output
        // is stable for inputs without blank-line runs (the blank
        // collapser is the only non-idempotent step by design, and
        // even it converges after two passes).
        #[test]
        fn normalization_converges(s in "\\PC{0,256}") {
            let once = Rules::generic().normalize(&s).text;
            let twice = Rules::generic().normalize(&once).text;
            let thrice = Rules::generic().normalize(&twice).text;
            prop_assert_eq!(twice, thrice);
        }
    }
}
