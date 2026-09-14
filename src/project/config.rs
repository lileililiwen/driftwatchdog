//! `driftwatch.toml` configuration: schema, defaults, and loader.
//!
//! The configuration is strict: unknown fields are rejected with a
//! `did-you-mean` hint so typos fail fast, and checker entries are
//! validated (unique names, non-empty commands, positive timeouts).
//! `working_dir` entries are jailed to the project root (see
//! [`Config::validate_working_dirs`]).

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::Error;

/// On-disk configuration model. Unknown fields are rejected so typos
/// surface immediately instead of silently changing behavior.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
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
#[serde(deny_unknown_fields)]
pub struct ProjectSection {
    /// Optional human-readable project name. Free-form.
    #[serde(default)]
    pub name: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
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
#[serde(deny_unknown_fields)]
pub struct FingerprintSection {
    /// Ordered, comma-separated placeholder names that the generic normalizer
    /// should strip. Empty by default.
    #[serde(default)]
    pub ignore: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CheckerEntry {
    pub name: String,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    /// Optional working directory. Relative paths resolve against the
    /// project root (not the process cwd); absolute paths must stay
    /// inside the project root. Defaults to the project root.
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

/// Known top-level + checker field names used for `did-you-mean` hints.
const KNOWN_FIELDS: &[&str] = &[
    "project",
    "storage",
    "fingerprint",
    "checkers",
    "name",
    "command",
    "args",
    "working_dir",
    "env",
    "timeout_ms",
    "max_output_bytes",
    "max_stdout_bytes",
    "max_stderr_bytes",
    "ignore",
];

fn edit_distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for (i, &ca) in a.iter().enumerate() {
        let mut cur = vec![i + 1];
        for (j, &cb) in b.iter().enumerate() {
            let cost = usize::from(ca != cb);
            cur.push((prev[j] + cost).min(cur[j] + 1).min(prev[j + 1] + 1));
        }
        prev = cur;
    }
    prev[b.len()]
}

fn suggest_field(unknown: &str) -> Option<&'static str> {
    let mut best: Option<(&'static str, usize)> = None;
    for &known in KNOWN_FIELDS {
        let d = edit_distance(unknown, known);
        if d <= 3 && best.is_none_or(|(_, bd)| d < bd) {
            best = Some((known, d));
        }
    }
    best.map(|(s, _)| s)
}

fn extract_unknown_field(msg: &str) -> Option<String> {
    // toml/serde unknown-field errors look like:
    //   "unknown field `commad`, expected ..." or 'unknown field "commad"'
    for pat in ["unknown field `", "unknown field \"", "unknown field '"] {
        if let Some(start) = msg.find(pat) {
            let rest = &msg[start + pat.len()..];
            let end = rest.find(['`', '"', '\'']).unwrap_or(rest.len());
            let field: String = rest[..end].chars().take(64).collect();
            if !field.is_empty() {
                return Some(field);
            }
        }
    }
    None
}

fn enrich_toml_error(path: &Path, e: toml::de::Error) -> Error {
    let msg = e.to_string();
    if msg.contains("unknown field") {
        if let Some(field) = extract_unknown_field(&msg) {
            let mut full = format!("unknown field \"{field}\"");
            if let Some(s) = suggest_field(&field) {
                full.push_str(&format!(", did you mean \"{s}\"?"));
            }
            return Error::ConfigInvalid {
                path: path.to_path_buf(),
                message: full,
            };
        }
    }
    Error::ConfigParse {
        path: path.to_path_buf(),
        source: e,
    }
}

/// Best-effort canonicalization: canonicalize when the path exists,
/// otherwise normalize `.`/`..` lexically so escape checks still work.
fn canonicalize_lossy(p: &Path) -> PathBuf {
    if let Ok(c) = std::fs::canonicalize(p) {
        return c;
    }
    let mut out = PathBuf::new();
    for comp in p.components() {
        use std::path::Component;
        match comp {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            c => out.push(c.as_os_str()),
        }
    }
    if out.as_os_str().is_empty() {
        PathBuf::from(".")
    } else {
        out
    }
}

impl Config {
    /// Default configuration used when no `driftwatch.toml` exists.
    pub fn defaults() -> Self {
        Self::default()
    }

