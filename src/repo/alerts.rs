//! `check_snapshots` + `drift_alerts` repositories.
//!
//! Populated by the `checker-and-drift-alerts` change. The foundation change
//! only defines the row types and provides a count helper used by `doctor`
//! (added later) and by future tests.

use rusqlite::Connection;

use crate::error::Error;
use crate::repo::Db;

pub struct Alerts<'a> {
    db: &'a Db,
}

impl<'a> Alerts<'a> {
    pub fn new(db: &'a Db) -> Self {
        Self { db }
    }

    /// Number of recorded check snapshots. Always defined; useful for tests
    /// and the future `doctor` command.
    pub fn snapshot_count(&self) -> Result<i64, Error> {
        count(self.db.conn(), "check_snapshots")
    }
}

fn count(conn: &Connection, table: &str) -> Result<i64, Error> {
    let q = format!("SELECT COUNT(*) FROM {table}");
    let n: i64 = conn.query_row(&q, [], |r| r.get(0))?;
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_count_is_zero_on_fresh_db() {
        let db = Db::open_in_memory().unwrap();
        assert_eq!(Alerts::new(&db).snapshot_count().unwrap(), 0);
    }
}
