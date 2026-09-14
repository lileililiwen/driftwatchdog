//! `driftwatch report --ai`: AI-oriented Markdown context.
//!
//! Emits a five-section Markdown document intended to be pasted
//! into an AI agent's context:
//!
//! 1. **Project context** — root, state dir, config, schema version.
//! 2. **Recurring failures** — same `Bugs::top` table as the
//!    standard report.
//! 3. **Current spec violations** — every recorded alert.
//! 4. **Possible relationships** — both heuristic correlations
//!    (from `correlations`) and manual links (from `manual_links`).
//!    Cautious labels: "possible relationship", "heuristic
//!    correlation". Never claims a similarity score is a root cause.
//! 5. **Investigation task** — explicit instructions for the agent.
//!
//! The render is deterministic for a given state. It never invokes
//! an LLM API and never reads network resources.

use std::path::Path;

use crate::cli::ReportArgs;
use crate::error::Error;
use crate::project::ProjectRoot;
use crate::repo::{alerts::Alerts, bugs::Bugs, correlations::Correlations, links::Links, Db};

/// Render the AI-context report. Returns the Markdown as a `String`
/// so the caller can decide how to print it; today `report`
/// prints to stdout.
pub fn render_ai(args: &ReportArgs, proj: &ProjectRoot, db: &mut Db) -> Result<String, Error> {
    let now = chrono::Utc::now().to_rfc3339();
    let cutoff = (chrono::Utc::now() - chrono::Duration::days(args.days)).to_rfc3339();
    let tag_like = args.tag.as_deref().map(crate::repo::tag_like_pattern);

    // Reads happen sequentially because each repo struct holds a
    // borrow on `db`. The mutable borrow is released between calls
    // so we can mix the `&mut Db` (Bugs) and `&Db` (Alerts,
    // Correlations, Links) variants.
    let top = Bugs::new(db).top(args.limit, &cutoff, tag_like.as_deref())?;
    let alert_rows = Alerts::new(db).list_alerts()?;
    let mut correlation_rows = Correlations::new(db).list_all()?;
    let link_rows = Links::new(db).list_all()?;

    // Lazy fallback: when the table is empty but both fingerprints
    // and alerts exist, run the correlator once so the AI report
    // has something to display. A failure here is a warning, not a
    // hard error; the report still renders with the cautious
    // empty-state message.
    if correlation_rows.is_empty() && !top.is_empty() && !alert_rows.is_empty() {
        if let Err(e) = crate::correlate::run_after_check(db) {
            eprintln!("driftwatch: correlation skipped: {e}");
        } else {
            correlation_rows = Correlations::new(db).list_all()?;
        }
    }

    render_ai_markdown(
        args,
        proj,
        db,
        &now,
        &cutoff,
        top,
        alert_rows,
        correlation_rows,
        link_rows,
    )
}

/// Render the AI-context report for the MCP server. Identical to
/// [`render_ai`] but skips the lazy correlation fallback because
/// the MCP server opens the database read-only and a write attempt
/// would fail at the driver level. Without the lazy fallback the
/// report renders deterministically from the existing state and
/// never produces the "correlation skipped" warning on stderr.
pub fn render_ai_for_mcp(
    args: &ReportArgs,
    proj: &ProjectRoot,
    db: &mut Db,
) -> Result<String, Error> {
    let now = chrono::Utc::now().to_rfc3339();
    let cutoff = (chrono::Utc::now() - chrono::Duration::days(args.days)).to_rfc3339();
    let tag_like = args.tag.as_deref().map(crate::repo::tag_like_pattern);

    let top = Bugs::new(db).top(args.limit, &cutoff, tag_like.as_deref())?;
    let alert_rows = Alerts::new(db).list_alerts()?;
    let correlation_rows = Correlations::new(db).list_all()?;
    let link_rows = Links::new(db).list_all()?;

    render_ai_markdown(
        args,
        proj,
        db,
        &now,
        &cutoff,
        top,
        alert_rows,
        correlation_rows,
        link_rows,
    )
}

