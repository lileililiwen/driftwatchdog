//! Repository layer: typed wrappers around the SQLite connection.
//!
//! Each submodule exposes CRUD primitives for one entity. The foundation
//! change keeps the surface intentionally small — enough to round-trip
//! records through the database so later changes can extend the methods
//! without re-plumbing connection handling.

pub mod alerts;
pub mod bugs;
pub mod correlations;
pub mod evidence;
pub mod links;
pub mod runs;

use std::path::Path;

use rusqlite::Connection;

use crate::error::Error;
use crate::storage;

/// Escape `LIKE` wildcards (`\`, `%`, `_`) so tag filters match
/// literally. Used with `ESCAPE '\'`.
///
/// Tags are stored as a JSON array (e.g. `["auth","db"]`); the
/// pattern matches the quoted JSON string (`%"auth"%`) so `auth`
/// never matches `oauth`, and embedded `%_` match literally.
pub fn escape_like(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        if ch == '\\' || ch == '%' || ch == '_' {
            out.push('\\');
        }
        out.push(ch);
    }
    out
}

/// `LIKE` pattern matching exactly one JSON-encoded tag value.
/// Pair with `ESCAPE '\'` in SQL.
pub fn tag_like_pattern(tag: &str) -> String {
    format!("%\"{}\"%", escape_like(tag))
}

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

    /// Open the database at `path` read-only without running
    /// migrations. Used by `driftwatch doctor` so diagnostics never
    /// mutate the production database. Fails when the file is
    /// missing or not a valid database.
    pub fn open_read_only(path: &Path) -> Result<Self, Error> {
        use rusqlite::OpenFlags;
        let conn = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        conn.pragma_update(None, "query_only", "ON")?;
        Ok(Self { conn })
    }

    /// Borrow the underlying connection. Used by repository implementations
    /// and exposed publicly for diagnostic queries (e.g. `PRAGMA` checks in
    /// tests and the future `doctor` command). Prefer the typed repository
    /// methods for application code.
    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    /// Borrow the underlying connection mutably. Used by repositories
    /// that need to open a transaction.
    pub fn conn_mut(&mut self) -> &mut Connection {
        &mut self.conn
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escape_like_escapes_wildcards() {
        assert_eq!(escape_like("auth"), "auth");
        assert_eq!(escape_like("a%b_c\\d"), "a\\%b\\_c\\\\d");
    }

    #[test]
    fn tag_pattern_quotes_value() {
        assert_eq!(tag_like_pattern("auth"), "%\"auth\"%");
        assert_eq!(tag_like_pattern("a%b"), "%\"a\\%b\"%");
    }

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
            "gate_artifacts",
            "schema_version",
        ] {
            assert!(tables.iter().any(|t| t == required), "missing {required}");
        }
    }
}
