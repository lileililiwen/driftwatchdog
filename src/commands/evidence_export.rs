//! `driftwatch gate evidence-export` — read-only export of a completed
//! Gate run in Workspace Governance's `release_evidence` vocabulary.
//!
//! The export is a pure read: it never mutates the run, the project,
//! or any registry, and it never invokes an LLM, signer, SBOM
//! generator, or publisher. The wire shape is versioned
//! (`schema_version: 1`) so a future change can be rejected at the
//! boundary.

use std::path::Path;

use crate::cli::{GateEvidenceExportArgs, GateFormatArg};
use crate::error::Error;
use crate::gate::dto::parse_gate_result_document;
use crate::gate::evidence_export::{
    build as build_evidence_export, render_human as render_export_human,
    render_json as render_export_json, ExportError,
};
use crate::project::{git, ProjectRoot};
use crate::repo::gates::Gates;

const EXIT_OK: i32 = 0;
const EXIT_NO_RUN: i32 = 1;

/// `driftwatch gate evidence-export [--format human|json] [--dry-run]`.
///
/// Reads the latest completed `gate_runs` row, parses its per-check
/// results, and prints the versioned export. Refuses with a
/// non-zero exit when no run exists.
pub fn evidence_export(args: GateEvidenceExportArgs, cwd: &Path) -> Result<i32, Error> {
    let proj = ProjectRoot::discover(cwd)?;
    let db = crate::repo::Db::open_read_only(&proj.db_path)?;
    let gates = Gates::new(&db);
    let run = gates.latest()?;
    let results = match run.as_ref() {
        Some(row) => parse_results_for_row(row)?,
        None => Vec::new(),
    };
    let project_id = project_id_for(&proj);
    let git_ctx = git::capture(&proj.root);
    let project_revision = git_ctx
        .commit
        .clone()
        .unwrap_or_else(|| run.as_ref().map(|r| r.revision.clone()).unwrap_or_default());
    let driftwatchdog_version = env!("CARGO_PKG_VERSION");

    match build_evidence_export(
        run.as_ref(),
        &results,
        &project_id,
        &project_revision,
        driftwatchdog_version,
    ) {
        Ok(doc) => {
            let text = match args.format {
                GateFormatArg::Json => render_export_json(&doc),
                GateFormatArg::Human => render_export_human(&doc),
            };
            println!("{text}");
            // `--dry-run` is a documentation flag: the export is a
            // pure read so the output is identical either way. We
            // accept the flag and continue.
            let _ = args.dry_run;
            Ok(EXIT_OK)
        }
        Err(ExportError::NoRun(project)) => {
            eprintln!(
                "driftwatch gate evidence-export: no completed gate run for project `{project}`; \
                 run `driftwatch gate` first."
            );
            Ok(EXIT_NO_RUN)
        }
        Err(ExportError::UnknownField(name)) => Err(Error::ConfigInvalid {
            path: proj.root.join("gate.toml"),
            message: format!(
                "unknown governance field `{name}` requested by export construction; \
                 this is a Rust error, not a configuration error"
            ),
        }),
    }
}

/// Parse the canonical per-check results JSON for a gate run. The
/// gate command persists the result list as a bare JSON array
/// (`[{GateResult}, ...]`); a missing or malformed document is
/// treated as no results so the export still runs (every supplied
/// field becomes `unverified`).
fn parse_results_for_row(
    row: &crate::repo::gates::GateRunRow,
) -> Result<Vec<crate::gate::types::GateResult>, Error> {
    if row.results_json.is_empty() {
        return Ok(Vec::new());
    }
    match serde_json::from_str::<Vec<crate::gate::types::GateResult>>(&row.results_json) {
        Ok(results) => Ok(results),
        // Fallback: the legacy versioned document shape.
        Err(_) => match parse_gate_result_document(row.results_json.as_bytes()) {
            Ok(results) => Ok(results),
            Err(_) => Ok(Vec::new()),
        },
    }
}

