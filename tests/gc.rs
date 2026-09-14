//! End-to-end tests of `driftwatch gc`.

#![cfg(unix)]

use assert_cmd::Command;
use rusqlite::Connection;
use tempfile::tempdir;

fn driftwatch() -> Command {
    Command::cargo_bin("driftwatchdog").expect("compiled driftwatch binary")
}

fn init_dir() -> tempfile::TempDir {
    let tmp = tempdir().unwrap();
    driftwatch()
        .arg("init")
        .current_dir(tmp.path())
        .assert()
        .success();
    tmp
}

fn open_db(tmp: &tempfile::TempDir) -> Connection {
    Connection::open(tmp.path().join(".driftwatch/state.db")).expect("open db")
}

#[test]
fn gc_dry_run_reports_zero_when_no_old_runs() {
    let tmp = init_dir();
    driftwatch()
        .args(["run", "sh", "-c", "echo boom >&2; exit 1"])
        .current_dir(tmp.path())
        .assert()
        .failure();
    let out = driftwatch()
        .args(["gc", "--dry-run"])
        .current_dir(tmp.path())
        .assert()
        .success();
    let stdout = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    assert!(stdout.contains("driftwatch gc --dry-run"));
    assert!(stdout.contains("0 runs"));
}

#[test]
fn gc_dry_run_does_not_modify() {
    let tmp = init_dir();
    driftwatch()
        .args(["run", "sh", "-c", "echo boom >&2; exit 1"])
        .current_dir(tmp.path())
        .assert()
        .failure();
    // The boom is in stderr (not stdout), so check stderr_excerpt.
    let conn_before: String = open_db(&tmp)
        .query_row(
            "SELECT stderr_excerpt FROM runs WHERE stderr_excerpt IS NOT NULL",
            [],
            |r| r.get(0),
        )
        .unwrap();
    driftwatch()
        .args(["gc", "--dry-run"])
        .current_dir(tmp.path())
        .assert()
        .success();
    let conn_after: String = open_db(&tmp)
        .query_row(
            "SELECT stderr_excerpt FROM runs WHERE stderr_excerpt IS NOT NULL",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(conn_before, conn_after);
}

#[test]
fn gc_default_days_is_90() {
    let tmp = init_dir();
    let conn = open_db(&tmp);
    conn.execute(
        "INSERT INTO runs (started_at, program, argv, cwd, status, stdout_excerpt)
         VALUES ('2020-01-01T00:00:00Z', 'old', '[]', '/tmp', 'failed', 'ancient boom')",
        [],
    )
    .unwrap();
    drop(conn);
    let out = driftwatch()
        .arg("gc")
        .current_dir(tmp.path())
        .assert()
        .success();
    let stdout = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    assert!(stdout.contains("pruned 1 runs"));
    let conn = open_db(&tmp);
    let excerpt: Option<String> = conn
        .query_row(
            "SELECT stdout_excerpt FROM runs WHERE program = 'old'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(excerpt.is_none());
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM runs WHERE program = 'old'", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(count, 1);
}

#[test]
fn gc_repeated_is_idempotent() {
    let tmp = init_dir();
    let conn = open_db(&tmp);
    conn.execute(
        "INSERT INTO runs (started_at, program, argv, cwd, status, stdout_excerpt)
         VALUES ('2020-01-01T00:00:00Z', 'old', '[]', '/tmp', 'failed', 'ancient boom')",
        [],
    )
    .unwrap();
    drop(conn);
    let out1 = driftwatch()
        .arg("gc")
        .current_dir(tmp.path())
        .assert()
        .success();
    let stdout1 = String::from_utf8(out1.get_output().stdout.clone()).unwrap();
    assert!(stdout1.contains("pruned 1 runs"));
    let out2 = driftwatch()
        .arg("gc")
        .current_dir(tmp.path())
        .assert()
        .success();
    let stdout2 = String::from_utf8(out2.get_output().stdout.clone()).unwrap();
    assert!(stdout2.contains("pruned 0 runs"));
}

#[test]
fn gc_recent_runs_untouched() {
    let tmp = init_dir();
    driftwatch()
        .args(["run", "sh", "-c", "echo boom >&2; exit 1"])
        .current_dir(tmp.path())
        .assert()
        .failure();
    let out = driftwatch()
        .arg("gc")
        .current_dir(tmp.path())
        .assert()
        .success();
    let stdout = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    assert!(stdout.contains("pruned 0 runs"));
    let conn = open_db(&tmp);
    // stderr_excerpt is the stream populated by `echo boom >&2`.
    let excerpt: Option<String> = conn
        .query_row(
            "SELECT stderr_excerpt FROM runs WHERE stderr_excerpt IS NOT NULL",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(excerpt.is_some());
}

#[test]
fn gc_prunes_artifact_files_but_keeps_identity_rows() {
    let tmp = init_dir();
    let artifact_dir = tmp.path().join(".driftwatch/artifacts");
    std::fs::create_dir_all(&artifact_dir).unwrap();
    std::fs::write(artifact_dir.join("old-out.log"), b"old bytes").unwrap();
    let conn = open_db(&tmp);
    conn.execute(
        "INSERT INTO gate_artifacts
             (key, kind, producer, created_at, byte_size, digest,
              rel_path, redacted, available, preview)
         VALUES ('old/out', 'command_output', 'probe', '2020-01-01T00:00:00Z',
                 9, 'sha256:abc', 'artifacts/old-out.log', 1, 1, 'old bytes')",
        [],
    )
    .unwrap();
    drop(conn);
    let out = driftwatch()
        .arg("gc")
        .current_dir(tmp.path())
        .assert()
        .success();
    let stdout = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    assert!(stdout.contains("1 artifacts"));
    assert!(!artifact_dir.join("old-out.log").exists());
    let conn = open_db(&tmp);
    // Identity row survives: key, producer, digest retained, unavailable.
    let (producer, digest, available): (String, String, i64) = conn
        .query_row(
            "SELECT producer, digest, available FROM gate_artifacts WHERE key = 'old/out'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!(producer, "probe");
    assert_eq!(digest, "sha256:abc");
    assert_eq!(available, 0);
}

#[test]
fn gc_preserves_fingerprint_history() {
    // After GC, the fingerprint row and its occurrence rows remain;
    // only the bulky excerpts on the underlying run are gone.
    let tmp = init_dir();
    let conn = open_db(&tmp);
    conn.execute(
        "INSERT INTO runs (started_at, program, argv, cwd, status, stdout_excerpt, stderr_excerpt)
         VALUES ('2020-01-01T00:00:00Z', 'old', '[]', '/tmp', 'failed', 'ancient boom', 'old err')",
        [],
    )
    .unwrap();
    let run_id: i64 = conn
        .query_row("SELECT id FROM runs WHERE program = 'old'", [], |r| {
            r.get(0)
        })
        .unwrap();
    conn.execute(
        "INSERT INTO fingerprints (hash, canonical, summary, first_seen_at, last_seen_at, occurrence_count)
         VALUES ('2222222222222222222222222222222222222222222222222222222222222222', 'old boom', 'old boom', '2020-01-01T00:00:00Z', '2020-01-01T00:00:00Z', 1)",
        [],
    )
    .unwrap();
    let fp_id: i64 = conn
        .query_row(
            "SELECT id FROM fingerprints WHERE hash = '2222222222222222222222222222222222222222222222222222222222222222'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    conn.execute(
        "INSERT INTO occurrences (fingerprint_id, run_id, seen_at, excerpt)
         VALUES (?1, ?2, '2020-01-01T00:00:00Z', 'excerpt text')",
        [fp_id, run_id],
    )
    .unwrap();
    drop(conn);
    driftwatch()
        .arg("gc")
        .current_dir(tmp.path())
        .assert()
        .success();
    let conn = open_db(&tmp);
    let fp_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM fingerprints", [], |r| r.get(0))
        .unwrap();
    let occ_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM occurrences", [], |r| r.get(0))
        .unwrap();
    assert_eq!(fp_count, 1);
    assert_eq!(occ_count, 1);
    let excerpts_null: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM runs WHERE id = ?1 AND stdout_excerpt IS NULL AND stderr_excerpt IS NULL",
            [run_id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(excerpts_null, 1);
}
