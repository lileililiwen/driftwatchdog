//! Built-in `source-file-size` Gate concern.
//!
//! This module owns a repository-aware, language-independent source
//! boundary. It discovers repository-owned source files, counts raw
//! newline bytes like `wc -l`, and returns a normalized [`GateResult`]:
//!
//! * `PASS` when every candidate is at or below the configured
//!   maximum (including the "zero candidates" case);
//! * `FAIL` with one bounded finding per oversized file;
//! * `REVIEW_REQUIRED` when the Git boundary, ignore evaluation, a path,
//!   or a candidate read fails, so incomplete coverage is never turned
//!   into a pass.
//!
//! The scanner never parses a language, never follows a symlink, never
//! leaves the repository root, never executes a discovered file, and
//! never writes or uploads source. It is dispatched in-process by
//! [`crate::commands::gate`]; no project command is required.

use std::collections::BTreeSet;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use crate::gate::concerns::SOURCE_FILE_SIZE;
use crate::gate::dto::{
    MAX_DIAGNOSTIC_BYTES, MAX_FINDINGS, MAX_LOCATION_BYTES, MAX_MISSING_EVIDENCE,
    MAX_REMEDIATION_BYTES, MAX_TITLE_BYTES,
};
use crate::gate::manifest::SourceSizePolicy;
use crate::gate::redact::{bound_text, bounded_diagnostic};
use crate::gate::types::{Finding, GateResult, GateSeverity, GateStatus};

/// Result `source` label for the built-in scanner.
const SOURCE_LABEL: &str = "driftwatchdog";
/// Stable rule id recorded on every oversized-file finding.
const RULE_ID: &str = "source-file-size";
/// Per-`git` invocation budget.
const GIT_TIMEOUT: Duration = Duration::from_secs(5);
/// Streaming read chunk size for line counting.
const READ_CHUNK: usize = 64 * 1024;
/// Hard ceiling on directory entries examined in one non-Git walk.
const MAX_WALK_ENTRIES: usize = 200_000;

/// Default path segments excluded from discovery. Segment-based and
/// language-independent: dependency, vendor, build, state, coverage,
/// generated, fixture, and test trees.
const DEFAULT_EXCLUDED_SEGMENTS: &[&str] = &[
    ".git",
    ".driftwatch",
    "node_modules",
    "target",
    "vendor",
    "third_party",
    "thirdparty",
    "dist",
    "build",
    ".venv",
    "venv",
    "coverage",
    "generated",
    "fixtures",
    "test",
    "tests",
    "__tests__",
];

/// Conventional source roots used when no explicit `include` is set.
const CONVENTIONAL_SOURCE_ROOTS: &[&str] = &[
    "src", "app", "lib", "bin", "cmd", "internal", "packages", "server", "client",
];

/// Scan `root` under `policy` and return the normalized concern result.
///
/// Pure with respect to repository state: the scanner only reads. Git is
/// consulted when the root contains a `.git` entry; a missing Git binary
/// selects the non-Git walk fallback, while a failing Git repository is a
/// review condition.
pub fn scan(root: &Path, policy: &SourceSizePolicy) -> GateResult {
    let files = match enumerate_repo_files(root, policy) {
        Ok(files) => files,
        Err(err) => return boundary_review(&err),
    };
    let candidates = select_candidates(&files, policy);

    let mut findings: Vec<Finding> = Vec::new();
    let mut errors: Vec<String> = Vec::new();
    let mut missing: Vec<String> = Vec::new();
    let mut evaluated: usize = 0;

    for rel in &candidates {
        let abs = root.join(rel);
        // Never follow a symlink: `symlink_metadata` does not traverse it.
        match std::fs::symlink_metadata(&abs) {
            Ok(md) if md.file_type().is_file() => {}
            Ok(_) => continue,
            Err(e) => {
                record_read_error(rel, &e, &mut errors, &mut missing);
                continue;
            }
        }
        match count_file_lines(&abs) {
            Ok(FileCount::NonText) => continue,
            Ok(FileCount::Text { lines }) => {
                evaluated += 1;
                if lines > u64::from(policy.max_lines) {
                    findings.push(violation_finding(rel, lines, policy.max_lines));
                }
            }
            Err(e) => record_read_error(rel, &e, &mut errors, &mut missing),
        }
    }

    if !errors.is_empty() {
        return review_result(&errors, &missing);
    }
    if !findings.is_empty() {
        return fail_result(findings, evaluated, policy);
    }
    pass_result(candidates.len(), evaluated, policy)
}