/// Resolve a stable project identity for the export. We use the
/// directory name (basename of the project root) so the export can be
/// produced without extending the manifest schema. Workspace
/// Governance correlates this with its own project record; a
/// collision across projects is the audit's job to detect.
fn project_id_for(proj: &ProjectRoot) -> String {
    proj.root
        .file_name()
        .and_then(|n| n.to_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| "project".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::GateFormatArg;
    use crate::repo::gates::{GateRunRow, NewGateRun};
    use crate::repo::Db;
    use tempfile::tempdir;

    fn seed_gate_run(dir: &Path, results_json: &str) {
        std::fs::create_dir_all(dir.join(".driftwatch")).unwrap();
        let mut db = Db::open(&dir.join(".driftwatch/state.db")).unwrap();
        crate::storage::migrations::apply(db.conn_mut()).unwrap();
        let new = NewGateRun {
            taken_at: "2026-09-26T00:00:00Z",
            change_id: "abc123",
            revision: "abc123",
            manifest_digest: "sha256:deadbeef",
            rule_pack_version: "release@0.1.0",
            status: "PASS",
            blocked: false,
            results_json,
        };
        Gates::insert(&mut db, &new).unwrap();
    }

    fn args(format: GateFormatArg) -> GateEvidenceExportArgs {
        GateEvidenceExportArgs {
            format,
            dry_run: false,
        }
    }

    #[test]
    fn refuses_when_no_run_exists() {
        let tmp = tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join(".driftwatch")).unwrap();
        let mut db = Db::open(&tmp.path().join(".driftwatch/state.db")).unwrap();
        crate::storage::migrations::apply(db.conn_mut()).unwrap();
        drop(db);
        let code = evidence_export(args(GateFormatArg::Human), tmp.path()).unwrap();
        assert_eq!(code, EXIT_NO_RUN);
    }

    #[test]
    fn prints_human_export_for_passing_run() {
        let tmp = tempdir().unwrap();
        let results_json = r#"{"version":1,"results":[{"gate_id":"release-evidence","source":"test","status":"PASS","severity":"info","findings":[],"evidence":[],"missing_evidence":[]}]}"#;
        seed_gate_run(tmp.path(), results_json);
        let code = evidence_export(args(GateFormatArg::Human), tmp.path()).unwrap();
        assert_eq!(code, EXIT_OK);
    }

    #[test]
    fn prints_json_export_for_passing_run() {
        let tmp = tempdir().unwrap();
        let results_json = r#"{"version":1,"results":[{"gate_id":"release-evidence","source":"test","status":"PASS","severity":"info","findings":[],"evidence":[],"missing_evidence":[]}]}"#;
        seed_gate_run(tmp.path(), results_json);
        let code = evidence_export(args(GateFormatArg::Json), tmp.path()).unwrap();
        assert_eq!(code, EXIT_OK);
    }

    #[test]
    fn refuses_malformed_results_json_as_empty_results() {
        // A malformed results_json is recoverable: the export still
        // runs and every field is emitted `unverified`.
        let tmp = tempdir().unwrap();
        seed_gate_run(tmp.path(), "not-json");
        let code = evidence_export(args(GateFormatArg::Json), tmp.path()).unwrap();
        assert_eq!(code, EXIT_OK);
    }

    #[test]
    fn evidence_export_does_not_mutate_state() {
        let tmp = tempdir().unwrap();
        let results_json = r#"{"version":1,"results":[]}"#;
        seed_gate_run(tmp.path(), results_json);
        let before =
            Gates::new(&Db::open_read_only(&tmp.path().join(".driftwatch/state.db")).unwrap())
                .count()
                .unwrap();
        for _ in 0..3 {
            let _ = evidence_export(args(GateFormatArg::Json), tmp.path()).unwrap();
        }
        let after =
            Gates::new(&Db::open_read_only(&tmp.path().join(".driftwatch/state.db")).unwrap())
                .count()
                .unwrap();
        assert_eq!(before, after);
    }

    #[test]
    fn project_id_prefers_directory_name() {
        let tmp = tempdir().unwrap();
        let dir = tmp.path().join("my-cool-project");
        std::fs::create_dir_all(&dir).unwrap();
        let proj = ProjectRoot::at(&dir);
        assert_eq!(project_id_for(&proj), "my-cool-project");
    }

    #[test]
    fn parse_results_for_row_returns_results_on_canonical_doc() {
        let row = GateRunRow {
            id: 1,
            taken_at: "t".into(),
            change_id: "c".into(),
            revision: "r".into(),
            manifest_digest: "m".into(),
            rule_pack_version: "p".into(),
            status: "PASS".into(),
            blocked: false,
            results_json: r#"{"version":1,"results":[{"gate_id":"smoke","source":"x","status":"PASS","severity":"info","findings":[],"evidence":[],"missing_evidence":[]}]}"#.into(),
        };
        let results = parse_results_for_row(&row).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].gate_id, "smoke");
    }

    #[test]
    fn parse_results_for_row_recovers_from_garbage() {
        let row = GateRunRow {
            id: 1,
            taken_at: "t".into(),
            change_id: "c".into(),
            revision: "r".into(),
            manifest_digest: "m".into(),
            rule_pack_version: "p".into(),
            status: "PASS".into(),
            blocked: false,
            results_json: "not-json".into(),
        };
        let results = parse_results_for_row(&row).unwrap();
        assert!(results.is_empty());
    }
}
