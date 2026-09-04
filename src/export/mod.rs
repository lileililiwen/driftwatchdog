//! Portable data export.
//!
//! Public entry point is [`format`], which selects a serializer and
//! renders the document. The DTO layer ([`dto`]) is the stable
//! contract; the build layer ([`build`]) is the only place that knows
//! how to assemble it from the local database.

pub mod build;
pub mod dto;
pub mod json;
pub mod jsonl;
pub mod markdown;

use crate::error::Error;
use crate::project::ProjectRoot;
use crate::repo::Db;

pub use build::build;
pub use dto::{
    AlertExport, CorrelationExport, ExportDocument, FingerprintExport, ManualLinkExport,
    OccurrenceExport, ProjectExport, RecordKind, RunExport, SnapshotExport, SCHEMA_VERSION,
};

/// Output format requested by the CLI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Json,
    Jsonl,
    Markdown,
}

impl Format {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "json" => Some(Format::Json),
            "jsonl" => Some(Format::Jsonl),
            "markdown" => Some(Format::Markdown),
            _ => None,
        }
    }
}

/// Build the export document and render it in the requested format.
pub fn format(db: &mut Db, proj: &ProjectRoot, fmt: Format) -> Result<String, Error> {
    let doc = build(db, proj)?;
    match fmt {
        Format::Json => json::to_string(&doc),
        Format::Jsonl => jsonl::to_string(&doc),
        Format::Markdown => markdown::to_string(&doc),
    }
}
