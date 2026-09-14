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

/// Migration: extend `correlations` with the per-component similarity
/// scores and the algorithm version. The `correlation-and-ai-context`
/// change requires the correlator to persist message/symbol/file/tag
/// scores independently (so future re-tuning can recompute without
/// losing the breakdown) and to tag every row with the algorithm
/// version that produced it. The migration is purely additive: existing
/// rows gain `NULL` for the new score columns and the unchanged
/// aggregate `score` column remains the authoritative total.
pub const MIGRATION_0003_CORRELATION_DETAIL: &str = r#"
ALTER TABLE correlations ADD COLUMN score_message REAL;
ALTER TABLE correlations ADD COLUMN score_symbol REAL;
ALTER TABLE correlations ADD COLUMN score_file REAL;
ALTER TABLE correlations ADD COLUMN score_tag REAL;
ALTER TABLE correlations ADD COLUMN algorithm_version TEXT NOT NULL DEFAULT 'v1';
"#;

/// Migration: harden `manual_links` identity. The
/// `identity-resolution` change requires `UNIQUE(fingerprint_id,
/// alert_id)` so duplicate bug+alert pairs are rejected idempotently
/// at the DB layer, plus best-effort dedupe of pre-existing
/// duplicate pairs (keep the lowest `id`).
///
/// Note on the `CHECK`: the baseline uses OR (at least one endpoint
/// set), which is correct because `driftwatch link` always sets
/// *both* `fingerprint_id` and `alert_id`. An XOR check would reject
/// every real link row, so the OR semantics are preserved here; the
/// safety gain comes from the new `UNIQUE` pair constraint plus the
/// application-level duplicate guard in `Links::create`.
/// Both statements are rerunnable (`IF NOT EXISTS` / idempotent
/// delete).
pub const MIGRATION_0004_LINK_IDENTITY: &str = r#"
DELETE FROM manual_links WHERE id NOT IN (
    SELECT MIN(id) FROM manual_links GROUP BY fingerprint_id, alert_id
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_manual_links_pair
    ON manual_links(fingerprint_id, alert_id);
"#;

/// Migration: gate evidence identity table. The
/// `evidence-and-artifacts` change persists one row per artifact with
/// key, kind, producer identity, size, digest, confined relative path,
/// redaction status, availability, and a redacted bounded preview.
/// Cleanup flips `available` to 0 and clears `rel_path`/`preview` but
/// never deletes the row, so references stay auditable after bulky
/// content is removed. Purely additive; rerunnable via
/// `IF NOT EXISTS`.
pub const MIGRATION_0005_GATE_EVIDENCE: &str = r#"
CREATE TABLE IF NOT EXISTS gate_artifacts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    key TEXT NOT NULL UNIQUE,
    kind TEXT NOT NULL,
    producer TEXT NOT NULL,
    producer_version TEXT,
    created_at TEXT NOT NULL,
    byte_size INTEGER NOT NULL,
    digest TEXT NOT NULL,
    media_type TEXT,
    rel_path TEXT,
    redacted INTEGER NOT NULL DEFAULT 0,
    available INTEGER NOT NULL DEFAULT 1,
    preview TEXT
);
CREATE INDEX IF NOT EXISTS idx_gate_artifacts_created
    ON gate_artifacts(created_at);
"#;
