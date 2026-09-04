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
        }
    }

    #[test]
    fn empty_document_serializes_with_schema_version() {
        let s = to_string(&empty_doc()).unwrap();
        let v: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert_eq!(v["schema_version"], SCHEMA_VERSION);
        assert_eq!(v["project"]["local_schema_version"], 1);
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
            correlations: vec![],
            manual_links: vec![],
        };
        let s = to_string(&doc).unwrap();
        let back: ExportDocument = serde_json::from_str(&s).unwrap();
        assert_eq!(back, doc);
    }
}
