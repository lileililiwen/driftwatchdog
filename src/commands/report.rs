//! `driftwatch report`: render a Markdown report of recurring bugs.

use std::path::Path;

use crate::cli::ReportArgs;
use crate::error::Error;
use crate::project::ProjectRoot;
use crate::repo::{bugs::Bugs, Db};

/// Render a Markdown report. Always exits 0 on success.
pub fn report(args: ReportArgs, cwd: &Path) -> Result<i32, Error> {
    let proj = ProjectRoot::discover(cwd)?;
    let mut db = Db::open(&proj.db_path)?;
    let bugs = Bugs::new(&mut db);

    let cutoff = (chrono::Utc::now() - chrono::Duration::days(args.days)).to_rfc3339();
    let tag_like = args.tag.as_deref().map(|t| format!("%\"{}\"%", t));
    let top = bugs.top(args.limit, &cutoff, tag_like.as_deref())?;

    println!("# Driftwatch report");
    println!();
    println!(
        "_Window: last {} days, limit {}. Tags: {}_",
        args.days,
        args.limit,
        args.tag.as_deref().unwrap_or("any")
    );
    println!();
    if top.is_empty() {
        println!("No recurring failures in the selected window.");
        return Ok(0);
    }
    println!("## Top recurring failures");
    println!();
    println!("| # | Bug | Count | First seen | Last seen | Summary |");
    println!("|---|-----|-------|------------|-----------|---------|");
    for (i, t) in top.iter().enumerate() {
        let bug = short_hash(&t.hash);
        let s = t
            .summary
            .clone()
            .unwrap_or_else(|| "(no summary)".to_string());
        println!(
            "| {} | `{}` | {} | {} | {} | {} |",
            i + 1,
            bug,
            t.count,
            t.first_seen_at,
            t.last_seen_at,
            md_escape(&s)
        );
    }
    println!();
    println!("## Recent commits across top failures");
    println!();
    for t in &top {
        let fp = match bugs.find_by_hash(&t.hash)? {
            Some(fp) => fp,
            None => continue,
        };
        let commits = bugs.recent_commits(fp.id, 5)?;
        if commits.is_empty() {
            continue;
        }
        let title = t.summary.clone().unwrap_or_default();
        println!("### {} — {}", short_hash(&t.hash), md_escape(&title));
        for c in &commits {
            println!("- {} @ {}", short_hash(&c.commit), c.seen_at);
        }
        println!();
    }
    println!("## Bug trend (last 7 days)");
    println!();
    println!("| Bug | Today | -1d | -2d | -3d | -4d | -5d | -6d |");
    println!("|-----|-------|-----|-----|-----|-----|-----|-----|");
    for t in &top {
        let cells = trend_cells(&db, &t.hash, 7);
        println!("| `{}` | {} |", short_hash(&t.hash), cells.join(" | "));
    }
    Ok(0)
}

fn trend_cells(db: &Db, hash: &str, days: usize) -> Vec<String> {
    use rusqlite::params;
    let today = chrono::Utc::now().date_naive();
    let mut out = Vec::with_capacity(days);
    for d in 0..days {
        let day = today - chrono::Duration::days(d as i64);
        let start = day.and_hms_opt(0, 0, 0).unwrap().and_utc().to_rfc3339();
        let end = (day + chrono::Duration::days(1))
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_utc()
            .to_rfc3339();
        let n: i64 = db
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM occurrences o
                 JOIN fingerprints f ON o.fingerprint_id = f.id
                 WHERE f.hash = ?1 AND o.seen_at >= ?2 AND o.seen_at < ?3",
                params![hash, start, end],
                |r| r.get(0),
            )
            .unwrap_or(0);
        out.push(n.to_string());
    }
    out
}

fn short_hash(h: &str) -> String {
    if h.len() >= 8 {
        h[..8].to_string()
    } else {
        h.to_string()
    }
}

fn md_escape(s: &str) -> String {
    s.replace('|', "\\|").replace('\n', " ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_hash_truncates() {
        assert_eq!(short_hash("abcdef0123456789"), "abcdef01");
    }

    #[test]
    fn md_escape_replaces_pipes_and_newlines() {
        assert_eq!(md_escape("a|b\nc"), "a\\|b c");
    }
}