/// Enumerate every repository file the boundary should consider. In a
/// Git repository this is `git ls-files --cached --others
/// --exclude-standard`; otherwise it is a bounded, symlink-free walk.
fn enumerate_repo_files(
    root: &Path,
    policy: &SourceSizePolicy,
) -> Result<Vec<String>, BoundaryError> {
    if root.join(".git").exists() {
        return match git_ls_files(root) {
            Ok(files) => Ok(files),
            // A missing Git binary is not an error: use the walk fallback.
            Err(BoundaryError::GitUnavailable) => walk_files(root, policy),
            Err(err) => Err(err),
        };
    }
    walk_files(root, policy)
}

/// Run `git ls-files` bounded by [`GIT_TIMEOUT`], returning raw paths.
fn git_ls_files(root: &Path) -> Result<Vec<String>, BoundaryError> {
    let root_buf = root.to_path_buf();
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let output = Command::new("git")
            .args([
                "ls-files",
                "--cached",
                "--others",
                "--exclude-standard",
                "-z",
            ])
            .current_dir(&root_buf)
            .output();
        let _ = tx.send(output);
    });
    let output = match rx.recv_timeout(GIT_TIMEOUT) {
        Ok(output) => output,
        Err(_) => return Err(BoundaryError::Git("git ls-files timed out".to_string())),
    };
    match output {
        // Spawn failure (usually `git` not installed): fall back to walk.
        Err(_) => Err(BoundaryError::GitUnavailable),
        Ok(out) if out.status.success() => {
            let text = String::from_utf8_lossy(&out.stdout);
            Ok(text
                .split('\0')
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string())
                .collect())
        }
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr);
            let detail = if stderr.trim().is_empty() {
                format!("git ls-files exited with {:?}", out.status.code())
            } else {
                stderr.trim().to_string()
            };
            Err(BoundaryError::Git(detail))
        }
    }
}

/// Recursively walk `root` without following symlinks, applying the
/// root `.gitignore` and path exclusions. Returns repository-relative
/// `/`-separated paths.
fn walk_files(root: &Path, policy: &SourceSizePolicy) -> Result<Vec<String>, BoundaryError> {
    let rules = load_gitignore(root)?;
    let mut out: Vec<String> = Vec::new();
    let mut budget = MAX_WALK_ENTRIES;
    walk_dir(root, "", &rules, policy, &mut out, &mut budget)?;
    Ok(out)
}

fn walk_dir(
    root: &Path,
    rel_dir: &str,
    rules: &[IgnoreRule],
    policy: &SourceSizePolicy,
    out: &mut Vec<String>,
    budget: &mut usize,
) -> Result<(), BoundaryError> {
    let dir = if rel_dir.is_empty() {
        root.to_path_buf()
    } else {
        root.join(rel_dir)
    };
    let entries = std::fs::read_dir(&dir)
        .map_err(|e| BoundaryError::Walk(format!("cannot read {rel_dir}: {e}")))?;
    let mut items: Vec<(String, PathBuf)> = Vec::new();
    for entry in entries {
        if *budget == 0 {
            return Err(BoundaryError::Walk(format!(
                "directory walk exceeded {MAX_WALK_ENTRIES} entries"
            )));
        }
        *budget -= 1;
        let entry =
            entry.map_err(|e| BoundaryError::Walk(format!("cannot read {rel_dir}: {e}")))?;
        let name = entry.file_name().to_string_lossy().to_string();
        items.push((name, entry.path()));
    }
    items.sort_by(|a, b| a.0.cmp(&b.0));

    for (name, path) in items {
        let rel = if rel_dir.is_empty() {
            name
        } else {
            format!("{rel_dir}/{name}")
        };
        let file_type = std::fs::symlink_metadata(&path)
            .map_err(|e| BoundaryError::Walk(format!("cannot stat {rel}: {e}")))?
            .file_type();
        if file_type.is_symlink() {
            continue;
        }
        if file_type.is_dir() {
            if has_default_excluded_segment(&rel) {
                continue;
            }
            if policy
                .exclude
                .iter()
                .any(|p| pattern_matches_with_ancestors(p, &rel))
            {
                continue;
            }
            if gitignore_ignores(rules, &rel, true) {
                continue;
            }
            walk_dir(root, &rel, rules, policy, out, budget)?;
        } else if file_type.is_file() && !gitignore_ignores(rules, &rel, false) {
            out.push(rel);
        }
    }
    Ok(())
}

