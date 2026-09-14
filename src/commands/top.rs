//! `driftwatch top` orchestration. Shows grouped recurring failures or, when
//! no fingerprint data is present, the spec's empty-state message.

use std::path::Path;

use crate::cli::TopArgs;
use crate::error::Error;
use crate::project::ProjectRoot;
use crate::repo::{bugs::Bugs, Db};
use crate::util::truncate_char_boundary;

/// Run the `top` command. Always returns exit 0 on success.
pub fn top(args: TopArgs, cwd: &Path) -> Result<i32, Error> {
    let proj = ProjectRoot::discover(cwd)?;
    let mut db = Db::open(&proj.db_path)?;
    let out = render(&args, &mut db)?;
    print!("{out}");
    Ok(0)
}

/// Render the `top` table as a string. Returns the same bytes the
/// `driftwatch top` command prints to stdout, so the MCP `top_bugs`
/// tool can reuse the same builder. `db` is borrowed mutably to
/// match the existing repository signatures; the caller's
/// read-only connection is sufficient because this function never
/// writes.
pub fn render(args: &TopArgs, db: &mut Db) -> Result<String, Error> {
    let cutoff = (chrono::Utc::now() - chrono::Duration::days(args.days)).to_rfc3339();
    let tag_like = args.tag.as_deref().map(crate::repo::tag_like_pattern);

    let rows = Bugs::new(db).top(args.limit, &cutoff, tag_like.as_deref())?;

    if rows.is_empty() {
        return Ok("No recurring failures in the selected window.\n".to_string());
    }

    let mut s = String::new();
    s.push_str(&format!(
        "{:<7}  {:<20}  {:<20}  {:<10}  SUMMARY\n",
        "COUNT", "FIRST SEEN", "LAST SEEN", "HASH"
    ));
    for r in rows {
        let summary = r
            .summary
            .unwrap_or_else(|| "(no summary yet — fingerprinting pending)".to_string());
        let summary = truncate_char_boundary(&summary, 59);
        let hash = if r.hash.len() >= 8 {
            &r.hash[..8]
        } else {
            &r.hash
        };
        s.push_str(&format!(
            "{:<7}  {:<20}  {:<20}  {:<10}  {}\n",
            r.count, r.first_seen_at, r.last_seen_at, hash, summary
        ));
    }
    Ok(s)
}
