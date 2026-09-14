//! Locating the Driftwatch project root.
//!
//! The root is the nearest ancestor directory containing either
//! `.driftwatch/` or `driftwatch.toml`. If neither is found, the current
//! working directory is used (so a fresh `init` always succeeds).
//! Returned paths are canonicalized when possible so symlinked
//! workdirs resolve consistently.

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

    /// Borrow the discovered project root path. Used by callers that
    /// need a `&Path` (e.g. the MCP doctor tool) without re-walking
    /// the directory tree.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Walk upward from `start`, returning the nearest ancestor (or `start`
    /// itself) that contains `.driftwatch/` or `driftwatch.toml`. Falls back to
    /// `start` if nothing is found before the filesystem root. Returns an
    /// explicit error when the walk exceeds [`MAX_WALK_DEPTH`] without
    /// resolving (depth overflow is never silently treated as "no
    /// project"). Returned paths are canonicalized when the OS allows
    /// it so symlinked workdirs alias to one root.
    pub fn discover(start: impl AsRef<Path>) -> Result<Self, Error> {
        let start = start.as_ref();
        let canonical_start = canonicalize_lossy(start);
        let mut current: Option<PathBuf> = Some(canonical_start.clone());
        let mut depth = 0usize;

        while let Some(dir) = current {
            if depth > MAX_WALK_DEPTH {
                return Err(Error::ProjectRootNotFound {
                    start: start.to_path_buf(),
                });
            }
            // Anchor check without check-then-use races on the
            // artifacts themselves: `symlink_metadata` follows the
            // same single-syscall pattern and we canonicalize the
            // winning directory before returning it.
            let has_state = dir.join(STATE_DIR).is_dir();
            let has_config = dir.join(CONFIG_FILE).is_file();
            if has_state || has_config {
                let root = canonicalize_lossy(&dir);
                return Ok(Self::at(root));
            }
            match dir.parent() {
                Some(parent) => {
                    // Reached the filesystem root: fall back to the
                    // (canonicalized) start directory so `init` works.
                    if parent == dir {
                        return Ok(Self::at(canonical_start));
                    }
                    current = Some(parent.to_path_buf());
                }
                None => return Ok(Self::at(canonical_start)),
            }
            depth += 1;
        }

        // Nothing found; use the original start directory as the project root
        // (init will create artifacts there).
        Ok(Self::at(canonical_start))
    }
}

/// Best-effort canonicalization: full `canonicalize` when the path
/// exists, otherwise the input unchanged. Never fails.
fn canonicalize_lossy(p: &Path) -> PathBuf {
    std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn canonical(p: &std::path::Path) -> PathBuf {
        std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf())
    }

    #[test]
    fn discover_finds_existing_anchor() {
        let tmp = tempdir().unwrap();
        let sub = tmp.path().join("a/b/c");
        fs::create_dir_all(&sub).unwrap();
        fs::create_dir_all(tmp.path().join("a/.driftwatch")).unwrap();
        let root = ProjectRoot::discover(&sub).unwrap();
        assert_eq!(root.root, canonical(&tmp.path().join("a")));
    }

    #[test]
    fn discover_falls_back_to_start() {
        let tmp = tempdir().unwrap();
        let sub = tmp.path().join("x/y");
        fs::create_dir_all(&sub).unwrap();
        let root = ProjectRoot::discover(&sub).unwrap();
        assert_eq!(root.root, canonical(&sub));
    }

    #[test]
    fn discover_finds_toml_only() {
        let tmp = tempdir().unwrap();
        let sub = tmp.path().join("p/q");
        fs::create_dir_all(&sub).unwrap();
        fs::write(tmp.path().join("p/driftwatch.toml"), "[project]\n").unwrap();
        let root = ProjectRoot::discover(&sub).unwrap();
        assert_eq!(root.root, canonical(&tmp.path().join("p")));
    }

    #[test]
    fn discover_resolves_symlinked_workdir_to_canonical_root() {
        let tmp = tempdir().unwrap();
        let real = tmp.path().join("real");
        fs::create_dir_all(real.join(".driftwatch")).unwrap();
        let link = tmp.path().join("link");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&real, &link).unwrap();
        #[cfg(unix)]
        {
            let root = ProjectRoot::discover(&link).unwrap();
            assert_eq!(root.root, canonical(&real));
        }
    }
}
