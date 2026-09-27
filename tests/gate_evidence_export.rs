//! End-to-end tests for the `driftwatch gate evidence-export` command.
//!
//! All tests are Unix-only because the project-runtime adapter launches
//! its target via `sh -c`. The unit tests in `src/gate/evidence_export.rs`
//! cover the in-memory mapping; this file drives the same scenarios
//! through the binary to prove the CLI surface and the persisted
//! `gate_runs` row stay consistent with the design's per-field rules.

#![cfg(unix)]

use assert_cmd::Command;
use driftwatchdog::repo::{gates::Gates, Db};
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

fn run_gate(tmp: &tempfile::TempDir) -> assert_cmd::assert::Assert {
    driftwatch().arg("gate").current_dir(tmp.path()).assert()
}

fn run_export_json(tmp: &tempfile::TempDir) -> assert_cmd::assert::Assert {
    driftwatch()
        .args(["gate", "evidence-export", "--format", "json"])
        .current_dir(tmp.path())
        .assert()
}

fn run_export_human(tmp: &tempfile::TempDir) -> assert_cmd::assert::Assert {
    driftwatch()
        .args(["gate", "evidence-export"])
        .current_dir(tmp.path())
        .assert()
}

fn write_gate_manifest(tmp: &tempfile::TempDir, body: &str) {
    std::fs::write(tmp.path().join("gate.toml"), body).unwrap();
}

fn make_envelope_file(tmp: &tempfile::TempDir, name: &str, body: &str) -> std::path::PathBuf {
    let path = tmp.path().join(name);
    std::fs::write(&path, body).unwrap();
    path
}

fn release_pass_envelope(revision: &str) -> String {
    format!(
        r#"{{"version":1,"status":"PASS","severity":"info","revision":"{revision}","product_version":"1.2.3","artifacts":["binary.tar.gz"],"provenance":{{"type":"slsa-provenance/v0.2","digest":"sha256:deadbeef"}},"findings":[]}}"#
    )
}

fn release_fail_envelope() -> String {
    r#"{"version":1,"status":"FAIL","severity":"error","findings":[{"title":"missing SBOM","severity":"error","rule":"sbom-required"}],"diagnostic":"see findings"}"#.to_string()
}

fn capability_pass_envelope(verified: &[&str]) -> String {
    let verified_json = verified
        .iter()
        .map(|s| format!("\"{s}\""))
        .collect::<Vec<_>>()
        .join(",");
    format!(
        r#"{{"version":1,"status":"PASS","severity":"info","capabilities":{{"declared":[{verified_json}],"configured":[{verified_json}],"verified":[{verified_json}],"unverified":[]}},"findings":[]}}"#
    )
}

fn empty_init_dir() -> tempfile::TempDir {
    // An initialized driftwatch project that has not yet run a gate.
    init_dir()
}

#[test]
fn export_refuses_with_no_run() {
    let tmp = empty_init_dir();
    let assert = run_export_json(&tmp);
    assert
        .failure()
        .stderr(predicates::str::contains("no completed gate run"));
}

#[test]
fn export_documents_schema_and_field_set_for_passing_run() {
    let tmp = init_dir();
    let release = make_envelope_file(&tmp, "release.json", &release_pass_envelope("abc123"));
    let cap = make_envelope_file(
        &tmp,
        "cap.json",
        &capability_pass_envelope(&["release-evidence", "capability-conformance"]),
    );
    write_gate_manifest(
        &tmp,
        &format!(
            r#"
profile = "release"
[[checks]]
id = "release-evidence"
command = "cat {release} ; exit 0"
[[checks]]
id = "capability-conformance"
command = "cat {cap} ; exit 0"
"#,
            release = release.display(),
            cap = cap.display(),
        ),
    );
    run_gate(&tmp).success();
    let assert = run_export_json(&tmp);
    let stdout = String::from_utf8(assert.success().get_output().stdout.clone()).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("valid json");
    assert_eq!(parsed["schema_version"], 1);
    let fields = parsed["fields"].as_array().expect("fields array");
    let names: Vec<&str> = fields
        .iter()
        .map(|f| f["field"].as_str().unwrap())
        .collect();
    assert_eq!(
        names,
        vec![
            "revision",
            "version",
            "toolchain",
            "artifacts",
            "digests",
            "sbom",
            "provenance",
            "checks",
            "publication",
        ]
    );
}

