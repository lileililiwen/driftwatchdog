//! Library-level error type. The CLI binary wraps these with `anyhow::Context`
//! at the top-level boundary; tests can match on these variants directly.

use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("io error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("io error: {0}")]
    IoBare(#[from] std::io::Error),

    #[error("config parse error in {path}: {source}")]
    ConfigParse {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },

    #[error("invalid config in {path}: {message}")]
    ConfigInvalid { path: PathBuf, message: String },

    #[error("config serialize error: {0}")]
    ConfigSerialize(#[from] toml::ser::Error),

    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("project root not found: walked up from {start}")]
    ProjectRootNotFound { start: PathBuf },

    #[error("migration failed at version {version}: {message}")]
    Migration { version: i64, message: String },

    #[error("no fingerprint matches '{id}'")]
    BugNotFound { id: String },

    #[error("ambiguous bug prefix '{prefix}': {count} fingerprints match ({candidates}); use a longer prefix (try 'driftwatch list' to see candidates)")]
    BugAmbiguous {
        prefix: String,
        count: usize,
        candidates: String,
    },

    #[error("already linked (id {id})")]
    DuplicateLink { id: i64 },

    #[error("could not resolve {side} reference '{raw}'")]
    LinkTarget { side: &'static str, raw: String },

    #[error("manual link #{id} not found")]
    ManualLinkNotFound { id: i64 },
}

impl Error {
    pub fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }

    /// Single-wrap remediation hint rendered by the binary as
    /// `hint: <text>`. Every user-facing error carries one so terse
    /// messages always point at a next step.
    pub fn hint(&self) -> Option<String> {
        match self {
            Error::Io { path, .. } => Some(format!(
                "check permissions on `{}`",
                path.display()
            )),
            Error::IoBare(_) => None,
            Error::ConfigParse { path, .. } | Error::ConfigInvalid { path, .. } => {
                Some(format!(
                    "edit `{}` to fix the error, or compare with `driftwatch.toml.example`",
                    path.display()
                ))
            }
            Error::ConfigSerialize(_) => {
                Some("check for non-UTF8 or unserializable config values".into())
            }
            Error::Json(_) => Some("verify the checker emits a single JSON document".into()),
            Error::Sqlite(_) => Some(
                "run `driftwatch doctor`; if the DB is corrupt, delete `.driftwatch/state.db` and run `driftwatch init` again".into(),
            ),
            Error::ProjectRootNotFound { start } => Some(format!(
                "run `driftwatch init` under `{}` to create a project",
                start.display()
            )),
            Error::Migration { version, .. } => Some(format!(
                "migration {version} failed; back up `.driftwatch/state.db` and re-run"
            )),
            Error::BugNotFound { .. } => Some(
                "try 'driftwatch list' or 'driftwatch top' to find the bug (prefix needs 8+ hex chars)".into(),
            ),
            Error::BugAmbiguous { .. } => Some(
                "use a longer hash prefix from 'driftwatch top'".into(),
            ),
            Error::DuplicateLink { .. } => {
                Some("the pair is already linked; use 'driftwatch unlink <id>' to remove it first".into())
            }
            Error::LinkTarget { side, .. } => Some(match *side {
                "bug" => "use 'bug:<hash8>', a bare hash prefix, or 'id:<n>'",
                _ => "use 'spec:<id>' with a numeric drift_alerts.id",
            }
            .into()),
            Error::ManualLinkNotFound { .. } => {
                Some("list links via `driftwatch report --ai` to find the id".into())
            }
        }
    }
}
