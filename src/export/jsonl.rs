//! JSONL serializer for `ExportDocument`.
//!
//! One record per line. Each line is a self-contained JSON object with
//! a `type` discriminator and a stable `id` so consumers can stream and
//! route records without buffering the whole document.

use serde::Serialize;
#[cfg(test)]
use serde_json::Value as JsonValue;

use crate::error::Error;

use super::dto::ExportDocument;

/// Wrapper struct that adds a `type` discriminator and a stable
/// `record_id` to each record. We avoid the bare key `id` because
/// many payload DTOs (e.g. `RunExport`) already carry an `id` field
/// and a flat `id` collision would mask the discriminator. Used only
/// by the JSONL serializer.
#[derive(Debug, Serialize)]
struct Tagged<'a, T: Serialize> {
    #[serde(rename = "type")]
    kind: &'a str,
    record_id: String,
    #[serde(flatten)]
    payload: T,
}

pub fn to_string(doc: &ExportDocument) -> Result<String, Error> {
    let mut out = String::new();

    // Project has a synthetic id; we use the root path.
    push(
        &mut out,
        &Tagged {
            kind: "project",
            record_id: format!("project:{}", doc.project.root),
            payload: &doc.project,
        },
    )?;

    for r in &doc.runs {
        push(
            &mut out,
            &Tagged {
                kind: "run",
                record_id: format!("run:{}", r.id),
                payload: r,
            },
        )?;
    }
    for f in &doc.fingerprints {
        push(
            &mut out,
            &Tagged {
                kind: "fingerprint",
                record_id: format!("fingerprint:{}", f.hash),
                payload: f,
            },
        )?;
    }
    for o in &doc.occurrences {
        push(
            &mut out,
            &Tagged {
                kind: "occurrence",
                record_id: format!("occurrence:{}", o.id),
                payload: o,
            },
        )?;
    }
    for s in &doc.snapshots {
        push(
            &mut out,
            &Tagged {
                kind: "snapshot",
                record_id: format!("snapshot:{}", s.id),
                payload: s,
            },
        )?;
    }
    for a in &doc.alerts {
        push(
            &mut out,
            &Tagged {
                kind: "alert",
                record_id: format!("alert:{}", a.id),
                payload: a,
            },
        )?;
    }
    for c in &doc.correlations {
        push(
            &mut out,
            &Tagged {
                kind: "correlation",
                record_id: format!("correlation:{}", c.id),
                payload: c,
            },
        )?;
    }
    for l in &doc.manual_links {
        push(
            &mut out,
            &Tagged {
                kind: "manual_link",
                record_id: format!("manual_link:{}", l.id),
                payload: l,
            },
        )?;
    }
    for g in &doc.gate_artifacts {
        push(
            &mut out,
            &Tagged {
                kind: "gate_artifact",
                record_id: format!("gate_artifact:{}", g.key),
                payload: g,
            },
        )?;
    }
    for g in &doc.gate_runs {
        push(
            &mut out,
            &Tagged {
                kind: "gate_run",
                record_id: format!("gate_run:{}", g.id),
                payload: g,
            },
        )?;
    }
    Ok(out)
}

fn push<T: Serialize>(out: &mut String, value: &T) -> Result<(), Error> {
    let line = serde_json::to_string(value)?;
    out.push_str(&line);
    out.push('\n');
    Ok(())
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
    fn empty_doc_produces_one_project_line() {
        let s = to_string(&empty_doc()).unwrap();
        let lines: Vec<&str> = s.lines().collect();
        assert_eq!(lines.len(), 1);
        let v: JsonValue = serde_json::from_str(lines[0]).unwrap();
        assert_eq!(v["type"], "project");
        assert!(v["record_id"].as_str().unwrap().starts_with("project:"));
    }

    #[test]
    fn each_record_has_type_and_id() {
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
                id: 7,
                started_at: "2026-01-01T00:00:00Z".into(),
                finished_at: None,
                duration_ms: None,
                program: "echo".into(),
                argv: vec![],
                cwd: "/p".into(),
                exit_code: Some(0),
                status: "success".into(),
                tags: vec![],
                stdout_excerpt: None,
                stderr_excerpt: None,
                stdout_truncated: false,
                stderr_truncated: false,
                git_commit: None,
                git_branch: None,
                git_dirty: None,
            }],
            fingerprints: vec![FingerprintExport {
                id: 1,
                hash: "deadbeef".into(),
                canonical: "boom".into(),
                summary: Some("boom".into()),
                first_seen_at: "2026-01-01T00:00:00Z".into(),
                last_seen_at: "2026-01-01T00:00:00Z".into(),
                occurrence_count: 1,
            }],
            occurrences: vec![],
            snapshots: vec![],
            alerts: vec![],
            correlations: vec![],
            manual_links: vec![],
            gate_artifacts: vec![],
            gate_runs: vec![],
        };
        let s = to_string(&doc).unwrap();
        let lines: Vec<&str> = s.lines().collect();
        // 1 project + 1 run + 1 fingerprint = 3 lines
        assert_eq!(lines.len(), 3);
        let parsed: Vec<JsonValue> = lines
            .iter()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();
        let kinds: Vec<&str> = parsed.iter().map(|v| v["type"].as_str().unwrap()).collect();
        assert!(kinds.contains(&"project"));
        assert!(kinds.contains(&"run"));
        assert!(kinds.contains(&"fingerprint"));
        for v in &parsed {
            assert!(v["record_id"].is_string(), "missing record_id on {v}");
        }
    }

    #[test]
    fn every_line_parses_independently() {
        let doc = empty_doc();
        let s = to_string(&doc).unwrap();
        for line in s.lines() {
            let v: JsonValue = serde_json::from_str(line)
                .unwrap_or_else(|e| panic!("line {line:?} did not parse: {e}"));
            assert!(v["type"].is_string());
            assert!(v["record_id"].is_string());
        }
    }
}