    /// Load configuration from `path`. If the file does not exist, return the
    /// defaults without creating it. Structural validation (duplicate
    /// names, empty commands, zero timeouts) runs on every load so
    /// `driftwatch check` and `doctor` fail fast with an actionable error.
    pub fn load(path: &Path) -> Result<Self, Error> {
        if !path.exists() {
            return Ok(Self::defaults());
        }
        let text = fs::read_to_string(path).map_err(|e| Error::io(path, e))?;
        let cfg: Config = toml::from_str(&text).map_err(|e| enrich_toml_error(path, e))?;
        cfg.validate(path)?;
        Ok(cfg)
    }

    /// Structural validation independent of the project root.
    pub fn validate(&self, path: &Path) -> Result<(), Error> {
        let mut seen = BTreeSet::new();
        for entry in &self.checkers {
            if !seen.insert(entry.name.clone()) {
                return Err(Error::ConfigInvalid {
                    path: path.to_path_buf(),
                    message: format!(
                        "duplicate checker name \"{}\" (checker names must be unique)",
                        entry.name
                    ),
                });
            }
            if entry.name.trim().is_empty() {
                return Err(Error::ConfigInvalid {
                    path: path.to_path_buf(),
                    message: "checker with an empty name (set `name = \"...\"`)".into(),
                });
            }
            if entry.command.trim().is_empty() {
                return Err(Error::ConfigInvalid {
                    path: path.to_path_buf(),
                    message: format!(
                        "checker \"{}\" has an empty command (set `command = \"...\"`)",
                        entry.name
                    ),
                });
            }
            if entry.timeout_ms == Some(0) {
                return Err(Error::ConfigInvalid {
                    path: path.to_path_buf(),
                    message: format!(
                        "checker \"{}\" sets `timeout_ms = 0` (use a positive value or omit the key)",
                        entry.name
                    ),
                });
            }
        }
        Ok(())
    }

    /// Jail `working_dir` entries to the canonical project root. Relative
    /// paths resolve against `project_root` regardless of the process cwd;
    /// absolute paths (or relative paths that escape via `..`) that land
    /// outside the canonical root are rejected.
    pub fn validate_working_dirs(&self, path: &Path, project_root: &Path) -> Result<(), Error> {
        let canonical_root = canonicalize_lossy(project_root);
        for entry in &self.checkers {
            if let Some(wd) = entry.working_dir.as_ref() {
                let joined = if wd.is_absolute() {
                    wd.clone()
                } else {
                    project_root.join(wd)
                };
                let canonical = canonicalize_lossy(&joined);
                if canonical != canonical_root && !canonical.starts_with(&canonical_root) {
                    return Err(Error::ConfigInvalid {
                        path: path.to_path_buf(),
                        message: format!(
                            "checker \"{}\" working_dir escapes the project root (got `{}`)",
                            entry.name,
                            wd.display()
                        ),
                    });
                }
            }
        }
        Ok(())
    }