#[allow(clippy::too_many_arguments)]
fn render_ai_markdown(
    args: &ReportArgs,
    proj: &ProjectRoot,
    db: &Db,
    now: &str,
    _cutoff: &str,
    top: Vec<crate::repo::bugs::TopRow>,
    alert_rows: Vec<crate::repo::alerts::Alert>,
    correlation_rows: Vec<crate::repo::correlations::Correlation>,
    link_rows: Vec<crate::repo::links::ManualLink>,
) -> Result<String, Error> {
    let local_schema_version: i64 = db
        .conn()
        .query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_version",
            [],
            |r| r.get(0),
        )
        .map_err(Error::Sqlite)?;

    let mut s = String::new();
    s.push_str("# Driftwatch AI context\n\n");
    s.push_str(&format!(
        "_Generated at {now}. Window: last {} days. Tags: {}_",
        args.days,
        args.tag.as_deref().unwrap_or("any")
    ));
    s.push_str("\n\n");

    push_section(&mut s, "Project context");
    s.push_str(&format!("- Root: `{}`\n", path_display(&proj.root)));
    s.push_str(&format!(
        "- State dir: `{}`\n",
        path_display(&proj.state_dir)
    ));
    s.push_str(&format!(
        "- Config: `{}`\n",
        path_display(&proj.config_path)
    ));
    s.push_str(&format!("- Local schema version: {local_schema_version}\n"));
    s.push('\n');

    push_section(&mut s, "Recurring failures");
    if top.is_empty() {
        s.push_str("No recurring failures in the selected window.\n\n");
    } else {
        s.push_str("| # | Bug | Count | First seen | Last seen | Summary |\n");
        s.push_str("|---|-----|-------|------------|-----------|---------|\n");
        for (i, t) in top.iter().enumerate() {
            let bug = short_hash(&t.hash);
            let summary = md_escape(t.summary.clone().unwrap_or_else(|| "(no summary)".into()));
            s.push_str(&format!(
                "| {} | `{}` | {} | {} | {} | {} |\n",
                i + 1,
                bug,
                t.count,
                t.first_seen_at,
                t.last_seen_at,
                summary
            ));
        }
        s.push('\n');
    }

    push_section(&mut s, "Current spec violations");
    if alert_rows.is_empty() {
        s.push_str("No current spec violations recorded.\n\n");
    } else {
        s.push_str("| ID | Severity | Source | Symbol | Message |\n");
        s.push_str("|----|----------|--------|--------|---------|\n");
        for a in &alert_rows {
            s.push_str(&format!(
                "| {} | {} | {} | {} | {} |\n",
                a.id,
                a.severity,
                md_inline(a.source.clone().unwrap_or_else(|| "-".into())),
                md_inline(a.symbol.clone().unwrap_or_else(|| "-".into())),
                md_inline(a.message.clone())
            ));
        }
        s.push('\n');
    }

    push_section(&mut s, "Possible relationships");
    if correlation_rows.is_empty() && link_rows.is_empty() {
        s.push_str("_No possible relationships were recorded._\n\n");
    } else {
        let has_heuristic = !correlation_rows.is_empty();
        let has_manual = !link_rows.is_empty();
        if has_heuristic {
            s.push_str("_Heuristic correlations are leads, not root-cause claims._\n\n");
        }
        if has_manual {
            s.push_str("_Manual links reflect explicit user assertions._\n\n");
        }
        s.push_str("| Type | Bug | Alert | Score | Algorithm | Note |\n");
        s.push_str("|------|-----|-------|-------|-----------|------|\n");
        for c in &correlation_rows {
            s.push_str(&format!(
                "| heuristic | `{}` | `{}` | {:.3} | {} | — |\n",
                short_hash_id(c.fingerprint_id),
                c.alert_id,
                c.score,
                c.algorithm_version
            ));
        }
        for l in &link_rows {
            let bug = l
                .fingerprint_id
                .map(short_hash_id)
                .unwrap_or_else(|| "-".to_string());
            let alert = l
                .alert_id
                .map(|n| n.to_string())
                .unwrap_or_else(|| "-".into());
            let note = md_inline(l.note.clone().unwrap_or_else(|| "—".into()));
            s.push_str(&format!(
                "| manual | `{bug}` | `{alert}` | — | — | {note} |\n"
            ));
        }
        s.push('\n');
    }

    push_section(&mut s, "Investigation task");
    s.push_str("- Investigate the **recurring failures** above; each row has multiple occurrences and is therefore a candidate for regression coverage.\n");
    s.push_str("- For each row, run `driftwatch show <bug-id>` to see the full canonical message, recent commits, and recent evidence excerpts.\n");
    s.push_str("- Review the **current spec violations** section. Read the `Source` column entries before changing application behavior; do not modify specs merely to silence warnings.\n");
    s.push_str("- For each **possible relationship** (heuristic or manual), inspect the linked bug and alert, form a hypothesis, and either add a regression test or document why the similarity is coincidental.\n");
    s.push_str("- Do not fix only the most recent occurrence; recurrence memory exists so the same bug is not rediscovered each session.\n");

    Ok(s)
}