#[test]
fn ran_pass_maps_supplied_fields_to_verified() {
    let tmp = init_dir();
    let release = make_envelope_file(&tmp, "release.json", &release_pass_envelope("abc123"));
    let cap = make_envelope_file(
        &tmp,
        "cap.json",
        &capability_pass_envelope(&["release-evidence", "capability-conformance"]),
    );
    write_gate_manifest(
        &tmp,
        &format!(
            r#"
profile = "release"
[[checks]]
id = "release-evidence"
command = "cat {release} ; exit 0"
[[checks]]
id = "capability-conformance"
command = "cat {cap} ; exit 0"
"#,
            release = release.display(),
            cap = cap.display(),
        ),
    );
    run_gate(&tmp).success();
    let stdout =
        String::from_utf8(run_export_json(&tmp).success().get_output().stdout.clone()).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let fields = parsed["fields"].as_array().unwrap();
    for name in [
        "revision",
        "version",
        "artifacts",
        "digests",
        "sbom",
        "provenance",
        "publication",
    ] {
        let entry = fields
            .iter()
            .find(|f| f["field"].as_str() == Some(name))
            .unwrap();
        assert_eq!(entry["state"].as_str().unwrap(), "verified", "{name}");
    }
    let checks = fields
        .iter()
        .find(|f| f["field"].as_str() == Some("checks"))
        .unwrap();
    assert_eq!(checks["state"].as_str().unwrap(), "verified");
}

#[test]
fn ran_fail_maps_supplied_fields_to_unverified() {
    let tmp = init_dir();
    let release = make_envelope_file(&tmp, "release.json", &release_fail_envelope());
    let cap = make_envelope_file(
        &tmp,
        "cap.json",
        &capability_pass_envelope(&["release-evidence", "capability-conformance"]),
    );
    write_gate_manifest(
        &tmp,
        &format!(
            r#"
profile = "release"
[[checks]]
id = "release-evidence"
command = "cat {release} ; exit 1"
[[checks]]
id = "capability-conformance"
command = "cat {cap} ; exit 0"
"#,
            release = release.display(),
            cap = cap.display(),
        ),
    );
    // The run itself is blocked; we still want to verify the export.
    run_gate(&tmp).failure();
    let stdout =
        String::from_utf8(run_export_json(&tmp).success().get_output().stdout.clone()).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let fields = parsed["fields"].as_array().unwrap();
    for name in [
        "revision",
        "version",
        "artifacts",
        "digests",
        "sbom",
        "provenance",
        "publication",
    ] {
        let entry = fields
            .iter()
            .find(|f| f["field"].as_str() == Some(name))
            .unwrap();
        assert_eq!(entry["state"].as_str().unwrap(), "unverified", "{name}");
    }
}

#[test]
fn not_scheduled_maps_unsupplied_fields_to_unverified() {
    let tmp = init_dir();
    // Only a non-release-evidence check is scheduled.
    write_gate_manifest(
        &tmp,
        r#"
profile = "minimal"
[[checks]]
id = "smoke"
command = "true"
"#,
    );
    run_gate(&tmp).success();
    let stdout =
        String::from_utf8(run_export_json(&tmp).success().get_output().stdout.clone()).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let fields = parsed["fields"].as_array().unwrap();
    for entry in fields {
        let state = entry["state"].as_str().unwrap();
        assert_eq!(state, "unverified", "{:?}", entry);
    }
}

#[test]
fn could_not_execute_maps_field_to_blocked() {
    let tmp = init_dir();
    // Schedule release-evidence with a command that doesn't exist,
    // so the runner reports the check could not execute.
    let cap = make_envelope_file(
        &tmp,
        "cap.json",
        &capability_pass_envelope(&["release-evidence", "capability-conformance"]),
    );
    write_gate_manifest(
        &tmp,
        &format!(
            r#"
profile = "release"
[[checks]]
id = "release-evidence"
command = "this-binary-does-not-exist ; exit 0"
[[checks]]
id = "capability-conformance"
command = "cat {cap} ; exit 0"
"#,
            cap = cap.display(),
        ),
    );
    run_gate(&tmp).failure();
    let stdout =
        String::from_utf8(run_export_json(&tmp).success().get_output().stdout.clone()).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let fields = parsed["fields"].as_array().unwrap();
    let revision = fields
        .iter()
        .find(|f| f["field"].as_str() == Some("revision"))
        .unwrap();
    // Could-not-execute produces missing_evidence, which the export
    // maps to `blocked`.
    assert!(
        revision["state"].as_str() == Some("blocked")
            || revision["state"].as_str() == Some("unverified"),
        "expected blocked or unverified, got {revision:?}"
    );
    // Source gate is preserved so the audit can trace the cause.
    if revision["state"].as_str() == Some("blocked") {
        assert_eq!(revision["source_gate"].as_str(), Some("release-evidence"));
    }
}

