//! Markdown serializer for `ExportDocument`.
//!
//! Human-readable projection of the export. Sections are always emitted
//! in the same order. Empty sections render an `_no records_` line so
//! the document is self-describing even on a fresh installation.

use crate::error::Error;

use super::dto::ExportDocument;

pub fn to_string(doc: &ExportDocument) -> Result<String, Error> {
    let mut s = String::new();
    s.push_str("# Driftwatch export\n\n");
    s.push_str(&format!(
        "- Schema version: {}\n- Exported at: {}\n- Project root: `{}`\n- Local schema version: {}\n",
        doc.schema_version,
        doc.exported_at,
        md_inline(&doc.project.root),
        doc.project.local_schema_version,
    ));
    s.push('\n');

    push_section(&mut s, "Project", |s| {
        s.push_str(&format!("- Root: `{}`\n", md_inline(&doc.project.root)));
        s.push_str(&format!(
            "- State dir: `{}`\n",
            md_inline(&doc.project.state_dir)
        ));
        s.push_str(&format!(
            "- Config: `{}`\n",
            md_inline(&doc.project.config_path)
        ));
        Ok(())
    })?;

    push_records(&mut s, "Runs", doc.runs.len(), |s| {
        s.push_str("| ID | Started | Status | Exit | Program | Tags | Hash |\n");
        s.push_str("|----|---------|--------|------|---------|------|------|\n");
        for r in &doc.runs {
            let hash = r
                .git_commit
                .as_deref()
                .map(|c| &c[..8.min(c.len())])
                .unwrap_or("-");
            s.push_str(&format!(
                "| {} | {} | {} | {} | `{}` | {} | `{}` |\n",
                r.id,
                r.started_at,
                r.status,
                r.exit_code
                    .map(|e| e.to_string())
                    .unwrap_or_else(|| "-".into()),
                md_inline(&format!("{} {}", r.program, r.argv.join(" "))),
                r.tags.join(","),
                hash,
            ));
        }
        Ok(())
    })?;

    push_records(&mut s, "Fingerprints", doc.fingerprints.len(), |s| {
        s.push_str("| Hash | Count | First seen | Last seen | Summary |\n");
        s.push_str("|------|-------|------------|-----------|---------|\n");
        for f in &doc.fingerprints {
            let prefix = short(&f.hash);
            s.push_str(&format!(
                "| `{}` | {} | {} | {} | {} |\n",
                prefix,
                f.occurrence_count,
                f.first_seen_at,
                f.last_seen_at,
                md_inline(f.summary.as_deref().unwrap_or("-")),
            ));
        }
        Ok(())
    })?;

    push_records(&mut s, "Occurrences", doc.occurrences.len(), |s| {
        s.push_str("| ID | Fingerprint | Run | Seen at |\n");
        s.push_str("|----|-------------|-----|---------|\n");
        for o in &doc.occurrences {
            s.push_str(&format!(
                "| {} | {} | {} | {} |\n",
                o.id, o.fingerprint_id, o.run_id, o.seen_at
            ));
        }
        Ok(())
    })?;

    push_records(&mut s, "Check snapshots", doc.snapshots.len(), |s| {
        s.push_str("| ID | Taken at | Checker | Status | Commit | Diagnostic |\n");
        s.push_str("|----|----------|---------|--------|--------|------------|\n");
        for sn in &doc.snapshots {
            s.push_str(&format!(
                "| {} | {} | `{}` | {} | {} | {} |\n",
                sn.id,
                sn.taken_at,
                md_inline(&sn.checker_name),
                md_inline(&sn.status),
                md_inline(sn.git_commit.as_deref().unwrap_or("-")),
                md_inline(sn.diagnostic.as_deref().unwrap_or("-")),
            ));
        }
        Ok(())
    })?;

    push_records(&mut s, "Drift alerts", doc.alerts.len(), |s| {
        s.push_str("| ID | Snapshot | Severity | Source | Symbol | Message |\n");
        s.push_str("|----|----------|----------|--------|--------|---------|\n");
        for a in &doc.alerts {
            s.push_str(&format!(
                "| {} | {} | {} | {} | {} | {} |\n",
                a.id,
                a.snapshot_id,
                a.severity,
                md_inline(a.source.as_deref().unwrap_or("-")),
                md_inline(a.symbol.as_deref().unwrap_or("-")),
                md_inline(&a.message),
            ));
        }
        Ok(())
    })?;

    push_records(&mut s, "Correlations", doc.correlations.len(), |s| {
        s.push_str("| ID | Fingerprint | Alert | Score | Message | Symbol | File | Tag | Algorithm | Label | Created |\n");
        s.push_str("|----|-------------|-------|-------|---------|--------|------|-----|-----------|-------|---------|\n");
        for c in &doc.correlations {
            s.push_str(&format!(
                "| {} | {} | {} | {:.3} | {} | {} | {} | {} | {} | {} | {} |\n",
                c.id,
                c.fingerprint_id,
                c.alert_id,
                c.score,
                fmt_opt(c.score_message),
                fmt_opt(c.score_symbol),
                fmt_opt(c.score_file),
                fmt_opt(c.score_tag),
                c.algorithm_version,
                md_inline(&c.label),
                c.created_at,
            ));
        }
        Ok(())
    })?;

    push_records(&mut s, "Manual links", doc.manual_links.len(), |s| {
        s.push_str("| ID | Fingerprint | Alert | Note | Created |\n");
        s.push_str("|----|-------------|-------|------|---------|\n");
        for l in &doc.manual_links {
            s.push_str(&format!(
                "| {} | {} | {} | {} | {} |\n",
                l.id,
                l.fingerprint_id
                    .map(|n| n.to_string())
                    .unwrap_or_else(|| "-".into()),
                l.alert_id
                    .map(|n| n.to_string())
                    .unwrap_or_else(|| "-".into()),
                md_inline(l.note.as_deref().unwrap_or("-")),
                l.created_at,
            ));
        }
        Ok(())
    })?;

    push_records(&mut s, "Gate evidence", doc.gate_artifacts.len(), |s| {
        s.push_str("| ID | Key | Kind | Producer | Digest | Available | Preview |\n");
        s.push_str("|----|-----|------|----------|--------|-----------|---------|\n");
        for g in &doc.gate_artifacts {
            s.push_str(&format!(
                "| {} | `{}` | {} | `{}` | `{}` | {} | {} |\n",
                g.id,
                md_inline(&g.key),
                md_inline(&g.kind),
                md_inline(&g.producer),
                md_inline(&short(&g.digest)),
                if g.available { "yes" } else { "no" },
                md_inline(g.preview.as_deref().unwrap_or("-")),
            ));
        }
        Ok(())
    })?;

    Ok(s)
}

