//! `runs` repository: persistent record of a single command invocation.
//!
//! The `runtime-memory` change extends this with `insert_full`, `list`, the
//! bounded-stream fields, Git metadata, and tags. The foundation change only
//! required a working `id` reservation.

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

/// Full projection of a row in the `runs` table.
#[derive(Debug, Clone)]
pub struct RunRecord {
    pub id: i64,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub duration_ms: Option<i64>,
    pub program: String,
    pub argv_json: String,
    pub cwd: String,
    pub exit_code: Option<i32>,
    pub status: RunStatus,
    pub tags: Vec<String>,
    pub stdout_excerpt: Option<String>,
    pub stderr_excerpt: Option<String>,
    pub stdout_truncated: bool,
    pub stderr_truncated: bool,
    pub git_commit: Option<String>,
    pub git_branch: Option<String>,
    pub git_dirty: Option<bool>,
}

/// Fields needed to finalize a reserved run row. Passed to `Runs::insert_full`.
#[derive(Debug, Clone)]
pub struct RunCompletion<'a> {
    pub finished_at: &'a str,
    pub duration_ms: i64,
    pub exit_code: Option<i32>,
    pub status: RunStatus,
    pub tags: &'a [String],
    pub stdout_excerpt: Option<&'a str>,
    pub stderr_excerpt: Option<&'a str>,
    pub stdout_truncated: bool,
    pub stderr_truncated: bool,
    pub git_commit: Option<&'a str>,
    pub git_branch: Option<&'a str>,
    pub git_dirty: Option<bool>,
}

/// Filter for `Runs::list`. All fields are optional; an empty filter is "list
/// the most recent runs up to the limit".
#[derive(Debug, Clone, Default)]
pub struct ListFilter {
    pub limit: usize,
    pub only_failed: bool,
    pub tag: Option<String>,
}

pub struct Runs<'a> {
    db: &'a Db,
}

impl<'a> Runs<'a> {
    pub fn new(db: &'a Db) -> Self {
        Self { db }
    }

    /// Reserve a new run row and return its id. Used by `runtime-memory` to
    /// obtain an id before the child process has finished. The full completion
    /// is written via `insert_full` after the child exits.
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
                "SELECT id, started_at, finished_at, duration_ms, program, argv, cwd, exit_code, status,
                        tags, stdout_excerpt, stderr_excerpt, stdout_truncated, stderr_truncated,
                        git_commit, git_branch, git_dirty
                 FROM runs WHERE id = ?1",
                params![id],
                map_run,
            )
            .optional()?;
        Ok(row)
    }

    /// Finalize a reserved run row with exit status, output excerpts,
    /// truncation flags, Git metadata, and tags.
    pub fn insert_full(&self, id: i64, completion: &RunCompletion<'_>) -> Result<(), Error> {
        let tags_json = serde_json::to_string(completion.tags)?;
        let stdout_truncated = if completion.stdout_truncated { 1 } else { 0 };
        let stderr_truncated = if completion.stderr_truncated { 1 } else { 0 };
        let git_dirty = completion.git_dirty.map(|b| if b { 1 } else { 0 });
        self.db.conn().execute(
            "UPDATE runs SET
                finished_at = ?2,
                duration_ms = ?3,
                exit_code = ?4,
                status = ?5,
                tags = ?6,
                stdout_excerpt = ?7,
                stderr_excerpt = ?8,
                stdout_truncated = ?9,
                stderr_truncated = ?10,
                git_commit = ?11,
                git_branch = ?12,
                git_dirty = ?13
             WHERE id = ?1",
            params![
                id,
                completion.finished_at,
                completion.duration_ms,
                completion.exit_code,
                completion.status.as_str(),
                tags_json,
                completion.stdout_excerpt,
                completion.stderr_excerpt,
                stdout_truncated,
                stderr_truncated,
                completion.git_commit,
                completion.git_branch,
                git_dirty,
            ],
        )?;
        Ok(())
    }

    /// List runs, newest first, with optional filters.
    pub fn list(&self, filter: &ListFilter) -> Result<Vec<RunRecord>, Error> {
        // Build the WHERE clause incrementally. Tag matching is a coarse
        // substring on the JSON-serialized tags column — fingerprinting is not
        // online yet, so we cannot rely on a normalized table.
        let mut sql = String::from(
            "SELECT id, started_at, finished_at, duration_ms, program, argv, cwd, exit_code, status,
                    tags, stdout_excerpt, stderr_excerpt, stdout_truncated, stderr_truncated,
                    git_commit, git_branch, git_dirty
             FROM runs",
        );
        let mut predicates: Vec<&'static str> = Vec::new();
        if filter.only_failed {
            predicates.push("status IN ('failed', 'start_failed')");
        }
        if filter.tag.is_some() {
            predicates.push("tags LIKE ?1");
        }
        if !predicates.is_empty() {
            sql.push_str(" WHERE ");
            sql.push_str(&predicates.join(" AND "));
        }
        sql.push_str(" ORDER BY started_at DESC LIMIT ");
        sql.push_str(&filter.limit.to_string());

        let mut stmt = self.db.conn().prepare(&sql)?;
        let rows: Vec<RunRecord> = if let Some(tag) = &filter.tag {
            let pattern = format!("%\"{}\"%", tag);
            stmt.query_map(params![pattern], map_run)?
                .collect::<rusqlite::Result<Vec<_>>>()?
        } else {
            stmt.query_map([], map_run)?
                .collect::<rusqlite::Result<Vec<_>>>()?
        };
        Ok(rows)
    }
}