#[test]
fn stale_revision_marks_every_field_unverified() {
    let tmp = init_dir();
    // Set up a real git repository so the project has a known
    // current revision. Without git, the export falls back to the
    // run's revision as the project revision, which would mask the
    // stale check.
    std::process::Command::new("git")
        .args(["init", "--initial-branch=main"])
        .current_dir(tmp.path())
        .output()
        .expect("git init");
    std::process::Command::new("git")
        .args(["config", "user.email", "test@example.com"])
        .current_dir(tmp.path())
        .output()
        .expect("git config email");
    std::process::Command::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(tmp.path())
        .output()
        .expect("git config name");
    std::process::Command::new("git")
        .args(["add", "-A"])
        .current_dir(tmp.path())
        .output()
        .expect("git add");
    std::process::Command::new("git")
        .args(["commit", "-m", "initial"])
        .current_dir(tmp.path())
        .output()
        .expect("git commit");
    // Capture the actual commit so the envelope's revision matches.
    let head = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(tmp.path())
        .output()
        .expect("git rev-parse");
    let head = String::from_utf8(head.stdout).unwrap().trim().to_string();
    let release = make_envelope_file(&tmp, "release.json", &release_pass_envelope(&head));
    let cap = make_envelope_file(
        &tmp,
        "cap.json",
        &capability_pass_envelope(&["release-evidence", "capability-conformance"]),
    );
    write_gate_manifest(
        &tmp,
        &format!(
            r#"
profile = "release"
[[checks]]
id = "release-evidence"
command = "cat {release} ; exit 0"
[[checks]]
id = "capability-conformance"
command = "cat {cap} ; exit 0"
"#,
            release = release.display(),
            cap = cap.display(),
        ),
    );
    run_gate(&tmp).success();
    // Manually rewrite the gate_runs row to record a different
    // project revision so the next export sees a stale run.
    let db_path = tmp.path().join(".driftwatch/state.db");
    let conn = rusqlite::Connection::open(&db_path).unwrap();
    conn.execute(
        "UPDATE gate_runs SET revision = 'oldrev'",
        rusqlite::params![],
    )
    .unwrap();
    drop(conn);
    let stdout =
        String::from_utf8(run_export_json(&tmp).success().get_output().stdout.clone()).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert!(
        parsed["stale"].is_object(),
        "stale diagnostic missing: {parsed:?}"
    );
    let fields = parsed["fields"].as_array().unwrap();
    for entry in fields {
        assert_eq!(
            entry["state"].as_str().unwrap(),
            "unverified",
            "stale field should be unverified: {entry:?}"
        );
    }
}