fn push_section<F>(s: &mut String, name: &str, body: F) -> Result<(), Error>
where
    F: FnOnce(&mut String) -> Result<(), Error>,
{
    s.push_str(&format!("## {name}\n\n"));
    body(s)?;
    s.push('\n');
    Ok(())
}

fn push_records<F>(s: &mut String, name: &str, n: usize, body: F) -> Result<(), Error>
where
    F: FnOnce(&mut String) -> Result<(), Error>,
{
    s.push_str(&format!("## {name} ({n})\n\n"));
    if n == 0 {
        s.push_str("_no records_\n\n");
        return Ok(());
    }
    body(s)?;
    s.push('\n');
    Ok(())
}

fn md_inline(s: &str) -> String {
    s.replace('|', "\\|")
        .replace('\n', " ")
        .chars()
        .take(160)
        .collect()
}

fn short(h: &str) -> String {
    if h.len() >= 12 {
        h[..12].to_string()
    } else {
        h.to_string()
    }
}

fn fmt_opt(v: Option<f64>) -> String {
    match v {
        Some(x) => format!("{:.3}", x),
        None => "-".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::export::dto::*;

    fn empty_doc() -> ExportDocument {
        ExportDocument {
            schema_version: SCHEMA_VERSION,
            exported_at: "2026-01-01T00:00:00Z".into(),
            project: ProjectExport {
                root: "/p".into(),
                state_dir: "/p/.driftwatch".into(),
                config_path: "/p/driftwatch.toml".into(),
                local_schema_version: 1,
            },
            runs: vec![],
            fingerprints: vec![],
            occurrences: vec![],
            snapshots: vec![],
            alerts: vec![],
            correlations: vec![],
            manual_links: vec![],
            gate_artifacts: vec![],
        }
    }

    #[test]
    fn empty_doc_renders_all_sections() {
        let s = to_string(&empty_doc()).unwrap();
        for section in [
            "# Driftwatch export",
            "## Project",
            "## Runs (0)",
            "## Fingerprints (0)",
            "## Occurrences (0)",
            "## Check snapshots (0)",
            "## Drift alerts (0)",
            "## Correlations (0)",
            "## Manual links (0)",
            "_no records_",
        ] {
            assert!(s.contains(section), "missing section {section} in:\n{s}");
        }
    }

    #[test]
    fn pipe_and_newline_in_summary_is_escaped() {
        let mut doc = empty_doc();
        doc.fingerprints.push(FingerprintExport {
            id: 1,
            hash: "deadbeefdeadbeefdeadbeefdeadbeef".into(),
            canonical: "boom".into(),
            summary: Some("a|b\nc".into()),
            first_seen_at: "2026-01-01T00:00:00Z".into(),
            last_seen_at: "2026-01-01T00:00:00Z".into(),
            occurrence_count: 1,
        });
        let s = to_string(&doc).unwrap();
        assert!(s.contains("a\\|b c"));
        assert!(!s.contains("a|b\nc"));
    }
}
