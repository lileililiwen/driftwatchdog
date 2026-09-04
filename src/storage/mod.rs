//! SQLite storage: connection setup and migration management.

pub mod migrations;
pub mod schema;

use std::path::Path;

use rusqlite::Connection;

use crate::error::Error;

/// Open the SQLite database at `path`, creating it if missing.
///
/// Applies a baseline PRAGMA configuration suitable for a local-first CLI:
/// foreign-key enforcement, WAL journal mode, and `NORMAL` synchronous
/// commits. These are set on every open so a database created with a future
/// version of Driftwatch remains consistent.
pub fn open(path: &Path) -> Result<Connection, Error> {
    let conn = Connection::open(path)?;
    configure(&conn)?;
    Ok(conn)
}

fn configure(conn: &Connection) -> Result<(), Error> {
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn pragmas_are_applied() {
        let tmp = tempdir().unwrap();
        let p = tmp.path().join("state.db");
        let conn = open(&p).unwrap();

        let journal: String = conn
            .query_row("PRAGMA journal_mode", [], |r| r.get(0))
            .unwrap();
        assert_eq!(journal.to_lowercase(), "wal");

        let fk: i64 = conn
            .query_row("PRAGMA foreign_keys", [], |r| r.get(0))
            .unwrap();
        assert_eq!(fk, 1);
    }
}
