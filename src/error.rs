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
}

impl Error {
    pub fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }
}
