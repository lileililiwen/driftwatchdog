//! `driftwatch show <bug-id>`: render a single recurring bug.

use std::path::Path;

use crate::cli::ShowArgs;
use crate::error::Error;
use crate::project::ProjectRoot;
use crate::repo::{bugs::Bugs, Db};

/// Run the `show` command. Always returns exit 0 on success; returns
/// a nonzero exit code when the identifier cannot be resolved.
pub fn show(args: ShowArgs, cwd: &Path) -> Result<i32, Error> {
    let proj = ProjectRoot::discover(cwd)?;
    let mut db = Db::open(&proj.db_path)?;
    let out = render(&mut db, &args.bug_id)?;
    print!("{out}");
    Ok(0)
}

/// Render a single bug as a human-readable report. Returns the same
/// bytes the `driftwatch show` command prints to stdout, so the MCP
/// `show_bug` tool can return it byte-for-byte. `bug_id` accepts the
/// documented forms: hash prefix, numeric `fingerprints.id`, or
/// `id:<n>`. The `Db` is borrowed mutably to satisfy the existing
/// repository signatures; the caller's read-only connection is
/// sufficient because this function never writes.
pub fn render(db: &mut Db, bug_id: &str) -> Result<String, Error> {
    let bugs = Bugs::new(db);
    let fp = resolve(&bugs, bug_id)?.ok_or_else(|| Error::BugNotFound {
        id: bug_id.to_string(),
    })?;
    let r = bugs.report_for(fp.id)?;

    let mut s = String::new();
    s.push_str(&format!("Bug  : {}\n", short_hash(&fp.hash)));
    s.push_str(&format!("ID   : #{} (full hash {})\n", fp.id, fp.hash));
    s.push_str(&format!(
        "Summary      : {}\n",
        fp.summary.as_deref().unwrap_or("(no summary)")
    ));
    s.push_str(&format!("Occurrences  : {}\n", fp.occurrence_count));
    s.push_str(&format!("First seen   : {}\n", fp.first_seen_at));
    s.push_str(&format!("Last seen    : {}\n", fp.last_seen_at));
    s.push_str(&format!("Distinct runs: {}\n", r.distinct_runs));
    s.push('\n');
    s.push_str("Recent commits:\n");
    if r.recent_commits.is_empty() {
        s.push_str("  (no git context captured)\n");
    } else {
        for c in r.recent_commits.iter().take(10) {
            s.push_str(&format!("  {}  @ {}\n", short_hash(&c.commit), c.seen_at));
        }
    }
    s.push('\n');
    s.push_str("Recent evidence (last 5):\n");
    if r.occurrences.is_empty() {
        s.push_str("  (none)\n");
    } else {
        for o in r.occurrences.iter().take(5) {
            s.push_str(&format!(
                "--- run #{} @ {} (commit {})\n",
                o.run_id,
                o.seen_at,
                o.git_commit
                    .as_deref()
                    .map(short_hash)
                    .unwrap_or_else(|| "-".to_string())
            ));
            if let Some(ex) = &o.excerpt {
                for line in ex.lines().take(8) {
                    s.push_str(&format!("    {line}\n"));
                }
            }
        }
    }
    Ok(s)
}

fn resolve(bugs: &Bugs<'_>, id: &str) -> Result<Option<crate::repo::bugs::Fingerprint>, Error> {
    let trimmed = id.trim();
    // Explicit `id:<n>` always means the numeric primary key.
    if let Some(rest) = trimmed.strip_prefix("id:") {
        let n: i64 = rest
            .trim()
            .parse()
            .map_err(|_| Error::BugNotFound { id: id.to_string() })?;
        return bugs.find_by_id(n);
    }
    // Otherwise try the hash prefix first so a digit-only string
    // that happens to match a hash never silently resolves to the
    // wrong numeric row. Ambiguity errors propagate (never fall
    // back to id). Only when no hash matches AND the input is
    // all digits do we try the numeric id.
    match bugs.find_by_hash_prefix(trimmed)? {
        Some(fp) => Ok(Some(fp)),
        None => {
            if !trimmed.is_empty() && trimmed.chars().all(|c| c.is_ascii_digit()) {
                if let Ok(n) = trimmed.parse::<i64>() {
                    if let Some(fp) = bugs.find_by_id(n)? {
                        return Ok(Some(fp));
                    }
                }
            }
            Ok(None)
        }
    }
}

fn short_hash(h: &str) -> String {
    if h.len() >= 12 {
        h[..12].to_string()
    } else {
        h.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_hash_truncates_long_strings() {
        let h = "abcdef0123456789";
        assert_eq!(short_hash(h), "abcdef012345");
    }

    #[test]
    fn short_hash_passes_through_short_strings() {
        assert_eq!(short_hash("abc"), "abc");
    }
}
