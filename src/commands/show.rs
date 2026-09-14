//! `driftwatch show <bug-id>`: render a single recurring bug.

use std::path::Path;

use crate::cli::ShowArgs;
use crate::error::Error;
use crate::project::ProjectRoot;
use crate::repo::{bugs::Bugs, Db};

/// Render a single bug by hash prefix (8+ hex chars preferred) or
/// numeric `fingerprints.id`. Returns a nonzero exit code when the
/// identifier cannot be resolved.
pub fn show(args: ShowArgs, cwd: &Path) -> Result<i32, Error> {
    let proj = ProjectRoot::discover(cwd)?;
    let mut db = Db::open(&proj.db_path)?;
    let bugs = Bugs::new(&mut db);

    let fp = resolve(&bugs, &args.bug_id)?.ok_or_else(|| Error::BugNotFound {
        id: args.bug_id.clone(),
    })?;
    let r = bugs.report_for(fp.id)?;

    println!("Bug  : {}", short_hash(&fp.hash));
    println!("ID   : #{} (full hash {})", fp.id, fp.hash);
    println!(
        "Summary      : {}",
        fp.summary.as_deref().unwrap_or("(no summary)")
    );
    println!("Occurrences  : {}", fp.occurrence_count);
    println!("First seen   : {}", fp.first_seen_at);
    println!("Last seen    : {}", fp.last_seen_at);
    println!("Distinct runs: {}", r.distinct_runs);
    println!();
    println!("Recent commits:");
    if r.recent_commits.is_empty() {
        println!("  (no git context captured)");
    } else {
        for c in r.recent_commits.iter().take(10) {
            println!("  {}  @ {}", short_hash(&c.commit), c.seen_at);
        }
    }
    println!();
    println!("Recent evidence (last 5):");
    if r.occurrences.is_empty() {
        println!("  (none)");
    } else {
        for o in r.occurrences.iter().take(5) {
            println!(
                "--- run #{} @ {} (commit {})",
                o.run_id,
                o.seen_at,
                o.git_commit
                    .as_deref()
                    .map(short_hash)
                    .unwrap_or_else(|| "-".to_string())
            );
            if let Some(ex) = &o.excerpt {
                for line in ex.lines().take(8) {
                    println!("    {line}");
                }
            }
        }
    }
    Ok(0)
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