fn map_run(row: &rusqlite::Row<'_>) -> rusqlite::Result<RunRecord> {
    let status_str: String = row.get(8)?;
    let status = RunStatus::parse(&status_str).ok_or_else(|| {
        rusqlite::Error::FromSqlConversionFailure(
            8,
            rusqlite::types::Type::Text,
            Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("unknown run status: {status_str}"),
            )),
        )
    })?;
    let tags_json: String = row.get(9)?;
    let tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();
    let stdout_truncated: i64 = row.get(12)?;
    let stderr_truncated: i64 = row.get(13)?;
    let git_dirty: Option<i64> = row.get(16)?;
    Ok(RunRecord {
        id: row.get(0)?,
        started_at: row.get(1)?,
        finished_at: row.get(2)?,
        duration_ms: row.get(3)?,
        program: row.get(4)?,
        argv_json: row.get(5)?,
        cwd: row.get(6)?,
        exit_code: row.get(7)?,
        status,
        tags,
        stdout_excerpt: row.get(10)?,
        stderr_excerpt: row.get(11)?,
        stdout_truncated: stdout_truncated != 0,
        stderr_truncated: stderr_truncated != 0,
        git_commit: row.get(14)?,
        git_branch: row.get(15)?,
        git_dirty: git_dirty.map(|v| v != 0),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reserve_running(db: &Db, started_at: &str, program: &str, argv_json: &str) -> i64 {
        Runs::new(db)
            .reserve(started_at, program, argv_json, "/tmp")
            .unwrap()
    }

    #[test]
    fn reserve_and_find_round_trip() {
        let db = Db::open_in_memory().unwrap();
        let id = reserve_running(&db, "2026-01-01T00:00:00Z", "echo", "[\"hi\"]");
        let rec = Runs::new(&db).find(id).unwrap().unwrap();
        assert_eq!(rec.id, id);
        assert_eq!(rec.program, "echo");
        assert_eq!(rec.status, RunStatus::Running);
        assert!(rec.tags.is_empty());
    }

    #[test]
    fn insert_full_round_trip() {
        let db = Db::open_in_memory().unwrap();
        let id = reserve_running(&db, "2026-01-01T00:00:00Z", "echo", "[\"hi\"]");
        Runs::new(&db)
            .insert_full(
                id,
                &RunCompletion {
                    finished_at: "2026-01-01T00:00:01Z",
                    duration_ms: 1000,
                    exit_code: Some(0),
                    status: RunStatus::Success,
                    tags: &["auth".to_string()],
                    stdout_excerpt: Some("hi\n"),
                    stderr_excerpt: None,
                    stdout_truncated: false,
                    stderr_truncated: false,
                    git_commit: Some("abc123"),
                    git_branch: Some("main"),
                    git_dirty: Some(false),
                },
            )
            .unwrap();
        let rec = Runs::new(&db).find(id).unwrap().unwrap();
        assert_eq!(rec.status, RunStatus::Success);
        assert_eq!(rec.exit_code, Some(0));
        assert_eq!(rec.duration_ms, Some(1000));
        assert_eq!(rec.tags, vec!["auth".to_string()]);
        assert_eq!(rec.stdout_excerpt.as_deref(), Some("hi\n"));
        assert_eq!(rec.git_commit.as_deref(), Some("abc123"));
        assert_eq!(rec.git_dirty, Some(false));
    }

    #[test]
    fn list_orders_by_started_at_desc() {
        let db = Db::open_in_memory().unwrap();
        let r1 = reserve_running(&db, "2026-01-01T00:00:00Z", "a", "[]");
        let r2 = reserve_running(&db, "2026-01-02T00:00:00Z", "b", "[]");
        let r3 = reserve_running(&db, "2026-01-03T00:00:00Z", "c", "[]");
        let rows = Runs::new(&db)
            .list(&ListFilter {
                limit: 10,
                ..Default::default()
            })
            .unwrap();
        let ids: Vec<i64> = rows.iter().map(|r| r.id).collect();
        assert_eq!(ids, vec![r3, r2, r1]);
    }

    #[test]
    fn list_filter_failed() {
        let db = Db::open_in_memory().unwrap();
        let id_ok = reserve_running(&db, "2026-01-01T00:00:00Z", "ok", "[]");
        let id_fail = reserve_running(&db, "2026-01-02T00:00:00Z", "fail", "[]");
        Runs::new(&db)
            .insert_full(
                id_ok,
                &RunCompletion {
                    finished_at: "2026-01-01T00:00:01Z",
                    duration_ms: 1,
                    exit_code: Some(0),
                    status: RunStatus::Success,
                    tags: &[],
                    stdout_excerpt: None,
                    stderr_excerpt: None,
                    stdout_truncated: false,
                    stderr_truncated: false,
                    git_commit: None,
                    git_branch: None,
                    git_dirty: None,
                },
            )
            .unwrap();
        Runs::new(&db)
            .insert_full(
                id_fail,
                &RunCompletion {
                    finished_at: "2026-01-02T00:00:01Z",
                    duration_ms: 1,
                    exit_code: Some(2),
                    status: RunStatus::Failed,
                    tags: &[],
                    stdout_excerpt: None,
                    stderr_excerpt: None,
                    stdout_truncated: false,
                    stderr_truncated: false,
                    git_commit: None,
                    git_branch: None,
                    git_dirty: None,
                },
            )
            .unwrap();
        let rows = Runs::new(&db)
            .list(&ListFilter {
                limit: 10,
                only_failed: true,
                tag: None,
            })
            .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, id_fail);
    }

    #[test]
    fn list_filter_tag_substring() {
        let db = Db::open_in_memory().unwrap();
        let id_auth = reserve_running(&db, "2026-01-01T00:00:00Z", "auth-cmd", "[]");
        let id_other = reserve_running(&db, "2026-01-02T00:00:00Z", "other", "[]");
        Runs::new(&db)
            .insert_full(
                id_auth,
                &RunCompletion {
                    finished_at: "2026-01-01T00:00:01Z",
                    duration_ms: 1,
                    exit_code: Some(0),
                    status: RunStatus::Success,
                    tags: &["auth".to_string()],
                    stdout_excerpt: None,
                    stderr_excerpt: None,
                    stdout_truncated: false,
                    stderr_truncated: false,
                    git_commit: None,
                    git_branch: None,
                    git_dirty: None,
                },
            )
            .unwrap();
        Runs::new(&db)
            .insert_full(
                id_other,
                &RunCompletion {
                    finished_at: "2026-01-02T00:00:01Z",
                    duration_ms: 1,
                    exit_code: Some(0),
                    status: RunStatus::Success,
                    tags: &["ui".to_string()],
                    stdout_excerpt: None,
                    stderr_excerpt: None,
                    stdout_truncated: false,
                    stderr_truncated: false,
                    git_commit: None,
                    git_branch: None,
                    git_dirty: None,
                },
            )
            .unwrap();
        let rows = Runs::new(&db)
            .list(&ListFilter {
                limit: 10,
                only_failed: false,
                tag: Some("auth".to_string()),
            })
            .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, id_auth);
    }

    #[test]
    fn list_respects_limit() {
        let db = Db::open_in_memory().unwrap();
        for i in 0..5 {
            reserve_running(&db, &format!("2026-01-0{}T00:00:00Z", i + 1), "x", "[]");
        }
        let rows = Runs::new(&db)
            .list(&ListFilter {
                limit: 3,
                ..Default::default()
            })
            .unwrap();
        assert_eq!(rows.len(), 3);
    }
}