/// Read the root `.gitignore`; a missing file is an empty rule set.
fn load_gitignore(root: &Path) -> Result<Vec<IgnoreRule>, BoundaryError> {
    let path = root.join(".gitignore");
    match std::fs::read_to_string(&path) {
        Ok(text) => Ok(parse_gitignore(&text)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(BoundaryError::Ignore(format!(".gitignore: {e}"))),
    }
}

/// Filter enumerated files down to the candidate set: default and
/// configured excludes always win; `include` (or conventional source
/// roots when it is empty) selects the remainder.
fn select_candidates(files: &[String], policy: &SourceSizePolicy) -> Vec<String> {
    let mut out: BTreeSet<String> = BTreeSet::new();
    for raw in files {
        let Some(rel) = normalize_rel(raw) else {
            continue;
        };
        if rel.is_empty() || has_default_excluded_segment(&rel) {
            continue;
        }
        if policy
            .exclude
            .iter()
            .any(|p| pattern_matches_with_ancestors(p, &rel))
        {
            continue;
        }
        if policy.include.is_empty() {
            if !under_source_root(&rel) {
                continue;
            }
        } else if !policy.include.iter().any(|p| pattern_matches(p, &rel)) {
            continue;
        }
        out.insert(rel);
    }
    // `BTreeSet` guarantees lexicographic order and deduplication.
    out.into_iter().collect()
}

/// Normalize a raw path to a repository-relative `/`-separated path.
/// Absolute paths and `..` traversal are rejected (returns `None`).
fn normalize_rel(raw: &str) -> Option<String> {
    let replaced = raw.replace('\\', "/");
    let trimmed = replaced.strip_prefix("./").unwrap_or(&replaced);
    if trimmed.is_empty() || trimmed.starts_with('/') {
        return None;
    }
    let mut parts: Vec<&str> = Vec::new();
    for component in trimmed.split('/') {
        match component {
            "" | "." => {}
            ".." => return None,
            other => parts.push(other),
        }
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("/"))
    }
}

fn under_source_root(rel: &str) -> bool {
    rel.split('/')
        .next()
        .is_some_and(|seg| CONVENTIONAL_SOURCE_ROOTS.contains(&seg))
}

fn has_default_excluded_segment(rel: &str) -> bool {
    rel.split('/')
        .any(|seg| DEFAULT_EXCLUDED_SEGMENTS.contains(&seg))
}

/// True when `pattern` matches the repository-relative `rel_path`. A
/// pattern containing `/` is anchored at the root and matched against the
/// whole path; a slash-free pattern matches any single path segment
/// (gitignore basename semantics).
fn pattern_matches(pattern: &str, rel_path: &str) -> bool {
    let (pat_segs, path_segs, anchored) = split_pattern(pattern, rel_path);
    if pat_segs.is_empty() || path_segs.is_empty() {
        return false;
    }
    if anchored {
        match_segments(&pat_segs, &path_segs)
    } else {
        path_segs.iter().any(|seg| segment_match(pat_segs[0], seg))
    }
}

/// True when `pattern` matches `rel_path` or one of its directory
/// ancestors. Used for `exclude`, where an anchored directory pattern
/// (e.g. `packages/service/generated`) excludes everything beneath it.
fn pattern_matches_with_ancestors(pattern: &str, rel_path: &str) -> bool {
    if pattern_matches(pattern, rel_path) {
        return true;
    }
    let (pat_segs, path_segs, anchored) = split_pattern(pattern, rel_path);
    if !anchored || pat_segs.is_empty() || path_segs.len() < 2 {
        return false;
    }
    (1..path_segs.len()).any(|end| match_segments(&pat_segs, &path_segs[..end]))
}

fn split_pattern<'a>(pattern: &'a str, rel_path: &'a str) -> (Vec<&'a str>, Vec<&'a str>, bool) {
    let trimmed = pattern.trim();
    let anchored = trimmed.starts_with('/') || trimmed.contains('/');
    let pattern = trimmed.strip_prefix('/').unwrap_or(trimmed);
    let pat_segs: Vec<&str> = pattern.split('/').filter(|s| !s.is_empty()).collect();
    let path_segs: Vec<&str> = rel_path
        .trim_matches('/')
        .split('/')
        .filter(|s| !s.is_empty())
        .collect();
    (pat_segs, path_segs, anchored)
}

/// Match `/`-split glob pattern segments against path segments, where a
/// `**` segment matches zero or more path segments.
fn match_segments(pattern: &[&str], path: &[&str]) -> bool {
    if pattern.is_empty() {
        return path.is_empty();
    }
    if pattern[0] == "**" {
        return (0..=path.len()).any(|skip| match_segments(&pattern[1..], &path[skip..]));
    }
    if path.is_empty() {
        return false;
    }
    if !segment_match(pattern[0], path[0]) {
        return false;
    }
    match_segments(&pattern[1..], &path[1..])
}

