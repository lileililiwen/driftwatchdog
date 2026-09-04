//! Canonical SQL for the foundation schema. Stored as `&str` constants so
//! later changes can reference the same definitions and migrations can be
//! diffed cleanly against `git` history.

/// Baseline migration: creates the foundation tables defined in the
/// `project-foundation` OpenSpec change. Tables and columns use stable
/// identifiers; later migrations may add columns or new tables but should
/// never rename or remove these.
pub const MIGRATION_0001_BASELINE: &str = r#"
CREATE TABLE IF NOT EXISTS schema_version (
    version INTEGER PRIMARY KEY,
    applied_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS runs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    started_at TEXT NOT NULL,
    finished_at TEXT,
    duration_ms INTEGER,
    program TEXT NOT NULL,
    argv TEXT NOT NULL,
    cwd TEXT NOT NULL,
    exit_code INTEGER,
    status TEXT NOT NULL,
    tags TEXT NOT NULL DEFAULT '[]',
    stdout_excerpt TEXT,
    stderr_excerpt TEXT,
    stdout_truncated INTEGER NOT NULL DEFAULT 0,
    stderr_truncated INTEGER NOT NULL DEFAULT 0,
    git_commit TEXT,
    git_branch TEXT,
    git_dirty INTEGER
);

CREATE TABLE IF NOT EXISTS fingerprints (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    hash TEXT NOT NULL UNIQUE,
    canonical TEXT NOT NULL,
    summary TEXT,
    first_seen_at TEXT NOT NULL,
    last_seen_at TEXT NOT NULL,
    occurrence_count INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS occurrences (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    fingerprint_id INTEGER NOT NULL REFERENCES fingerprints(id) ON DELETE CASCADE,
    run_id INTEGER NOT NULL REFERENCES runs(id) ON DELETE CASCADE,
    seen_at TEXT NOT NULL,
    excerpt TEXT
);

CREATE TABLE IF NOT EXISTS check_snapshots (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    taken_at TEXT NOT NULL,
    checker_name TEXT NOT NULL,
    raw_json TEXT,
    status TEXT NOT NULL,
    diagnostic TEXT
);

CREATE TABLE IF NOT EXISTS drift_alerts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    snapshot_id INTEGER NOT NULL REFERENCES check_snapshots(id) ON DELETE CASCADE,
    severity TEXT NOT NULL,
    message TEXT NOT NULL,
    source TEXT,
    symbol TEXT
);

CREATE TABLE IF NOT EXISTS correlations (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    fingerprint_id INTEGER NOT NULL REFERENCES fingerprints(id) ON DELETE CASCADE,
    alert_id INTEGER NOT NULL REFERENCES drift_alerts(id) ON DELETE CASCADE,
    score REAL NOT NULL,
    label TEXT NOT NULL,
    created_at TEXT NOT NULL,
    UNIQUE(fingerprint_id, alert_id)
);

CREATE TABLE IF NOT EXISTS manual_links (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    fingerprint_id INTEGER REFERENCES fingerprints(id) ON DELETE CASCADE,
    alert_id INTEGER REFERENCES drift_alerts(id) ON DELETE CASCADE,
    note TEXT,
    created_at TEXT NOT NULL,
    CHECK ((fingerprint_id IS NOT NULL) OR (alert_id IS NOT NULL))
);

CREATE INDEX IF NOT EXISTS idx_occurrences_fingerprint ON occurrences(fingerprint_id);
CREATE INDEX IF NOT EXISTS idx_occurrences_run ON occurrences(run_id);
CREATE INDEX IF NOT EXISTS idx_alerts_snapshot ON drift_alerts(snapshot_id);
CREATE INDEX IF NOT EXISTS idx_runs_started_at ON runs(started_at);
"#;

/// Migration: extend `check_snapshots` with the Git context captured at the
/// time of the run. The `checker-and-drift-alerts` change uses these
/// columns to attribute snapshots to a commit and to compare them across
/// runs. The migration is additive and never destructive.
pub const MIGRATION_0002_SNAPSHOT_GIT: &str = r#"
ALTER TABLE check_snapshots ADD COLUMN git_commit TEXT;
ALTER TABLE check_snapshots ADD COLUMN git_branch TEXT;
"#;
