//! End-to-end tests of `driftwatch show <bug-id>`.

#![cfg(unix)]

use assert_cmd::Command;
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

fn run_failing(tmp: &tempfile::TempDir, msg: &str) {
    driftwatch()
        .args(["run", "sh", "-c", &format!("echo {msg} >&2; exit 1")])
        .current_dir(tmp.path())
        .assert()
        .failure();
}

#[test]
fn show_by_short_hash_prefix() {
    let tmp = init_dir();
    run_failing(&tmp, "boom_show");
    let out = driftwatch()
        .arg("show")
        .arg("--help")
        .current_dir(tmp.path())
        .assert()
        .success();
    let _ = out; // touch clap help to make sure the command is registered

    // Find the first 8 hex chars of any fingerprint in the DB.
    let db_path = tmp.path().join(".driftwatch/state.db");
    let conn = rusqlite::Connection::open(&db_path).unwrap();
    let hash: String = conn
        .query_row("SELECT hash FROM fingerprints LIMIT 1", [], |r| r.get(0))
        .unwrap();
    let prefix = &hash[..8];
    let out = driftwatch()
        .args(["show", prefix])
        .current_dir(tmp.path())
        .assert()
        .success();
    let stdout = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    assert!(stdout.contains("Bug  :"), "missing Bug: line: {stdout}");
    assert!(stdout.contains(prefix), "missing hash prefix: {stdout}");
    assert!(stdout.contains("Occurrences"));
    assert!(stdout.contains("boom_show"));
}

#[test]
fn show_by_numeric_id() {
    let tmp = init_dir();
    run_failing(&tmp, "boom_id");
    let out = driftwatch()
        .args(["show", "1"])
        .current_dir(tmp.path())
        .assert()
        .success();
    let stdout = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    assert!(stdout.contains("ID   : #1"), "missing numeric id: {stdout}");
    assert!(stdout.contains("boom_id"));
}

#[test]
fn show_unknown_id_exits_nonzero() {
    let tmp = init_dir();
    run_failing(&tmp, "boom_unknown");
    driftwatch()
        .args(["show", "deadbeef"])
        .current_dir(tmp.path())
        .assert()
        .failure();
}

#[test]
fn show_ambiguous_short_prefix_exits_nonzero() {
    let tmp = init_dir();
    run_failing(&tmp, "alpha_ambig");
    run_failing(&tmp, "alpha_ambig"); // same canonical → same hash
                                      // The above produces one fingerprint, not two. Insert a second
                                      // distinct fingerprint by writing through the DB so the test is
                                      // deterministic: we don't rely on the SHA-256 producing two
                                      // hashes with the same first hex char (it does for some inputs,
                                      // but not deterministically).
    let db_path = tmp.path().join(".driftwatch/state.db");
    let conn = rusqlite::Connection::open(&db_path).unwrap();
    conn.execute(
        "INSERT INTO fingerprints (hash, canonical, summary, first_seen_at, last_seen_at, occurrence_count)
         VALUES ('0abcdef00000000000000000000000000000000000000000000000000000000', 'other', 'other', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z', 1)",
        [],
    )
    .unwrap();
    // Now the 1-char prefix "0" matches the synthetic row, and any
    // other row that happens to start with 0. The first char of any
    // SHA-256 hex is one of [0-9a-f]; if no real row starts with 0,
    // we still have one match. We just need to assert that "0" alone
    // is ambiguous when 2+ rows start with 0. Insert another:
    conn.execute(
        "INSERT INTO fingerprints (hash, canonical, summary, first_seen_at, last_seen_at, occurrence_count)
         VALUES ('0a00000000000000000000000000000000000000000000000000000000000000', 'yet another', 'other', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z', 1)",
        [],
    )
    .unwrap();
    driftwatch()
        .args(["show", "0"])
        .current_dir(tmp.path())
        .assert()
        .failure();
}
