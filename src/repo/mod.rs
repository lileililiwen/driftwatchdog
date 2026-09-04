//! Repository layer: typed wrappers around the SQLite connection.
//!
//! Each submodule exposes CRUD primitives for one entity. The foundation
//! change keeps the surface intentionally small — enough to round-trip
//! records through the database so later changes can extend the methods
//! without re-plumbing connection handling.

pub mod alerts;
pub mod bugs;
pub mod correlations;
pub mod links;
pub mod runs;

use std::path::Path;

use rusqlite::Connection;

use crate::error::Error;
use crate::storage;

/// Thin wrapper around `rusqlite::Connection` passed to each repository.
/// Repositories take `&Db` (or `&mut Db` when they need a transaction) and
/// operate on the shared connection.
pub struct Db {
    conn: Connection,
}

impl Db {
    /// Open the database at `path` and apply pending migrations.
    pub fn open(path: &Path) -> Result<Self, Error> {
        let conn = storage::open(path)?;
        let mut me = Self { conn };
        storage::migrations::apply(&mut me.conn)?;
        Ok(me)
    }

    /// Open an in-memory database (used by tests).
    pub fn open_in_memory() -> Result<Self, Error> {
        let conn = storage::open(std::path::Path::new(":memory:"))?;
        let mut me = Self { conn };
        storage::migrations::apply(&mut me.conn)?;
        Ok(me)
    }

    /// Borrow the underlying connection. Used by repository implementations
    /// and exposed publicly for diagnostic queries (e.g. `PRAGMA` checks in
    /// tests and the future `doctor` command). Prefer the typed repository
    /// methods for application code.
    pub fn conn(&self) -> &Connection {
        &self.conn
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_in_memory_creates_schema() {
        let db = Db::open_in_memory().unwrap();
        let tables: Vec<String> = db
            .conn()
            .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .filter_map(Result::ok)
            .collect();
        for required in [
            "runs",
            "fingerprints",
            "occurrences",
            "check_snapshots",
            "drift_alerts",
            "correlations",
            "manual_links",
            "schema_version",
        ] {
            assert!(tables.iter().any(|t| t == required), "missing {required}");
        }
    }
}