    /// Resolve an entry's working directory against the project root.
    /// Always returns an absolute path inside (or equal to) the
    /// canonical root; escapes are rejected.
    pub fn resolve_working_dir(
        entry: &CheckerEntry,
        project_root: &Path,
    ) -> Result<PathBuf, Error> {
        let canonical_root = canonicalize_lossy(project_root);
        match entry.working_dir.as_ref() {
            None => Ok(project_root.to_path_buf()),
            Some(wd) => {
                let joined = if wd.is_absolute() {
                    wd.clone()
                } else {
                    project_root.join(wd)
                };
                let canonical = canonicalize_lossy(&joined);
                if canonical != canonical_root && !canonical.starts_with(&canonical_root) {
                    return Err(Error::ConfigInvalid {
                        path: PathBuf::from("driftwatch.toml"),
                        message: format!(
                            "checker \"{}\" working_dir escapes the project root (got `{}`)",
                            entry.name,
                            wd.display()
                        ),
                    });
                }
                // Prefer the canonical path when it exists so symlinked
                // roots resolve consistently; otherwise use the joined
                // (still jailed) path.
                if joined.exists() {
                    Ok(canonical)
                } else {
                    Ok(joined)
                }
            }
        }
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

    #[test]
    fn rejects_duplicate_checker_names() {
        let tmp = tempdir().unwrap();
        let p = tmp.path().join("driftwatch.toml");
        fs::write(
            &p,
            "[[checkers]]\nname = \"architecture\"\ncommand = \"a\"\n\n[[checkers]]\nname = \"architecture\"\ncommand = \"b\"\n",
        )
        .unwrap();
        let err = Config::load(&p).unwrap_err();
        assert!(
            err.to_string()
                .contains("duplicate checker name \"architecture\""),
            "got: {err}"
        );
    }

    #[test]
    fn rejects_empty_command() {
        let tmp = tempdir().unwrap();
        let p = tmp.path().join("driftwatch.toml");
        fs::write(&p, "[[checkers]]\nname = \"x\"\ncommand = \"\"\n").unwrap();
        let err = Config::load(&p).unwrap_err();
        assert!(
            err.to_string().contains("has an empty command"),
            "got: {err}"
        );
    }

    #[test]
    fn rejects_zero_timeout() {
        let tmp = tempdir().unwrap();
        let p = tmp.path().join("driftwatch.toml");
        fs::write(
            &p,
            "[[checkers]]\nname = \"x\"\ncommand = \"c\"\ntimeout_ms = 0\n",
        )
        .unwrap();
        let err = Config::load(&p).unwrap_err();
        assert!(err.to_string().contains("timeout_ms = 0"), "got: {err}");
    }

    #[test]
    fn rejects_unknown_field_with_hint() {
        let tmp = tempdir().unwrap();
        let p = tmp.path().join("driftwatch.toml");
        fs::write(
            &p,
            "[[checkers]]\nname = \"x\"\ncommand = \"c\"\ncommad = \"oops\"\n",
        )
        .unwrap();
        let err = Config::load(&p).unwrap_err();
        let s = err.to_string();
        assert!(s.contains("unknown field \"commad\""), "got: {s}");
        assert!(s.contains("did you mean \"command\""), "got: {s}");
    }

    #[test]
    fn rejects_working_dir_escape() {
        let tmp = tempdir().unwrap();
        let root = tmp.path().join("proj");
        std::fs::create_dir_all(&root).unwrap();
        let p = root.join("driftwatch.toml");
        fs::write(
            &p,
            "[[checkers]]\nname = \"x\"\ncommand = \"c\"\nworking_dir = \"../../etc\"\n",
        )
        .unwrap();
        let cfg = Config::load(&p).unwrap();
        let err = cfg.validate_working_dirs(&p, &root).unwrap_err();
        assert!(
            err.to_string()
                .contains("working_dir escapes the project root"),
            "got: {err}"
        );
    }

    #[test]
    fn resolves_relative_working_dir_against_root() {
        let tmp = tempdir().unwrap();
        let root = tmp.path().join("proj");
        let sub = root.join("tools/spec");
        std::fs::create_dir_all(&sub).unwrap();
        let entry = CheckerEntry {
            name: "x".into(),
            command: "c".into(),
            working_dir: Some(PathBuf::from("tools/spec")),
            ..Default::default()
        };
        let resolved = Config::resolve_working_dir(&entry, &root).unwrap();
        let canonical_sub = std::fs::canonicalize(&sub).unwrap();
        assert_eq!(resolved, canonical_sub);
    }
}
