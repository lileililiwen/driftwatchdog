//! Locating the Driftwatch project root.
//!
//! The root is the nearest ancestor directory containing either
//! `.driftwatch/` or `driftwatch.toml`. If neither is found, the current
//! working directory is used (so a fresh `init` always succeeds).

use std::path::{Path, PathBuf};

use crate::error::Error;

const STATE_DIR: &str = ".driftwatch";
const CONFIG_FILE: &str = "driftwatch.toml";
/// Hard cap on how many parent directories we will walk.
const MAX_WALK_DEPTH: usize = 32;

/// A resolved Driftwatch project root.
#[derive(Debug, Clone)]
pub struct ProjectRoot {
    pub root: PathBuf,
    pub state_dir: PathBuf,
    pub config_path: PathBuf,
    pub db_path: PathBuf,
}

impl ProjectRoot {
    /// Build a `ProjectRoot` for `root`, regardless of whether it already
    /// contains Driftwatch artifacts. Use [`ProjectRoot::discover`] to search
    /// upward from a working directory.
    pub fn at(root: impl Into<PathBuf>) -> Self {
        let root = root.into();
        let state_dir = root.join(STATE_DIR);
        let config_path = root.join(CONFIG_FILE);
        let db_path = state_dir.join("state.db");
        Self {
            root,
            state_dir,
            config_path,
            db_path,
        }
    }

    /// Walk upward from `start`, returning the nearest ancestor (or `start`
    /// itself) that contains `.driftwatch/` or `driftwatch.toml`. Falls back to
    /// `start` if nothing is found within [`MAX_WALK_DEPTH`] levels.
    pub fn discover(start: impl AsRef<Path>) -> Result<Self, Error> {
        let start = start.as_ref();
        let mut current: Option<&Path> = Some(start);
        let mut depth = 0usize;

        while let Some(dir) = current {
            if depth > MAX_WALK_DEPTH {
                break;
            }
            if dir.join(STATE_DIR).is_dir() || dir.join(CONFIG_FILE).is_file() {
                return Ok(Self::at(dir));
            }
            current = dir.parent();
            depth += 1;
        }

        // Nothing found; use the original start directory as the project root
        // (init will create artifacts there).
        Ok(Self::at(start))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn discover_finds_existing_anchor() {
        let tmp = tempdir().unwrap();
        let sub = tmp.path().join("a/b/c");
        fs::create_dir_all(&sub).unwrap();
        fs::create_dir_all(tmp.path().join("a/.driftwatch")).unwrap();
        let root = ProjectRoot::discover(&sub).unwrap();
        assert_eq!(root.root, tmp.path().join("a"));
    }

    #[test]
    fn discover_falls_back_to_start() {
        let tmp = tempdir().unwrap();
        let sub = tmp.path().join("x/y");
        fs::create_dir_all(&sub).unwrap();
        let root = ProjectRoot::discover(&sub).unwrap();
        assert_eq!(root.root, sub);
    }

    #[test]
    fn discover_finds_toml_only() {
        let tmp = tempdir().unwrap();
        let sub = tmp.path().join("p/q");
        fs::create_dir_all(&sub).unwrap();
        fs::write(tmp.path().join("p/driftwatch.toml"), "[project]\n").unwrap();
        let root = ProjectRoot::discover(&sub).unwrap();
        assert_eq!(root.root, tmp.path().join("p"));
    }
}