/// Match one glob segment (`*`, `?`, `[...]`; no `/`) against a path
/// segment. Unknown/partial classes treat `[` literally.
fn segment_match(pattern: &str, text: &str) -> bool {
    let p: Vec<char> = pattern.chars().collect();
    let t: Vec<char> = text.chars().collect();
    let (mut pi, mut ti) = (0usize, 0usize);
    let mut star: Option<usize> = None;
    let mut star_text = 0usize;
    while ti < t.len() {
        if pi < p.len() {
            match p[pi] {
                '?' => {
                    pi += 1;
                    ti += 1;
                    continue;
                }
                '*' => {
                    star = Some(pi);
                    star_text = ti;
                    pi += 1;
                    continue;
                }
                '[' => {
                    if let Some((matched, next)) = class_match(&p, pi, t[ti]) {
                        if matched {
                            pi = next;
                            ti += 1;
                            continue;
                        }
                    } else if t[ti] == '[' {
                        pi += 1;
                        ti += 1;
                        continue;
                    }
                }
                c if c == t[ti] => {
                    pi += 1;
                    ti += 1;
                    continue;
                }
                _ => {}
            }
        }
        if let Some(sp) = star {
            pi = sp + 1;
            star_text += 1;
            ti = star_text;
        } else {
            return false;
        }
    }
    while pi < p.len() && p[pi] == '*' {
        pi += 1;
    }
    pi == p.len()
}

/// Match a `[...]` character class starting at `p[start] == '['`.
/// Returns `Some((matched, index_after_class))` when the class is
/// well-formed, or `None` when the `[` should be treated literally.
fn class_match(p: &[char], start: usize, ch: char) -> Option<(bool, usize)> {
    let mut i = start + 1;
    let negate = matches!(p.get(i), Some('!') | Some('^'));
    if negate {
        i += 1;
    }
    let mut matched = false;
    let mut first = true;
    while i < p.len() {
        if p[i] == ']' && !first {
            return Some((matched ^ negate, i + 1));
        }
        first = false;
        if i + 2 < p.len() && p[i + 1] == '-' && p[i + 2] != ']' {
            let (lo, hi) = (p[i], p[i + 2]);
            if lo <= ch && ch <= hi {
                matched = true;
            }
            i += 3;
        } else {
            if p[i] == ch {
                matched = true;
            }
            i += 1;
        }
    }
    None
}

/// One parsed `.gitignore` rule (root `.gitignore` only in the non-Git
/// fallback).
#[derive(Debug, Clone, PartialEq, Eq)]
struct IgnoreRule {
    negated: bool,
    dir_only: bool,
    anchored: bool,
    pattern: String,
}

/// Parse `.gitignore` text into ordered rules. Blank lines and `#`
/// comments are skipped; a leading `!` negates; a trailing `/` marks a
/// directory-only pattern; a leading `/` anchors the pattern.
fn parse_gitignore(text: &str) -> Vec<IgnoreRule> {
    let mut rules = Vec::new();
    for raw in text.lines() {
        let line = raw.trim_end();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut rule = IgnoreRule {
            negated: false,
            dir_only: false,
            anchored: false,
            pattern: String::new(),
        };
        let mut body = line;
        if let Some(rest) = body.strip_prefix('!') {
            rule.negated = true;
            body = rest;
        }
        if let Some(rest) = body.strip_suffix('/') {
            rule.dir_only = true;
            body = rest;
        }
        if let Some(rest) = body.strip_prefix('/') {
            rule.anchored = true;
            body = rest;
        }
        if body.is_empty() {
            continue;
        }
        rule.pattern = body.to_string();
        rules.push(rule);
    }
    rules
}

/// Apply ordered `.gitignore` rules (last match wins) to `rel`.
fn gitignore_ignores(rules: &[IgnoreRule], rel: &str, is_dir: bool) -> bool {
    let mut ignored = false;
    for rule in rules {
        if ignore_rule_matches(rule, rel, is_dir) {
            ignored = !rule.negated;
        }
    }
    ignored
}

fn ignore_rule_matches(rule: &IgnoreRule, rel: &str, is_dir: bool) -> bool {
    let path_segs: Vec<&str> = rel
        .trim_matches('/')
        .split('/')
        .filter(|s| !s.is_empty())
        .collect();
    if path_segs.is_empty() {
        return false;
    }
    let pat_segs: Vec<&str> = rule.pattern.split('/').filter(|s| !s.is_empty()).collect();
    if pat_segs.is_empty() {
        return false;
    }
    // A directory-only rule can never match a file leaf.
    let max_end = if rule.dir_only && !is_dir {
        path_segs.len().saturating_sub(1)
    } else {
        path_segs.len()
    };
    if max_end == 0 {
        return false;
    }
    if rule.anchored {
        (1..=max_end).any(|end| match_segments(&pat_segs, &path_segs[..end]))
    } else {
        (0..max_end).any(|start| {
            (start + 1..=max_end).any(|end| match_segments(&pat_segs, &path_segs[start..end]))
        })
    }
}

