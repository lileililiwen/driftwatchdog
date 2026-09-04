//! Export DTO layer.
//!
//! These types are intentionally separate from the SQLite row types in
//! `crate::repo::*`. They are the stable, versioned contract for
//! `driftwatch export json|jsonl|markdown` and the future MCP
//! surface. Changes to repository types should not change these DTOs
//! silently; bump `SCHEMA_VERSION` when the on-disk shape changes.

use serde::{Deserialize, Serialize};

/// Bumped when the on-disk export shape changes. Consumers should
/// reject documents whose `schema_version` they do not understand.
pub const SCHEMA_VERSION: u32 = 1;

/// Top-level JSON document. The `runs`, `fingerprints`, etc. fields are
/// flat arrays so consumers can index them directly. `project` carries
/// the resolved root path and the schema version of the local SQLite
/// database (separate from the export schema version).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExportDocument {
    pub schema_version: u32,
    pub exported_at: String,
    pub project: ProjectExport,
    pub runs: Vec<RunExport>,
    pub fingerprints: Vec<FingerprintExport>,
    pub occurrences: Vec<OccurrenceExport>,
    pub snapshots: Vec<SnapshotExport>,
    pub alerts: Vec<AlertExport>,
    pub correlations: Vec<CorrelationExport>,
    pub manual_links: Vec<ManualLinkExport>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProjectExport {
    /// Resolved Driftwatch project root (canonical path).
    pub root: String,
    /// Filesystem path of the state directory (`.driftwatch`).
    pub state_dir: String,
    /// Filesystem path of the configuration file (`driftwatch.toml`).
    pub config_path: String,
    /// Local SQLite schema version observed at export time.
    pub local_schema_version: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RunExport {
    pub id: i64,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub duration_ms: Option<i64>,
    pub program: String,
    pub argv: Vec<String>,
    pub cwd: String,
    pub exit_code: Option<i32>,
    pub status: String,
    pub tags: Vec<String>,
    pub stdout_excerpt: Option<String>,
    pub stderr_excerpt: Option<String>,
    pub stdout_truncated: bool,
    pub stderr_truncated: bool,
    pub git_commit: Option<String>,
    pub git_branch: Option<String>,
    pub git_dirty: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FingerprintExport {
    pub id: i64,
    pub hash: String,
    pub canonical: String,
    pub summary: Option<String>,
    pub first_seen_at: String,
    pub last_seen_at: String,
    pub occurrence_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OccurrenceExport {
    pub id: i64,
    pub fingerprint_id: i64,
    pub run_id: i64,
    pub seen_at: String,
    pub excerpt: Option<String>,
    pub git_commit: Option<String>,
    pub git_branch: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SnapshotExport {
    pub id: i64,
    pub taken_at: String,
    pub checker_name: String,
    pub status: String,
    pub diagnostic: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AlertExport {
    pub id: i64,
    pub snapshot_id: i64,
    pub severity: String,
    pub message: String,
    pub source: Option<String>,
    pub symbol: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CorrelationExport {
    pub id: i64,
    pub fingerprint_id: i64,
    pub alert_id: i64,
    pub score: f64,
    pub label: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ManualLinkExport {
    pub id: i64,
    pub fingerprint_id: Option<i64>,
    pub alert_id: Option<i64>,
    pub note: Option<String>,
    pub created_at: String,
}

/// Discriminator used by the JSONL serializer. One record per line, each
/// self-contained, with `type` selecting the DTO and `id` providing a
/// stable identifier per record kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordKind {
    Project,
    Run,
    Fingerprint,
    Occurrence,
    Snapshot,
    Alert,
    Correlation,
    ManualLink,
}

impl RecordKind {
    /// Stable string discriminator used in JSONL output and JSON
    /// document grouping. The mapping is part of the public contract.
    pub fn as_str(self) -> &'static str {
        match self {
            RecordKind::Project => "project",
            RecordKind::Run => "run",
            RecordKind::Fingerprint => "fingerprint",
            RecordKind::Occurrence => "occurrence",
            RecordKind::Snapshot => "snapshot",
            RecordKind::Alert => "alert",
            RecordKind::Correlation => "correlation",
            RecordKind::ManualLink => "manual_link",
        }
    }
}
