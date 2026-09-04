//! Read the local SQLite database and assemble an `ExportDocument`.
//!
//! This is the only place that knows how to map from repository row
//! types to the versioned DTOs in [`super::dto`]. It tolerates empty
//! alert / correlation / link tables so the export is usable before
//! those later changes land.

use rusqlite::Connection;

use crate::error::Error;
use crate::project::ProjectRoot;
use crate::repo::{
    alerts::{schema_version, Alert, Alerts, Snapshot},
    bugs::Bugs,
    correlations::Correlations,
    links::Links,
    runs::Runs,
    Db,
};

use super::dto::{
    AlertExport, CorrelationExport, ExportDocument, FingerprintExport, ManualLinkExport,
    OccurrenceExport, ProjectExport, RunExport, SnapshotExport, SCHEMA_VERSION,
};

/// Build an `ExportDocument` from the open database at `db` rooted at
/// `proj`. `exported_at` is the timestamp embedded in the document.
pub fn build(db: &mut Db, proj: &ProjectRoot) -> Result<ExportDocument, Error> {
    let runs = Runs::new(db).all(100_000)?;
    let mut fingerprints = Bugs::new(db).all()?;
    // Sort fingerprints by hash for stable, content-ordered output.
    fingerprints.sort_by(|a, b| a.hash.cmp(&b.hash));
    let occurrences = load_occurrences(db.conn())?;
    let alerts_repo = Alerts::new(db);
    let snapshots = alerts_repo.list_snapshots()?;
    let alerts = alerts_repo.list_alerts()?;
    let correlations = Correlations::new(db).list_all()?;
    let manual_links = Links::new(db).list_all()?;
    let local_schema_version = schema_version(db.conn())?;

    Ok(ExportDocument {
        schema_version: SCHEMA_VERSION,
        exported_at: chrono::Utc::now().to_rfc3339(),
        project: ProjectExport {
            root: path_to_string(&proj.root),
            state_dir: path_to_string(&proj.state_dir),
            config_path: path_to_string(&proj.config_path),
            local_schema_version,
        },
        runs: runs.into_iter().map(run_to_dto).collect(),
        fingerprints: fingerprints.into_iter().map(fp_to_dto).collect(),
        occurrences: occurrences.into_iter().map(occ_to_dto).collect(),
        snapshots: snapshots.into_iter().map(snap_to_dto).collect(),
        alerts: alerts.into_iter().map(alert_to_dto).collect(),
        correlations: correlations.into_iter().map(corr_to_dto).collect(),
        manual_links: manual_links.into_iter().map(link_to_dto).collect(),
    })
}

fn load_occurrences(conn: &Connection) -> Result<Vec<OccRow>, Error> {
    let mut stmt = conn.prepare(
        "SELECT o.id, o.fingerprint_id, o.run_id, o.seen_at, o.excerpt,
                r.git_commit, r.git_branch
         FROM occurrences o
         JOIN runs r ON o.run_id = r.id
         ORDER BY o.id",
    )?;
    let rows = stmt
        .query_map([], |r| {
            Ok(OccRow {
                id: r.get(0)?,
                fingerprint_id: r.get(1)?,
                run_id: r.get(2)?,
                seen_at: r.get(3)?,
                excerpt: r.get(4)?,
                git_commit: r.get(5)?,
                git_branch: r.get(6)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

struct OccRow {
    id: i64,
    fingerprint_id: i64,
    run_id: i64,
    seen_at: String,
    excerpt: Option<String>,
    git_commit: Option<String>,
    git_branch: Option<String>,
}

fn path_to_string(p: &std::path::Path) -> String {
    p.to_string_lossy().into_owned()
}

fn run_to_dto(r: crate::repo::runs::RunRecord) -> RunExport {
    let argv: Vec<String> = serde_json::from_str(&r.argv_json).unwrap_or_default();
    RunExport {
        id: r.id,
        started_at: r.started_at,
        finished_at: r.finished_at,
        duration_ms: r.duration_ms,
        program: r.program,
        argv,
        cwd: r.cwd,
        exit_code: r.exit_code,
        status: r.status.as_str().to_string(),
        tags: r.tags,
        stdout_excerpt: r.stdout_excerpt,
        stderr_excerpt: r.stderr_excerpt,
        stdout_truncated: r.stdout_truncated,
        stderr_truncated: r.stderr_truncated,
        git_commit: r.git_commit,
        git_branch: r.git_branch,
        git_dirty: r.git_dirty,
    }
}

fn fp_to_dto(f: crate::repo::bugs::Fingerprint) -> FingerprintExport {
    FingerprintExport {
        id: f.id,
        hash: f.hash,
        canonical: f.canonical,
        summary: f.summary,
        first_seen_at: f.first_seen_at,
        last_seen_at: f.last_seen_at,
        occurrence_count: f.occurrence_count,
    }
}

fn occ_to_dto(o: OccRow) -> OccurrenceExport {
    OccurrenceExport {
        id: o.id,
        fingerprint_id: o.fingerprint_id,
        run_id: o.run_id,
        seen_at: o.seen_at,
        excerpt: o.excerpt,
        git_commit: o.git_commit,
        git_branch: o.git_branch,
    }
}

fn snap_to_dto(s: Snapshot) -> SnapshotExport {
    SnapshotExport {
        id: s.id,
        taken_at: s.taken_at,
        checker_name: s.checker_name,
        status: s.status,
        diagnostic: s.diagnostic,
        git_commit: s.git_commit,
        git_branch: s.git_branch,
    }
}

fn alert_to_dto(a: Alert) -> AlertExport {
    AlertExport {
        id: a.id,
        snapshot_id: a.snapshot_id,
        severity: a.severity,
        message: a.message,
        source: a.source,
        symbol: a.symbol,
    }
}

fn corr_to_dto(c: crate::repo::correlations::Correlation) -> CorrelationExport {
    CorrelationExport {
        id: c.id,
        fingerprint_id: c.fingerprint_id,
        alert_id: c.alert_id,
        score: c.score,
        label: c.label,
        created_at: c.created_at,
        score_message: c.score_message,
        score_symbol: c.score_symbol,
        score_file: c.score_file,
        score_tag: c.score_tag,
        algorithm_version: c.algorithm_version,
    }
}

fn link_to_dto(l: crate::repo::links::ManualLink) -> ManualLinkExport {
    ManualLinkExport {
        id: l.id,
        fingerprint_id: l.fingerprint_id,
        alert_id: l.alert_id,
        note: l.note,
        created_at: l.created_at,
    }
}
