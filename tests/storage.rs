//! Schema, migration, and foreign-key checks. The spec mandates that
//! "opening the database does not recreate or delete records" — this file
//! asserts that round-trip plus the post-init invariants.

use driftwatchdog::repo::runs::{RunStatus, Runs};
use driftwatchdog::repo::Db;
use driftwatchdog::storage;
use rusqlite::Connection;
use tempfile::tempdir;

#[test]
fn schema_contains_all_foundation_tables() {
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
        assert!(
            tables.iter().any(|t| t == required),
            "missing foundation table {required}; found {tables:?}"
        );
    }
}

#[test]
fn migration_version_is_three_after_init() {
    let tmp = tempdir().unwrap();
    let path = tmp.path().join("state.db");
    let mut conn = storage::open(&path).unwrap();
    let v = storage::migrations::apply(&mut conn).unwrap();
    assert_eq!(v, 4);
    let recorded: i64 = conn
        .query_row("SELECT MAX(version) FROM schema_version", [], |r| r.get(0))
        .unwrap();
    assert_eq!(recorded, 4);
}

#[test]
fn migration_0003_adds_correlation_component_columns() {
    let db = Db::open_in_memory().unwrap();
    // The new columns must be present and queryable.
    for required in [
        "score_message",
        "score_symbol",
        "score_file",
        "score_tag",
        "algorithm_version",
    ] {
        let present: i64 = db
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('correlations') WHERE name = ?1",
                rusqlite::params![required],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(present, 1, "missing column {required}");
    }
}

#[test]
fn foreign_keys_are_enforced() {
    let tmp = tempdir().unwrap();
    let conn = storage::open(&tmp.path().join("state.db")).unwrap();
    let fk: i64 = conn
        .query_row("PRAGMA foreign_keys", [], |r| r.get(0))
        .unwrap();
    assert_eq!(fk, 1);
}

#[test]
fn reopening_db_does_not_drop_rows() {
    let tmp = tempdir().unwrap();
    let path = tmp.path().join("state.db");

    let db = Db::open(&path).unwrap();
    Runs::new(&db)
        .reserve("2026-01-01T00:00:00Z", "echo", "[\"hi\"]", "/tmp")
        .unwrap();

    // Reopen and verify the row still exists.
    let db2 = Db::open(&path).unwrap();
    let rec = Runs::new(&db2).find(1).unwrap().unwrap();
    assert_eq!(rec.program, "echo");
    assert_eq!(rec.status, RunStatus::Running);
}

#[test]
fn manual_links_require_at_least_one_target() {
    // Schema-level constraint: a manual_link row must point at a fingerprint
    // or an alert (or both). Inserting an empty row must fail.
    let db = Db::open_in_memory().unwrap();
    let conn: &Connection = db.conn();
    let err = conn
        .execute(
            "INSERT INTO manual_links (note, created_at) VALUES (?1, ?2)",
            rusqlite::params!["orphan", "2026-01-01T00:00:00Z"],
        )
        .unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.to_lowercase().contains("check"),
        "expected CHECK constraint failure, got: {msg}"
    );
}
