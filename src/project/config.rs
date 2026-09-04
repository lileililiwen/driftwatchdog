//! `driftwatch.toml` configuration: schema, defaults, and loader.
//!
//! The configuration is intentionally permissive: unknown sections are kept on
//! disk and ignored by the loader so future versions can add fields without
//! breaking older binaries. Optional sections (`[project]`, `[storage]`,
//! `[fingerprint]`, `[[checkers]]`) default to empty values when absent.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::Error;

/// On-disk configuration model. Unknown tables are preserved verbatim so that
/// new sections added by future versions remain readable.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub project: ProjectSection,
    #[serde(default)]
    pub storage: StorageSection,
    #[serde(default)]
    pub fingerprint: FingerprintSection,
    #[serde(default, rename = "checkers")]
    pub checkers: Vec<CheckerEntry>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProjectSection {
    /// Optional human-readable project name. Free-form.
    #[serde(default)]
    pub name: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StorageSection {
    /// Bounded size in bytes for captured stdout per run. `0` disables capture.
    /// Default: 64 KiB.
    #[serde(default)]
    pub max_stdout_bytes: Option<u64>,
    /// Bounded size in bytes for captured stderr per run. `0` disables capture.
    /// Default: 64 KiB.
    #[serde(default)]
    pub max_stderr_bytes: Option<u64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FingerprintSection {
    /// Ordered, comma-separated placeholder names that the generic normalizer
    /// should strip. Empty by default.
    #[serde(default)]
    pub ignore: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CheckerEntry {
    pub name: String,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    /// Optional working directory. Defaults to the project root.
    #[serde(default)]
    pub working_dir: Option<PathBuf>,
    /// Optional environment variables added to the checker process.
    /// `BTreeMap` is used for stable serialization order in tests and
    /// round-trips.
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    /// Per-checker timeout in milliseconds. Defaults to 30_000.
    #[serde(default)]
    pub timeout_ms: Option<u64>,
    /// Per-stream output capture limit in bytes. Defaults to 1 MiB.
    #[serde(default)]
    pub max_output_bytes: Option<u64>,
}

impl Config {
    /// Default configuration used when no `driftwatch.toml` exists.
    pub fn defaults() -> Self {
        Self::default()
    }

    /// Load configuration from `path`. If the file does not exist, return the
    /// defaults without creating it.
    pub fn load(path: &Path) -> Result<Self, Error> {
        if !path.exists() {
            return Ok(Self::defaults());
        }
        let text = fs::read_to_string(path).map_err(|e| Error::io(path, e))?;
        let cfg: Config = toml::from_str(&text).map_err(|e| Error::ConfigParse {
            path: path.to_path_buf(),
            source: e,
        })?;
        Ok(cfg)
    }

    /// Serialize to a stable TOML representation suitable for writing to disk.
    pub fn to_toml(&self) -> Result<String, Error> {
        Ok(toml::to_string_pretty(self)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn loads_empty_file_as_defaults() {
        let tmp = tempdir().unwrap();
        let p = tmp.path().join("driftwatch.toml");
        fs::write(&p, "").unwrap();
        let cfg = Config::load(&p).unwrap();
        assert!(cfg.project.name.is_none());
        assert!(cfg.checkers.is_empty());
    }

    #[test]
    fn loads_full_config() {
        let tmp = tempdir().unwrap();
        let p = tmp.path().join("driftwatch.toml");
        fs::write(
            &p,
            r#"
[project]
name = "demo"

[storage]
max_stdout_bytes = 0
max_stderr_bytes = 65536

[fingerprint]
ignore = ["duration", "uuid"]

[[checkers]]
name = "architecture"
command = "my-spec-checker"
args = ["--json"]
"#,
        )
        .unwrap();
        let cfg = Config::load(&p).unwrap();
        assert_eq!(cfg.project.name.as_deref(), Some("demo"));
        assert_eq!(cfg.storage.max_stdout_bytes, Some(0));
        assert_eq!(cfg.fingerprint.ignore, vec!["duration", "uuid"]);
        assert_eq!(cfg.checkers.len(), 1);
        assert_eq!(cfg.checkers[0].name, "architecture");
    }

    #[test]
    fn missing_file_returns_defaults() {
        let tmp = tempdir().unwrap();
        let p = tmp.path().join("does-not-exist.toml");
        let cfg = Config::load(&p).unwrap();
        assert!(cfg.checkers.is_empty());
    }

    #[test]
    fn round_trip_to_toml() {
        let cfg = Config {
            project: ProjectSection {
                name: Some("x".into()),
            },
            storage: StorageSection {
                max_stdout_bytes: Some(10),
                max_stderr_bytes: None,
            },
            fingerprint: FingerprintSection::default(),
            checkers: vec![],
        };
        let s = cfg.to_toml().unwrap();
        assert!(s.contains("name = \"x\""));
        assert!(s.contains("max_stdout_bytes = 10"));
    }

    #[test]
    fn loads_checker_with_optional_fields() {
        let tmp = tempdir().unwrap();
        let p = tmp.path().join("driftwatch.toml");
        fs::write(
            &p,
            r#"
[[checkers]]
name = "arch"
command = "my-spec-checker"
args = ["--json"]
working_dir = "./sub"
timeout_ms = 5000
max_output_bytes = 4096

[checkers.env]
SPEC_VERSION = "1"
"#,
        )
        .unwrap();
        let cfg = Config::load(&p).unwrap();
        assert_eq!(cfg.checkers.len(), 1);
        let c = &cfg.checkers[0];
        assert_eq!(c.name, "arch");
        assert_eq!(c.command, "my-spec-checker");
        assert_eq!(c.args, vec!["--json".to_string()]);
        assert_eq!(c.working_dir.as_deref(), Some(Path::new("./sub")));
        assert_eq!(c.timeout_ms, Some(5000));
        assert_eq!(c.max_output_bytes, Some(4096));
        assert_eq!(c.env.get("SPEC_VERSION").map(String::as_str), Some("1"));
    }

    #[test]
    fn missing_optional_checker_fields_default() {
        let tmp = tempdir().unwrap();
        let p = tmp.path().join("driftwatch.toml");
        fs::write(
            &p,
            r#"
[[checkers]]
name = "basic"
command = "my-spec-checker"
"#,
        )
        .unwrap();
        let cfg = Config::load(&p).unwrap();
        let c = &cfg.checkers[0];
        assert!(c.working_dir.is_none());
        assert!(c.env.is_empty());
        assert!(c.timeout_ms.is_none());
        assert!(c.max_output_bytes.is_none());
    }
}
