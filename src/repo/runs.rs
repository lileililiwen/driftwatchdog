//! `runs` repository: persistent record of a single command invocation.
//!
//! The `runtime-memory` change will extend this with `insert`, `list`,
//! `find_by_id`, and the bounded-stream fields. The foundation change only
//! requires that the type compiles and that an `id` is reserved on insert.

use rusqlite::{params, OptionalExtension};

use crate::error::Error;
use crate::repo::Db;

/// Status of a recorded run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunStatus {
    Running,
    Success,
    Failed,
    StartFailed,
}

impl RunStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            RunStatus::Running => "running",
            RunStatus::Success => "success",
            RunStatus::Failed => "failed",
            RunStatus::StartFailed => "start_failed",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "running" => Some(Self::Running),
            "success" => Some(Self::Success),
            "failed" => Some(Self::Failed),
            "start_failed" => Some(Self::StartFailed),
            _ => None,
        }
    }
}

/// Minimal projection used by the foundation change. Later changes will
/// extend this with output excerpts, Git metadata, and a bug fingerprint.
#[derive(Debug, Clone)]
pub struct RunRecord {
    pub id: i64,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub program: String,
    pub argv_json: String,
    pub cwd: String,
    pub exit_code: Option<i32>,
    pub status: RunStatus,
}

pub struct Runs<'a> {
    db: &'a Db,
}

impl<'a> Runs<'a> {
    pub fn new(db: &'a Db) -> Self {
        Self { db }
    }

    /// Reserve a new run row and return its id. Used by `runtime-memory` to
    /// obtain an id before the child process has finished. The full insert
    /// with all fields will be added in the next change package.
    pub fn reserve(
        &self,
        started_at: &str,
        program: &str,
        argv_json: &str,
        cwd: &str,
    ) -> Result<i64, Error> {
        self.db.conn().execute(
            "INSERT INTO runs (started_at, program, argv, cwd, status) VALUES (?1, ?2, ?3, ?4, 'running')",
            params![started_at, program, argv_json, cwd],
        )?;
        Ok(self.db.conn().last_insert_rowid())
    }

    /// Look up a run by id. Returns `None` when not present.
    pub fn find(&self, id: i64) -> Result<Option<RunRecord>, Error> {
        let row = self
            .db
            .conn()
            .query_row(
                "SELECT id, started_at, finished_at, program, argv, cwd, exit_code, status
                 FROM runs WHERE id = ?1",
                params![id],
                map_run,
            )
            .optional()?;
        Ok(row)
    }
}

fn map_run(row: &rusqlite::Row<'_>) -> rusqlite::Result<RunRecord> {
    let status_str: String = row.get(7)?;
    let status = RunStatus::parse(&status_str).ok_or_else(|| {
        rusqlite::Error::FromSqlConversionFailure(
            7,
            rusqlite::types::Type::Text,
            Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("unknown run status: {status_str}"),
            )),
        )
    })?;
    Ok(RunRecord {
        id: row.get(0)?,
        started_at: row.get(1)?,
        finished_at: row.get(2)?,
        program: row.get(3)?,
        argv_json: row.get(4)?,
        cwd: row.get(5)?,
        exit_code: row.get(6)?,
        status,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reserve_and_find_round_trip() {
        let db = Db::open_in_memory().unwrap();
        let id = Runs::new(&db)
            .reserve("2026-01-01T00:00:00Z", "echo", "[\"hi\"]", "/tmp")
            .unwrap();
        let rec = Runs::new(&db).find(id).unwrap().unwrap();
        assert_eq!(rec.id, id);
        assert_eq!(rec.program, "echo");
        assert_eq!(rec.status, RunStatus::Running);
    }
}
