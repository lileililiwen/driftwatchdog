//! `driftwatch report`: render a Markdown report of recurring bugs.

use std::io::Write;
use std::path::Path;

use crate::cli::ReportArgs;
use crate::error::Error;
use crate::project::ProjectRoot;
use crate::repo::{bugs::Bugs, Db};

/// Render a Markdown report. Always exits 0 on success. When
/// `args.ai` is set, delegates to `report_ai::render_ai` and prints
/// the AI-oriented Markdown context instead.
pub fn report(args: ReportArgs, cwd: &Path) -> Result<i32, Error> {
    let proj = ProjectRoot::discover(cwd)?;
    let mut db = Db::open(&proj.db_path)?;

    if args.ai {
        let md = crate::commands::report_ai::render_ai(&args, &proj, &mut db)?;
        let stdout = std::io::stdout();
        let mut handle = stdout.lock();
        handle.write_all(md.as_bytes())?;
        handle.flush()?;
        return Ok(0);
    }

    let bugs = Bugs::new(&mut db);

    let cutoff = (chrono::Utc::now() - chrono::Duration::days(args.days)).to_rfc3339();
    let tag_like = args.tag.as_deref().map(crate::repo::tag_like_pattern);
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
    // One aggregate query for all trend cells (no N+1), with DB
    // errors propagated instead of swallowed.
    let trends = trend_counts(&db, 7)?;
    for t in &top {
        let cells = trend_cells_for(&trends, &t.hash, 7);
        println!("| `{}` | {} |", short_hash(&t.hash), cells.join(" | "));
    }
    Ok(0)
}

/// Per-(hash, UTC-day) occurrence counts for the last `days` days in
/// a single aggregate query. `seen_at` values carrying non-UTC
/// offsets are normalized by SQLite's `date()` (which applies the
/// offset) so runs land on the correct UTC day. Errors propagate to
/// the caller.
fn trend_counts(
    db: &Db,
    days: usize,
) -> Result<std::collections::HashMap<(String, String), i64>, Error> {
    use rusqlite::params;
    let today = chrono::Utc::now().date_naive();
    let earliest = today - chrono::Duration::days(days as i64 - 1);
    let start = earliest
        .and_hms_opt(0, 0, 0)
        .unwrap()
        .and_utc()
        .to_rfc3339();
    let mut stmt = db.conn().prepare(
        "SELECT f.hash, date(o.seen_at) AS day, COUNT(*)
         FROM occurrences o
         JOIN fingerprints f ON o.fingerprint_id = f.id
         WHERE o.seen_at >= ?1
         GROUP BY f.hash, day",
    )?;
    let rows = stmt.query_map(params![start], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, i64>(2)?,
        ))
    })?;
    let mut map = std::collections::HashMap::new();
    for row in rows {
        let (hash, day, n) = row?;
        map.insert((hash, day), n);
    }
    Ok(map)
}

fn trend_cells_for(
    trends: &std::collections::HashMap<(String, String), i64>,
    hash: &str,
    days: usize,
) -> Vec<String> {
    let today = chrono::Utc::now().date_naive();
    (0..days)
        .map(|d| {
            let day = today - chrono::Duration::days(d as i64);
            trends
                .get(&(hash.to_string(), day.format("%Y-%m-%d").to_string()))
                .copied()
                .unwrap_or(0)
                .to_string()
        })
        .collect()
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
