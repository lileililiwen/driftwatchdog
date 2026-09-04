//! End-to-end tests of `driftwatch link`, `driftwatch unlink`, and
//! the `report --ai` AI-context report. The tests run the
//! compiled binary against a temp directory and assert on the
//! stdout/stderr output and the persisted database state.

#![cfg(unix)]

use assert_cmd::Command;
use driftwatch::repo::{alerts::Alerts, bugs::Bugs, correlations::Correlations, links::Links, Db};
use rusqlite::Connection;
use tempfile::tempdir;

fn driftwatch() -> Command {
    Command::cargo_bin("driftwatch").expect("compiled driftwatch binary")
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

fn insert_fingerprint_and_alert(db_path: &std::path::Path) -> (i64, i64) {
    // Insert the fingerprint row first.
    let mut db = Db::open(db_path).unwrap();
    let fp = Bugs::new(&mut db)
        .upsert_for_occurrence("err", "err", "2026-01-01T00:00:00Z")
        .unwrap();
    // Then record the alert snapshot (separate `&mut db` borrow).
    Alerts::record_run(
        &mut db,
        &driftwatch::repo::alerts::NewSnapshot {
            taken_at: "2026-01-01T00:00:00Z",
            checker_name: "spec",
            status: "success",
            diagnostic: None,
            raw_json: None,
            git_commit: None,
            git_branch: None,
        },
        &[driftwatch::repo::alerts::NewAlert {
            severity: "warning",
            message: "DbPool: timeout exceeded",
            source: Some("specs/db.md"),
            symbol: Some("DbPool"),
        }],
    )
    .unwrap();
    let alert_id: i64 = db
        .conn()
        .query_row("SELECT id FROM drift_alerts LIMIT 1", [], |r| r.get(0))
        .unwrap();
    (fp.id, alert_id)
}

#[test]
fn link_persists_pair_and_lists_it_back() {
    let tmp = init_dir();
    let db_path = tmp.path().join(".driftwatch/state.db");
    let (fp_id, alert_id) = insert_fingerprint_and_alert(&db_path);

    driftwatch()
        .args(["link", &format!("bug:{fp_id}"), &format!("spec:{alert_id}")])
        .current_dir(tmp.path())
        .assert()
        .success();

    let conn = open_db(&tmp);
    let n: i64 = conn
        .query_row("SELECT COUNT(*) FROM manual_links", [], |r| r.get(0))
        .unwrap();
    assert_eq!(n, 1);
    let (got_fp, got_alert): (Option<i64>, Option<i64>) = conn
        .query_row(
            "SELECT fingerprint_id, alert_id FROM manual_links LIMIT 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(got_fp, Some(fp_id));
    assert_eq!(got_alert, Some(alert_id));
}

#[test]
fn link_with_note_stores_note() {
    let tmp = init_dir();
    let db_path = tmp.path().join(".driftwatch/state.db");
    let (fp_id, alert_id) = insert_fingerprint_and_alert(&db_path);

    driftwatch()
        .args([
            "link",
            &format!("bug:{fp_id}"),
            &format!("spec:{alert_id}"),
            "--note",
            "see issue #108",
        ])
        .current_dir(tmp.path())
        .assert()
        .success();

    let conn = open_db(&tmp);
    let note: String = conn
        .query_row("SELECT note FROM manual_links LIMIT 1", [], |r| r.get(0))
        .unwrap();
    assert_eq!(note, "see issue #108");
}

#[test]
fn link_bug_side_accepts_bare_hash_prefix() {
    let tmp = init_dir();
    let db_path = tmp.path().join(".driftwatch/state.db");
    let (fp_id, alert_id) = insert_fingerprint_and_alert(&db_path);
    // Read the actual hash so we can take an 8-char prefix.
    let conn = open_db(&tmp);
    let hash: String = conn
        .query_row(
            "SELECT hash FROM fingerprints WHERE id = ?1",
            rusqlite::params![fp_id],
            |r| r.get(0),
        )
        .unwrap();
    let prefix = &hash[..8];

    driftwatch()
        .args(["link", prefix, &format!("spec:{alert_id}")])
        .current_dir(tmp.path())
        .assert()
        .success();
    let n: i64 = conn
        .query_row("SELECT COUNT(*) FROM manual_links", [], |r| r.get(0))
        .unwrap();
    assert_eq!(n, 1);
}

#[test]
fn link_unknown_bug_errors() {
    let tmp = init_dir();
    driftwatch()
        .args(["link", "bug:9999", "spec:1"])
        .current_dir(tmp.path())
        .assert()
        .failure();
}

#[test]
fn link_unknown_alert_errors() {
    let tmp = init_dir();
    let db_path = tmp.path().join(".driftwatch/state.db");
    let (fp_id, _alert_id) = insert_fingerprint_and_alert(&db_path);
    driftwatch()
        .args(["link", &format!("bug:{fp_id}"), "spec:9999"])
        .current_dir(tmp.path())
        .assert()
        .failure();
}

#[test]
fn unlink_removes_only_targeted_row() {
    let tmp = init_dir();
    let db_path = tmp.path().join(".driftwatch/state.db");
    let (fp_id, alert_id) = insert_fingerprint_and_alert(&db_path);
    let mut db = Db::open(&db_path).unwrap();
    let link_a = Links::create(
        &mut db,
        Some(fp_id),
        Some(alert_id),
        Some("a"),
        "2026-01-01T00:00:00Z",
    )
    .unwrap();
    let link_b = Links::create(
        &mut db,
        Some(fp_id),
        Some(alert_id),
        Some("b"),
        "2026-01-01T00:00:00Z",
    )
    .unwrap();

    driftwatch()
        .args(["unlink", &link_a.id.to_string()])
        .current_dir(tmp.path())
        .assert()
        .success();

    // Only link_a is gone; link_b is still there.
    let conn = open_db(&tmp);
    let remaining: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM manual_links WHERE id = ?1",
            rusqlite::params![link_b.id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(remaining, 1);
    let note_b: String = conn
        .query_row(
            "SELECT note FROM manual_links WHERE id = ?1",
            rusqlite::params![link_b.id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(note_b, "b");
}

#[test]
fn unlink_missing_id_errors() {
    let tmp = init_dir();
    driftwatch()
        .args(["unlink", "9999"])
        .current_dir(tmp.path())
        .assert()
        .failure();
}

#[test]
fn report_ai_contains_required_sections_with_recurring_failure() {
    let tmp = init_dir();
    let db_path = tmp.path().join(".driftwatch/state.db");
    let _ = insert_fingerprint_and_alert(&db_path);
    let out = driftwatch()
        .args(["report", "--ai"])
        .current_dir(tmp.path())
        .assert()
        .success();
    let stdout = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    for section in [
        "# Driftwatch AI context",
        "## Project context",
        "## Recurring failures",
        "## Current spec violations",
        "## Possible relationships",
        "## Investigation task",
    ] {
        assert!(stdout.contains(section), "missing {section} in:\n{stdout}");
    }
}

#[test]
fn report_ai_includes_possible_relationships_when_heuristic_passes_threshold() {
    let tmp = init_dir();
    let db_path = tmp.path().join(".driftwatch/state.db");
    let (fp_id, alert_id) = insert_fingerprint_and_alert(&db_path);
    // Persist a heuristic correlation directly so we do not
    // depend on `driftwatch check` populating it.
    let db = Db::open(&db_path).unwrap();
    Correlations::new(&db)
        .upsert(
            fp_id,
            alert_id,
            driftwatch::similarity::score::ComponentScores {
                message: 1.0,
                symbol: 1.0,
                file: 1.0,
                tag: 0.0,
                total: 0.9,
            },
            "2026-01-01T00:00:00Z",
        )
        .unwrap();
    let out = driftwatch()
        .args(["report", "--ai"])
        .current_dir(tmp.path())
        .assert()
        .success();
    let stdout = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    assert!(stdout.contains("| heuristic |"), "missing heuristic row");
    assert!(
        stdout.contains("Heuristic correlations are leads"),
        "missing cautious label"
    );
}

#[test]
fn report_ai_empty_state_omits_relationships_table_and_includes_investigation_task() {
    let tmp = init_dir();
    let out = driftwatch()
        .args(["report", "--ai"])
        .current_dir(tmp.path())
        .assert()
        .success();
    let stdout = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    assert!(stdout.contains("_No possible relationships were recorded._"));
    assert!(stdout.contains("## Investigation task"));
}