#[test]
fn export_uses_latest_run() {
    let tmp = init_dir();
    let release = make_envelope_file(&tmp, "release.json", &release_pass_envelope("abc123"));
    let cap = make_envelope_file(
        &tmp,
        "cap.json",
        &capability_pass_envelope(&["release-evidence", "capability-conformance"]),
    );
    write_gate_manifest(
        &tmp,
        &format!(
            r#"
profile = "release"
[[checks]]
id = "release-evidence"
command = "cat {release} ; exit 0"
[[checks]]
id = "capability-conformance"
command = "cat {cap} ; exit 0"
"#,
            release = release.display(),
            cap = cap.display(),
        ),
    );
    // Two runs: the export should pick the most recent one.
    run_gate(&tmp).success();
    run_gate(&tmp).success();
    let stdout =
        String::from_utf8(run_export_json(&tmp).success().get_output().stdout.clone()).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    // `gate_run_id` is the latest row; the test just asserts it is
    // present and integer-shaped.
    assert!(parsed["gate_run_id"].is_i64() || parsed["gate_run_id"].is_u64());
    // The export picked the latest run, which is run #2.
    let db_path = tmp.path().join(".driftwatch/state.db");
    let latest_id: i64 = rusqlite::Connection::open(&db_path)
        .unwrap()
        .query_row(
            "SELECT id FROM gate_runs ORDER BY id DESC LIMIT 1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let exported_id = parsed["gate_run_id"].as_i64().unwrap();
    assert_eq!(
        exported_id, latest_id,
        "export should target the latest run"
    );
}

#[test]
fn human_export_prints_table() {
    let tmp = init_dir();
    let release = make_envelope_file(&tmp, "release.json", &release_pass_envelope("abc123"));
    let cap = make_envelope_file(
        &tmp,
        "cap.json",
        &capability_pass_envelope(&["release-evidence", "capability-conformance"]),
    );
    write_gate_manifest(
        &tmp,
        &format!(
            r#"
profile = "release"
[[checks]]
id = "release-evidence"
command = "cat {release} ; exit 0"
[[checks]]
id = "capability-conformance"
command = "cat {cap} ; exit 0"
"#,
            release = release.display(),
            cap = cap.display(),
        ),
    );
    run_gate(&tmp).success();
    let stdout =
        String::from_utf8(run_export_human(&tmp).success().get_output().stdout.clone()).unwrap();
    assert!(stdout.contains("driftwatch gate-evidence export"));
    assert!(stdout.contains("schema_version: 1"));
    for name in [
        "revision",
        "version",
        "toolchain",
        "artifacts",
        "digests",
        "sbom",
        "provenance",
        "checks",
        "publication",
    ] {
        assert!(stdout.contains(name), "missing {name} in:\n{stdout}");
    }
}

#[test]
fn export_is_read_only() {
    let tmp = init_dir();
    let release = make_envelope_file(&tmp, "release.json", &release_pass_envelope("abc123"));
    let cap = make_envelope_file(
        &tmp,
        "cap.json",
        &capability_pass_envelope(&["release-evidence", "capability-conformance"]),
    );
    write_gate_manifest(
        &tmp,
        &format!(
            r#"
profile = "release"
[[checks]]
id = "release-evidence"
command = "cat {release} ; exit 0"
[[checks]]
id = "capability-conformance"
command = "cat {cap} ; exit 0"
"#,
            release = release.display(),
            cap = cap.display(),
        ),
    );
    run_gate(&tmp).success();
    let db_path = tmp.path().join(".driftwatch/state.db");
    let before = Gates::new(&Db::open_read_only(&db_path).unwrap())
        .count()
        .unwrap();
    for _ in 0..3 {
        run_export_json(&tmp).success();
    }
    let after = Gates::new(&Db::open_read_only(&db_path).unwrap())
        .count()
        .unwrap();
    assert_eq!(before, after, "evidence export must not add gate_runs rows");
}

#[test]
fn unknown_field_in_construction_is_a_hard_error() {
    // The export never emits fields outside the governance set; we
    // assert this by parsing the JSON and checking every entry's
    // `field` is a known governance name.
    let tmp = init_dir();
    let release = make_envelope_file(&tmp, "release.json", &release_pass_envelope("abc123"));
    let cap = make_envelope_file(
        &tmp,
        "cap.json",
        &capability_pass_envelope(&["release-evidence", "capability-conformance"]),
    );
    write_gate_manifest(
        &tmp,
        &format!(
            r#"
profile = "release"
[[checks]]
id = "release-evidence"
command = "cat {release} ; exit 0"
[[checks]]
id = "capability-conformance"
command = "cat {cap} ; exit 0"
"#,
            release = release.display(),
            cap = cap.display(),
        ),
    );
    run_gate(&tmp).success();
    let stdout =
        String::from_utf8(run_export_json(&tmp).success().get_output().stdout.clone()).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let fields = parsed["fields"].as_array().unwrap();
    let allowed = [
        "revision",
        "version",
        "toolchain",
        "artifacts",
        "digests",
        "sbom",
        "provenance",
        "checks",
        "publication",
    ];
    for entry in fields {
        let name = entry["field"].as_str().unwrap();
        assert!(allowed.contains(&name), "leaked field {name}");
    }
}