/// Outcome of reading one candidate file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FileCount {
    /// A UTF-8 text file with this many `0x0A` bytes.
    Text { lines: u64 },
    /// Binary or invalid UTF-8 content: not source, skipped.
    NonText,
}

/// Count `0x0A` bytes in a file without holding it in memory, treating
/// NUL bytes or invalid UTF-8 as non-source. A final unterminated line
/// does not increment the count (`wc -l` semantics).
fn count_file_lines(path: &Path) -> std::io::Result<FileCount> {
    let file = File::open(path)?;
    let mut reader = BufReader::with_capacity(READ_CHUNK, file);
    let mut buf = vec![0u8; READ_CHUNK];
    let mut tail: Vec<u8> = Vec::new();
    let mut lines: u64 = 0;
    loop {
        let read = reader.read(&mut buf)?;
        if read == 0 {
            break;
        }
        let chunk = &buf[..read];
        if chunk.contains(&0) {
            return Ok(FileCount::NonText);
        }
        if tail.is_empty() {
            if !accumulate_utf8(chunk, &mut lines, &mut tail) {
                return Ok(FileCount::NonText);
            }
        } else {
            let mut combined = std::mem::take(&mut tail);
            combined.extend_from_slice(chunk);
            if !accumulate_utf8(&combined, &mut lines, &mut tail) {
                return Ok(FileCount::NonText);
            }
        }
    }
    if tail.is_empty() {
        Ok(FileCount::Text { lines })
    } else {
        // Trailing bytes are an incomplete UTF-8 sequence: not text.
        Ok(FileCount::NonText)
    }
}

/// Validate `bytes`, count newlines in the valid prefix, and stash any
/// incomplete trailing sequence in `tail`. Returns `false` when the
/// bytes contain an invalid UTF-8 sequence.
fn accumulate_utf8(bytes: &[u8], lines: &mut u64, tail: &mut Vec<u8>) -> bool {
    match std::str::from_utf8(bytes) {
        Ok(text) => {
            *lines += text.bytes().filter(|b| *b == b'\n').count() as u64;
            true
        }
        Err(e) if e.error_len().is_some() => false,
        Err(e) => {
            let valid = e.valid_up_to();
            *lines += bytes[..valid].iter().filter(|b| **b == b'\n').count() as u64;
            tail.extend_from_slice(&bytes[valid..]);
            true
        }
    }
}

fn violation_finding(rel: &str, lines: u64, max_lines: u32) -> Finding {
    Finding {
        title: bound_text(
            &format!("{rel} has {lines} physical lines; maximum is {max_lines}"),
            MAX_TITLE_BYTES,
        ),
        severity: GateSeverity::Error,
        location: Some(bound_text(rel, MAX_LOCATION_BYTES)),
        rule: Some(RULE_ID.to_string()),
    }
}

fn record_read_error(
    rel: &str,
    error: &std::io::Error,
    errors: &mut Vec<String>,
    missing: &mut Vec<String>,
) {
    errors.push(format!("cannot read {rel}: {error}"));
    if missing.len() < MAX_MISSING_EVIDENCE {
        missing.push(bound_text(
            &format!("{SOURCE_FILE_SIZE}:read:{rel}"),
            crate::gate::dto::MAX_EVIDENCE_KEY_BYTES,
        ));
    }
}

fn base_result(status: GateStatus, severity: GateSeverity) -> GateResult {
    GateResult {
        gate_id: SOURCE_FILE_SIZE.to_string(),
        source: SOURCE_LABEL.to_string(),
        status,
        severity,
        findings: Vec::new(),
        evidence: Vec::new(),
        missing_evidence: Vec::new(),
        diagnostic: None,
        remediation: None,
    }
}

fn pass_result(candidates: usize, evaluated: usize, policy: &SourceSizePolicy) -> GateResult {
    let mut result = base_result(GateStatus::Pass, GateSeverity::Info);
    result.diagnostic = Some(bounded_diagnostic(
        &if candidates == 0 {
            format!(
                "no candidate files matched the source boundary (maximum {} physical line(s))",
                policy.max_lines
            )
        } else {
            format!(
                "evaluated {evaluated} of {candidates} candidate file(s); maximum {} physical line(s)",
                policy.max_lines
            )
        },
        MAX_DIAGNOSTIC_BYTES,
    ));
    result
}

