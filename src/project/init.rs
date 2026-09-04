//! `driftwatch init`: idempotent project initialization.
//!
//! Steps performed:
//!  1. Discover the project root (walks up from `cwd`).
//!  2. Create `.driftwatch/` if absent.
//!  3. Open (or create) `state.db` and apply pending migrations.
//!  4. Write a default `driftwatch.toml` if `--no-config` is not set and the
//!     file does not exist.
//!  5. Print the resolved project root.

use std::fs;
use std::path::Path;

use crate::cli::InitArgs;
use crate::error::Error;
use crate::project::config::Config;
use crate::project::root::ProjectRoot;
use crate::storage;

/// Result of a successful `init` invocation. Surfaced for tests and used by
/// future commands that want to know the resolved root without re-discovering.
#[derive(Debug, Clone)]
pub struct InitOutcome {
    pub root: ProjectRoot,
    pub config_written: bool,
    pub db_created: bool,
    pub schema_version: i64,
}

/// Initialize the project rooted at `cwd`. Returns the outcome so callers
/// (notably tests) can assert against it.
pub fn init(args: InitArgs) -> Result<InitOutcome, Error> {
    let cwd = std::env::current_dir().map_err(|e| Error::io("<cwd>", e))?;
    init_at(&cwd, &args)
}

/// Initialize the project at an explicit directory. Used by `init` and tests.
pub fn init_at(cwd: &Path, args: &InitArgs) -> Result<InitOutcome, Error> {
    let root = ProjectRoot::discover(cwd)?;

    let state_existed = root.state_dir.exists();
    fs::create_dir_all(&root.state_dir).map_err(|e| Error::io(&root.state_dir, e))?;
    let db_existed = root.db_path.exists();

    let mut conn = storage::open(&root.db_path)?;
    let version = storage::migrations::apply(&mut conn)?;

    let mut config_written = false;
    if !args.no_config && !root.config_path.exists() {
        let cfg = Config::defaults();
        let body = cfg.to_toml()?;
        fs::write(&root.config_path, body).map_err(|e| Error::io(&root.config_path, e))?;
        config_written = true;
    }

    println!("driftwatch: initialized project at {}", root.root.display());
    if !state_existed {
        println!("  created {}", root.state_dir.display());
    }
    if !db_existed {
        println!("  created {}", root.db_path.display());
    }
    if config_written {
        println!("  created {}", root.config_path.display());
    } else if !args.no_config && root.config_path.exists() {
        println!("  preserved existing {}", root.config_path.display());
    }
    println!("  schema version: {}", version);

    Ok(InitOutcome {
        root,
        config_written,
        db_created: !db_existed,
        schema_version: version,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn init_creates_artifacts() {
        let tmp = tempdir().unwrap();
        let out = init_at(tmp.path(), &InitArgs::default()).unwrap();
        assert!(out.root.state_dir.is_dir());
        assert!(out.root.db_path.is_file());
        assert!(out.root.config_path.is_file());
        assert!(out.config_written);
        assert!(out.db_created);
        assert_eq!(out.schema_version, 1);
    }

    #[test]
    fn init_is_idempotent() {
        let tmp = tempdir().unwrap();
        init_at(tmp.path(), &InitArgs::default()).unwrap();
        let out2 = init_at(tmp.path(), &InitArgs::default()).unwrap();
        assert!(!out2.config_written);
        assert!(!out2.db_created);
    }

    #[test]
    fn init_preserves_user_config() {
        let tmp = tempdir().unwrap();
        let cfg_path = tmp.path().join("driftwatch.toml");
        std::fs::write(&cfg_path, "[project]\nname = \"keep-me\"\n").unwrap();
        let out = init_at(tmp.path(), &InitArgs::default()).unwrap();
        assert!(!out.config_written);
        let read = std::fs::read_to_string(&cfg_path).unwrap();
        assert!(read.contains("keep-me"));
    }

    #[test]
    fn init_with_no_config_skips_toml() {
        let tmp = tempdir().unwrap();
        let out = init_at(tmp.path(), &InitArgs { no_config: true }).unwrap();
        assert!(!out.root.config_path.exists());
        assert!(!out.config_written);
    }
}
