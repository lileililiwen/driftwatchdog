//! End-to-end tests of `driftwatch export`.

#![cfg(unix)]

use assert_cmd::Command;
use predicates::str::contains;
use tempfile::tempdir;
// tempdir is used inside init_dir; suppress the unused warning when no
// other test references it directly.

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

#[test]
fn export_json_on_empty_db_produces_valid_document() {
    let tmp = init_dir();
    let out = driftwatch()
        .args(["export", "json"])
        .current_dir(tmp.path())
        .assert()
        .success();
    let stdout = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    let v: serde_json::Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("stdout was not valid JSON: {e}; stdout={stdout}"));
    assert_eq!(v["schema_version"], 1);
    assert!(v["runs"].as_array().unwrap().is_empty());
    assert!(v["fingerprints"].as_array().unwrap().is_empty());
    assert!(v["occurrences"].as_array().unwrap().is_empty());
    assert!(v["alerts"].as_array().unwrap().is_empty());
    assert!(v["correlations"].as_array().unwrap().is_empty());
    assert!(v["manual_links"].as_array().unwrap().is_empty());
    assert_eq!(v["project"]["local_schema_version"], 1);
}

#[test]
fn export_json_after_failing_run_includes_run_fingerprint_occurrence() {
    let tmp = init_dir();
    driftwatch()
        .args(["run", "sh", "-c", "echo boom_export >&2; exit 1"])
        .current_dir(tmp.path())
        .assert()
        .failure();
    let out = driftwatch()
        .args(["export", "json"])
        .current_dir(tmp.path())
        .assert()
        .success();
    let stdout = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["runs"].as_array().unwrap().len(), 1);
    assert_eq!(v["fingerprints"].as_array().unwrap().len(), 1);
    assert_eq!(v["occurrences"].as_array().unwrap().len(), 1);
    // The program and argv are split: program holds the executable,
    // argv holds the rest. Assert both sides.
    assert_eq!(v["runs"][0]["program"], "sh");
    let argv = v["runs"][0]["argv"].as_array().unwrap();
    assert!(argv.iter().any(|a| a == "-c"));
}

#[test]
fn export_jsonl_emits_one_record_per_line_with_type_and_record_id() {
    let tmp = init_dir();
    driftwatch()
        .args(["run", "sh", "-c", "echo boom_jsonl >&2; exit 1"])
        .current_dir(tmp.path())
        .assert()
        .failure();
    let out = driftwatch()
        .args(["export", "jsonl"])
        .current_dir(tmp.path())
        .assert()
        .success();
    let stdout = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    let lines: Vec<&str> = stdout.lines().collect();
    assert!(lines.len() >= 3, "got {} lines", lines.len());
    let parsed: Vec<serde_json::Value> = lines
        .iter()
        .map(|l| serde_json::from_str(l).expect("every line must be valid JSON"))
        .collect();
    let kinds: Vec<&str> = parsed.iter().map(|v| v["type"].as_str().unwrap()).collect();
    assert!(kinds.contains(&"project"));
    assert!(kinds.contains(&"run"));
    assert!(kinds.contains(&"fingerprint"));
    assert!(kinds.contains(&"occurrence"));
    for v in &parsed {
        assert!(v["record_id"].is_string(), "missing record_id on {v}");
    }
}

#[test]
fn export_markdown_renders_sections_and_run_command() {
    let tmp = init_dir();
    driftwatch()
        .args(["run", "echo", "hello_md"])
        .current_dir(tmp.path())
        .assert()
        .success();
    let out = driftwatch()
        .args(["export", "markdown"])
        .current_dir(tmp.path())
        .assert()
        .success();
    let stdout = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    assert!(stdout.contains("# Driftwatch export"));
    assert!(stdout.contains("## Project"));
    assert!(stdout.contains("## Runs (1)"));
    assert!(stdout.contains("echo hello_md"));
    // Empty sections are still emitted with a clear marker.
    assert!(stdout.contains("## Fingerprints (0)"));
    assert!(stdout.contains("_no records_"));
}

#[test]
fn export_unknown_format_is_rejected() {
    let tmp = init_dir();
    driftwatch()
        .args(["export", "xml"])
        .current_dir(tmp.path())
        .assert()
        .failure();
}

#[test]
fn export_round_trip_is_stable() {
    // Same DB exported twice yields byte-identical JSON (timestamps
    // aside; we compare structural equality instead).
    let tmp = init_dir();
    driftwatch()
        .args(["run", "sh", "-c", "echo stable_round_trip >&2; exit 1"])
        .current_dir(tmp.path())
        .assert()
        .failure();
    let out1 = driftwatch()
        .args(["export", "json"])
        .current_dir(tmp.path())
        .assert()
        .success();
    let out2 = driftwatch()
        .args(["export", "json"])
        .current_dir(tmp.path())
        .assert()
        .success();
    let v1: serde_json::Value =
        serde_json::from_slice(out1.get_output().stdout.as_slice()).unwrap();
    let v2: serde_json::Value =
        serde_json::from_slice(out2.get_output().stdout.as_slice()).unwrap();
    // Exported-at differs between the two calls; everything else must match.
    assert_ne!(v1["exported_at"], v2["exported_at"]);
    assert_eq!(v1["runs"], v2["runs"]);
    assert_eq!(v1["fingerprints"], v2["fingerprints"]);
    assert_eq!(v1["occurrences"], v2["occurrences"]);
}

#[test]
fn export_help_works() {
    // Sanity: the export subcommand is registered with clap.
    driftwatch()
        .args(["export", "--help"])
        .assert()
        .success()
        .stdout(contains("FORMAT"));
}