fn fail_result(
    mut findings: Vec<Finding>,
    evaluated: usize,
    policy: &SourceSizePolicy,
) -> GateResult {
    let mut result = base_result(GateStatus::Fail, GateSeverity::Error);
    let mut truncated = false;
    if findings.len() > MAX_FINDINGS {
        findings.truncate(MAX_FINDINGS);
        truncated = true;
    }
    let violating = findings.len();
    result.findings = findings;
    result.diagnostic = Some(bounded_diagnostic(
        &format!(
            "{violating} of {evaluated} evaluated file(s) exceed the maximum of {} physical line(s){}",
            policy.max_lines,
            if truncated {
                format!("; findings truncated to {MAX_FINDINGS}")
            } else {
                String::new()
            }
        ),
        MAX_DIAGNOSTIC_BYTES,
    ));
    result.remediation = Some(bound_text(
        "Split or reduce the oversized source file(s) below the configured maximum, then re-run.",
        MAX_REMEDIATION_BYTES,
    ));
    result
}

fn review_result(errors: &[String], missing: &[String]) -> GateResult {
    let mut result = base_result(GateStatus::ReviewRequired, GateSeverity::Warning);
    result.missing_evidence = missing.iter().take(MAX_MISSING_EVIDENCE).cloned().collect();
    result.diagnostic = Some(bounded_diagnostic(&errors.join("; "), MAX_DIAGNOSTIC_BYTES));
    result.remediation = Some(bound_text(
        "Resolve the repository boundary or read error so the source-size scan can complete.",
        MAX_REMEDIATION_BYTES,
    ));
    result
}

fn boundary_review(err: &BoundaryError) -> GateResult {
    let missing = vec![match err {
        BoundaryError::Git(_) => format!("{SOURCE_FILE_SIZE}:git"),
        BoundaryError::Ignore(_) => format!("{SOURCE_FILE_SIZE}:gitignore"),
        BoundaryError::Walk(_) => format!("{SOURCE_FILE_SIZE}:walk"),
        BoundaryError::GitUnavailable => format!("{SOURCE_FILE_SIZE}:git"),
    }];
    review_result(&[err.to_string()], &missing)
}

/// Boundary-resolution failures that prevent complete evaluation.
#[derive(Debug, Clone, PartialEq, Eq)]
enum BoundaryError {
    /// Git is not installed: the caller falls back to the walk.
    GitUnavailable,
    /// A real Git repository error (nonzero exit, timeout).
    Git(String),
    /// `.gitignore` could not be evaluated.
    Ignore(String),
    /// The non-Git directory walk failed.
    Walk(String),
}