fn push_section(s: &mut String, name: &str) {
    s.push_str(&format!("## {name}\n\n"));
}

fn short_hash(h: &str) -> String {
    if h.len() >= 8 {
        h[..8].to_string()
    } else {
        h.to_string()
    }
}

fn short_hash_id(id: i64) -> String {
    format!("#{id}")
}

fn md_escape(s: String) -> String {
    s.replace('|', "\\|").replace('\n', " ")
}

fn md_inline(s: String) -> String {
    s.replace('|', "\\|")
        .replace('\n', " ")
        .chars()
        .take(160)
        .collect()
}

fn path_display(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::root::ProjectRoot;
    use crate::repo::alerts::{Alerts, NewAlert, NewSnapshot};
    use crate::repo::bugs::Bugs;
    use crate::repo::links::Links;
    use crate::similarity::score::ComponentScores;

    fn proj_at(tmp: &tempfile::TempDir) -> ProjectRoot {
        ProjectRoot::at(tmp.path())
    }

    fn seed_failure_and_alert(db: &mut Db) -> (i64, i64) {
        // Use identical text on both sides so the heuristic
        // correlator always scores above the 0.65 threshold. The
        // seen_at timestamp is `now` so the row passes the default
        // 30-day cutoff in `Bugs::top`.
        let canonical = "DbPool connection refused to db.md while reading pool";
        let now = chrono::Utc::now().to_rfc3339();
        let fp = Bugs::new(db)
            .upsert_for_occurrence(canonical, canonical, &now)
            .unwrap();
        Alerts::record_run(
            db,
            &NewSnapshot {
                taken_at: &now,
                checker_name: "spec",
                status: "success",
                diagnostic: None,
                raw_json: None,
                git_commit: None,
                git_branch: None,
            },
            &[NewAlert {
                severity: "warning",
                message: canonical,
                source: Some("specs/db.md"),
                symbol: Some("DbPool"),
            }],
        )
        .unwrap();
        let alert_id: i64 = db
            .conn()
            .query_row("SELECT id FROM drift_alerts LIMIT 1", [], |r| r.get(0))
            .unwrap();
        (fp.id, alert_id)
    }

    fn args() -> ReportArgs {
        ReportArgs {
            limit: 20,
            days: 30,
            tag: None,
            ai: true,
        }
    }

    #[test]
    fn render_ai_contains_all_required_sections_on_empty_db() {
        let tmp = tempfile::tempdir().unwrap();
        let mut db = Db::open_in_memory().unwrap();
        let proj = proj_at(&tmp);
        let md = render_ai(&args(), &proj, &mut db).unwrap();
        for section in [
            "# Driftwatch AI context",
            "## Project context",
            "## Recurring failures",
            "## Current spec violations",
            "## Possible relationships",
            "## Investigation task",
            "No recurring failures in the selected window.",
            "No current spec violations recorded.",
            "_No possible relationships were recorded._",
        ] {
            assert!(md.contains(section), "missing {section} in:\n{md}");
        }
    }

    #[test]
    fn render_ai_includes_investigation_task_even_when_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let mut db = Db::open_in_memory().unwrap();
        let proj = proj_at(&tmp);
        let md = render_ai(&args(), &proj, &mut db).unwrap();
        assert!(md.contains("## Investigation task"));
        assert!(md.contains("regression coverage"));
    }

    #[test]
    fn render_ai_marks_manual_link_type_as_manual() {
        let tmp = tempfile::tempdir().unwrap();
        let mut db = Db::open_in_memory().unwrap();
        let proj = proj_at(&tmp);
        let (fp_id, alert_id) = seed_failure_and_alert(&mut db);
        let now = "2026-01-01T00:00:00Z";
        let _ = Links::create(
            &mut db,
            Some(fp_id),
            Some(alert_id),
            Some("investigate this"),
            now,
        )
        .unwrap();
        let md = render_ai(&args(), &proj, &mut db).unwrap();
        assert!(md.contains("| manual |"));
        assert!(md.contains("investigate this"));
    }

    #[test]
    fn render_ai_marks_heuristic_correlation_type_as_heuristic() {
        let tmp = tempfile::tempdir().unwrap();
        let mut db = Db::open_in_memory().unwrap();
        let proj = proj_at(&tmp);
        let (fp_id, alert_id) = seed_failure_and_alert(&mut db);
        let scores = ComponentScores {
            message: 1.0,
            symbol: 1.0,
            file: 1.0,
            tag: 0.0,
            total: 0.9,
        };
        crate::repo::correlations::Correlations::new(&db)
            .upsert(fp_id, alert_id, scores, "2026-01-01T00:00:00Z")
            .unwrap();
        let md = render_ai(&args(), &proj, &mut db).unwrap();
        assert!(md.contains("| heuristic |"));
        assert!(md.contains("0.900"));
        assert!(md.contains(crate::similarity::score::ALGO_VERSION));
    }

    #[test]
    fn render_ai_prompts_cautious_label_for_heuristic_rows() {
        let tmp = tempfile::tempdir().unwrap();
        let mut db = Db::open_in_memory().unwrap();
        let proj = proj_at(&tmp);
        let (fp_id, alert_id) = seed_failure_and_alert(&mut db);
        let scores = ComponentScores {
            message: 1.0,
            symbol: 1.0,
            file: 1.0,
            tag: 0.0,
            total: 0.9,
        };
        crate::repo::correlations::Correlations::new(&db)
            .upsert(fp_id, alert_id, scores, "2026-01-01T00:00:00Z")
            .unwrap();
        let md = render_ai(&args(), &proj, &mut db).unwrap();
        assert!(md.contains("Heuristic correlations are leads, not root-cause claims"));
    }

    #[test]
    fn render_ai_lazy_fallback_runs_correlation_on_empty_table() {
        // Pre-populate a fingerprint and an alert but no
        // correlation row. The fallback should run the correlator
        // and produce a row.
        let tmp = tempfile::tempdir().unwrap();
        let mut db = Db::open_in_memory().unwrap();
        let proj = proj_at(&tmp);
        let (fp_id, alert_id) = seed_failure_and_alert(&mut db);
        assert_eq!(
            crate::repo::correlations::Correlations::new(&db)
                .count()
                .unwrap(),
            0
        );
        let _ = render_ai(&args(), &proj, &mut db).unwrap();
        // After rendering, the table should be non-empty.
        let n = crate::repo::correlations::Correlations::new(&db)
            .count()
            .unwrap();
        assert!(n > 0, "lazy fallback did not persist a correlation");
        let _ = (fp_id, alert_id);
    }
}
