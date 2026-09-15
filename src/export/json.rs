//! JSON serializer for `ExportDocument`.
//!
//! Pretty-printed for human inspection; the DTO is also a stable machine
//! contract (consumers should parse with their preferred JSON library).

use crate::error::Error;

use super::dto::ExportDocument;

pub fn to_string(doc: &ExportDocument) -> Result<String, Error> {
    Ok(serde_json::to_string_pretty(doc)?)
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
                root: "/tmp/proj".into(),
                state_dir: "/tmp/proj/.driftwatch".into(),
                config_path: "/tmp/proj/driftwatch.toml".into(),
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
            gate_runs: vec![],
        }
    }

    #[test]
    fn empty_document_serializes_with_schema_version() {
        let s = to_string(&empty_doc()).unwrap();
        let v: serde_json::Value = serde_json::from_str(&s).unwrap();
        // The current SCHEMA_VERSION is exported as the document's
        // schema_version field. A consumer that does not understand
        // it should reject the document.
        assert_eq!(v["schema_version"], SCHEMA_VERSION);
        assert!(v["runs"].as_array().unwrap().is_empty());
    }

    #[test]
    fn round_trip_preserves_dto() {
        let doc = ExportDocument {
            schema_version: SCHEMA_VERSION,
            exported_at: "2026-01-01T00:00:00Z".into(),
            project: ProjectExport {
                root: "/p".into(),
                state_dir: "/p/.driftwatch".into(),
                config_path: "/p/driftwatch.toml".into(),
                local_schema_version: 1,
            },
            runs: vec![RunExport {
                id: 1,
                started_at: "2026-01-01T00:00:00Z".into(),
                finished_at: Some("2026-01-01T00:00:01Z".into()),
                duration_ms: Some(1000),
                program: "cargo".into(),
                argv: vec!["test".into()],
                cwd: "/p".into(),
                exit_code: Some(0),
                status: "success".into(),
                tags: vec!["ci".into()],
                stdout_excerpt: Some("ok".into()),
                stderr_excerpt: None,
                stdout_truncated: false,
                stderr_truncated: false,
                git_commit: Some("abc123".into()),
                git_branch: Some("main".into()),
                git_dirty: Some(false),
            }],
            fingerprints: vec![],
            occurrences: vec![],
            snapshots: vec![],
            alerts: vec![],
            correlations: vec![CorrelationExport {
                id: 1,
                fingerprint_id: 1,
                alert_id: 1,
                score: 0.78,
                label: "heuristic correlation".into(),
                created_at: "2026-01-01T00:00:00Z".into(),
                score_message: Some(0.9),
                score_symbol: Some(1.0),
                score_file: Some(0.5),
                score_tag: None,
                algorithm_version: "v1".into(),
            }],
            manual_links: vec![],
            gate_artifacts: vec![],
            gate_runs: vec![],
        };
        let s = to_string(&doc).unwrap();
        let back: ExportDocument = serde_json::from_str(&s).unwrap();
        assert_eq!(back, doc);
    }

    #[test]
    fn correlation_export_round_trips_v2_fields() {
        // The v2 fields are present in the JSON output and survive
        // a deserialize → serialize round-trip.
        let doc = ExportDocument {
            schema_version: 2,
            exported_at: "2026-01-01T00:00:00Z".into(),
            project: ProjectExport {
                root: "/p".into(),
                state_dir: "/p/.driftwatch".into(),
                config_path: "/p/driftwatch.toml".into(),
                local_schema_version: 3,
            },
            runs: vec![],
            fingerprints: vec![],
            occurrences: vec![],
            snapshots: vec![],
            alerts: vec![],
            correlations: vec![CorrelationExport {
                id: 7,
                fingerprint_id: 11,
                alert_id: 13,
                score: 0.84,
                label: "heuristic correlation (strong)".into(),
                created_at: "2026-01-01T00:00:00Z".into(),
                score_message: Some(1.0),
                score_symbol: Some(0.5),
                score_file: Some(1.0),
                score_tag: Some(0.0),
                algorithm_version: "v1".into(),
            }],
            manual_links: vec![],
            gate_artifacts: vec![],
            gate_runs: vec![],
        };
        let s = to_string(&doc).unwrap();
        let v: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert_eq!(v["schema_version"], 2);
        let corr = &v["correlations"][0];
        assert_eq!(corr["score_message"], 1.0);
        assert_eq!(corr["score_symbol"], 0.5);
        assert_eq!(corr["score_file"], 1.0);
        assert_eq!(corr["score_tag"], 0.0);
        assert_eq!(corr["algorithm_version"], "v1");
    }
}