impl std::fmt::Display for BoundaryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BoundaryError::GitUnavailable => {
                write!(f, "git is unavailable; used the directory walk")
            }
            BoundaryError::Git(detail) => write!(f, "git boundary failed: {detail}"),
            BoundaryError::Ignore(detail) => write!(f, "cannot evaluate .gitignore: {detail}"),
            BoundaryError::Walk(detail) => write!(f, "directory walk failed: {detail}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn policy(max_lines: u32, include: &[&str], exclude: &[&str]) -> SourceSizePolicy {
        SourceSizePolicy {
            max_lines,
            include: include.iter().map(|s| s.to_string()).collect(),
            exclude: exclude.iter().map(|s| s.to_string()).collect(),
        }
    }

    fn write_lines(root: &Path, rel: &str, newlines: usize) -> PathBuf {
        let path = root.join(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        let body = "x\n".repeat(newlines);
        fs::write(&path, body).unwrap();
        path
    }

    #[test]
    fn counts_newline_bytes_like_wc() {
        let tmp = tempdir().unwrap();
        write_lines(tmp.path(), "src/feature.rs", 1000);
        let path = tmp.path().join("src/feature.rs");
        assert_eq!(
            count_file_lines(&path).unwrap(),
            FileCount::Text { lines: 1000 }
        );
    }

    #[test]
    fn unterminated_final_line_is_not_counted() {
        let tmp = tempdir().unwrap();
        let path = tmp.path().join("src/feature.rs");
        fs::create_dir_all(tmp.path().join("src")).unwrap();
        fs::write(&path, "x\nx\nx").unwrap();
        assert_eq!(
            count_file_lines(&path).unwrap(),
            FileCount::Text { lines: 2 }
        );
    }

    #[test]
    fn binary_and_invalid_utf8_are_skipped() {
        let tmp = tempdir().unwrap();
        fs::create_dir_all(tmp.path().join("src")).unwrap();
        let nul = tmp.path().join("src/nul.rs");
        fs::write(&nul, b"line\n\0binary\n").unwrap();
        assert_eq!(count_file_lines(&nul).unwrap(), FileCount::NonText);

        let bad = tmp.path().join("src/bad.rs");
        fs::write(&bad, [0xff, 0xfe, b'\n']).unwrap();
        assert_eq!(count_file_lines(&bad).unwrap(), FileCount::NonText);
    }

    #[test]
    fn boundary_at_1000_passes_and_1001_fails() {
        let tmp = tempdir().unwrap();
        write_lines(tmp.path(), "src/ok.rs", 1000);
        let result = scan(tmp.path(), &policy(1000, &[], &[]));
        assert_eq!(result.status, GateStatus::Pass);
        assert!(result.findings.is_empty());

        write_lines(tmp.path(), "src/big.rs", 1001);
        let result = scan(tmp.path(), &policy(1000, &[], &[]));
        assert_eq!(result.status, GateStatus::Fail);
        assert_eq!(result.findings.len(), 1);
        let finding = &result.findings[0];
        assert_eq!(finding.severity, GateSeverity::Error);
        assert_eq!(finding.location.as_deref(), Some("src/big.rs"));
        assert_eq!(finding.rule.as_deref(), Some(RULE_ID));
        assert!(
            finding.title.contains("1001 physical lines"),
            "{}",
            finding.title
        );
        assert!(
            finding.title.contains("maximum is 1000"),
            "{}",
            finding.title
        );
    }

    #[test]
    fn conventional_source_root_is_discovered() {
        let tmp = tempdir().unwrap();
        write_lines(tmp.path(), "src/feature.rs", 10);
        // A root-level README is not a source file under default discovery.
        fs::write(tmp.path().join("README.md"), "x\n".repeat(5000)).unwrap();
        let result = scan(tmp.path(), &policy(1000, &[], &[]));
        assert_eq!(result.status, GateStatus::Pass);
        let diagnostic = result.diagnostic.unwrap_or_default();
        assert!(diagnostic.contains("1 candidate"), "got: {diagnostic}");
    }

    #[test]
    fn no_candidates_passes_with_evidence() {
        let tmp = tempdir().unwrap();
        let result = scan(tmp.path(), &policy(1000, &[], &[]));
        assert_eq!(result.status, GateStatus::Pass);
        assert!(result
            .diagnostic
            .as_deref()
            .unwrap()
            .contains("no candidate files"));
    }

    #[test]
    fn default_exclusions_prune_dependency_trees() {
        let tmp = tempdir().unwrap();
        write_lines(tmp.path(), "src/ok.rs", 5);
        write_lines(tmp.path(), "node_modules/dep/index.js", 10_000);
        write_lines(tmp.path(), "target/debug/build.rs", 10_000);
        let result = scan(tmp.path(), &policy(1000, &[], &[]));
        assert_eq!(result.status, GateStatus::Pass, "{:?}", result.diagnostic);
    }

    #[test]
    fn explicit_include_overrides_discovery() {
        let tmp = tempdir().unwrap();
        write_lines(tmp.path(), "packages/service/a.rs", 2000);
        write_lines(tmp.path(), "src/huge.rs", 5000);
        // Only `packages/service/**` is included, so the oversized
        // `src/huge.rs` is not a candidate.
        let result = scan(tmp.path(), &policy(1000, &["packages/service/**"], &[]));
        assert_eq!(result.status, GateStatus::Fail);
        assert_eq!(result.findings.len(), 1);
        assert_eq!(
            result.findings[0].location.as_deref(),
            Some("packages/service/a.rs")
        );
    }

    #[test]
    fn exclude_wins_over_include() {
        let tmp = tempdir().unwrap();
        write_lines(tmp.path(), "packages/service/generated/a.rs", 5000);
        write_lines(tmp.path(), "packages/service/keep.rs", 10);
        let result = scan(
            tmp.path(),
            &policy(
                1000,
                &["packages/service/**"],
                &["packages/service/generated/**"],
            ),
        );
        assert_eq!(result.status, GateStatus::Pass, "{:?}", result.diagnostic);
    }

    #[test]
    fn slashless_exclude_matches_any_depth() {
        let tmp = tempdir().unwrap();
        write_lines(tmp.path(), "src/vendorized/big.rs", 5000);
        let excluded = scan(tmp.path(), &policy(1000, &[], &["vendorized"]));
        assert_eq!(excluded.status, GateStatus::Pass);
        let included = scan(tmp.path(), &policy(1000, &[], &[]));
        assert_eq!(included.status, GateStatus::Fail);
    }

    #[test]
    fn gitignore_is_applied_in_non_git_walk() {
        let tmp = tempdir().unwrap();
        fs::write(tmp.path().join(".gitignore"), "ignored/\n*.log\n").unwrap();
        write_lines(tmp.path(), "src/ok.rs", 5);
        write_lines(tmp.path(), "ignored/big.rs", 5000);
        write_lines(tmp.path(), "src/debug.log", 5000);
        let result = scan(tmp.path(), &policy(1000, &[], &[]));
        assert_eq!(result.status, GateStatus::Pass, "{:?}", result.diagnostic);
    }

    #[test]
    fn gitignore_negation_reincludes() {
        let tmp = tempdir().unwrap();
        fs::write(tmp.path().join(".gitignore"), "src/*.rs\n!src/keep.rs\n").unwrap();
        write_lines(tmp.path(), "src/keep.rs", 5000);
        write_lines(tmp.path(), "src/drop.rs", 5000);
        let result = scan(tmp.path(), &policy(1000, &[], &[]));
        assert_eq!(result.status, GateStatus::Fail);
        assert_eq!(result.findings.len(), 1);
        assert_eq!(result.findings[0].location.as_deref(), Some("src/keep.rs"));
    }

    #[test]
    fn symlinked_files_are_not_followed() {
        let tmp = tempdir().unwrap();
        write_lines(tmp.path(), "src/ok.rs", 5);
        let target = write_lines(tmp.path(), "outside/big.rs", 5000);
        #[cfg(unix)]
        std::os::unix::fs::symlink(&target, tmp.path().join("src/link.rs")).unwrap();
        #[cfg(not(unix))]
        let _ = target;
        let result = scan(tmp.path(), &policy(1000, &[], &[]));
        assert_eq!(result.status, GateStatus::Pass, "{:?}", result.diagnostic);
    }

    #[test]
    fn findings_are_deterministically_ordered() {
        let tmp = tempdir().unwrap();
        write_lines(tmp.path(), "src/z.rs", 2000);
        write_lines(tmp.path(), "src/a.rs", 2000);
        write_lines(tmp.path(), "src/m.rs", 2000);
        let result = scan(tmp.path(), &policy(1000, &[], &[]));
        let paths: Vec<&str> = result
            .findings
            .iter()
            .filter_map(|f| f.location.as_deref())
            .collect();
        assert_eq!(paths, vec!["src/a.rs", "src/m.rs", "src/z.rs"]);
    }

    #[test]
    fn read_failure_is_review_required_and_does_not_abort() {
        let tmp = tempdir().unwrap();
        write_lines(tmp.path(), "src/ok.rs", 2000); // independent failure still recorded
        let broken = write_lines(tmp.path(), "src/broken.rs", 5);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&broken).unwrap().permissions();
            perms.set_mode(0o000);
            fs::set_permissions(&broken, perms).unwrap();
        }
        let result = scan(tmp.path(), &policy(1000, &[], &[]));
        #[cfg(unix)]
        {
            // Root bypasses file permissions; only assert the review path
            // when the read actually fails.
            if unsafe { libc::geteuid() } != 0 {
                assert_eq!(result.status, GateStatus::ReviewRequired);
                assert!(result
                    .missing_evidence
                    .iter()
                    .any(|m| m.contains("read:src/broken.rs")));
            }
        }
        #[cfg(not(unix))]
        let _ = result;
    }

    #[test]
    fn git_boundary_failure_is_review_required() {
        let tmp = tempdir().unwrap();
        write_lines(tmp.path(), "src/ok.rs", 5);
        // An empty `.git` makes `git ls-files` exit non-zero.
        fs::create_dir_all(tmp.path().join(".git")).unwrap();
        let result = scan(tmp.path(), &policy(1000, &[], &[]));
        assert_eq!(
            result.status,
            GateStatus::ReviewRequired,
            "{:?}",
            result.diagnostic
        );
        assert!(result
            .missing_evidence
            .iter()
            .any(|m| m == "source-file-size:git"));
    }

    #[test]
    fn pattern_matching_covers_common_globs() {
        assert!(pattern_matches("src/**", "src/a/b.rs"));
        assert!(pattern_matches("*.rs", "src/a.rs"));
        assert!(!pattern_matches("*.rs", "src/a.js"));
        assert!(pattern_matches(
            "packages/service/*",
            "packages/service/a.rs"
        ));
        assert!(!pattern_matches(
            "packages/service/*",
            "packages/service/x/a.rs"
        ));
        assert!(pattern_matches("src/generated/**", "src/generated"));
        assert!(pattern_matches("**/test/**", "a/test/b.rs"));
        assert!(segment_match("a?c", "abc"));
        assert!(segment_match("[a-c]x", "bx"));
        assert!(!segment_match("[!a-c]x", "bx"));
    }

    #[test]
    fn normalize_rel_rejects_escapes_and_absolute_paths() {
        assert_eq!(normalize_rel("./src/a.rs").as_deref(), Some("src/a.rs"));
        assert_eq!(normalize_rel("src\\a.rs").as_deref(), Some("src/a.rs"));
        assert_eq!(normalize_rel("/etc/passwd"), None);
        assert_eq!(normalize_rel("../escape.rs"), None);
        assert_eq!(normalize_rel(""), None);
    }
}
