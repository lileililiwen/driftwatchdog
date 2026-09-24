//! End-to-end tests for the product-quality Gate contract.
//!
//! All tests are Unix-only because the project-runtime adapter launches
//! its target via `sh -c`. The unit tests in `src/gate/` cover the
//! adapter pieces (envelope parsing, mapping rules, exit-code
//! authority).

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

fn latest_run(tmp: &tempfile::TempDir) -> driftwatchdog::repo::gates::GateRunRow {
    let path = tmp.path().join(".driftwatch/state.db");
    let db = Db::open_read_only(&path).expect("read-only db");
    Gates::new(&db)
        .latest()
        .expect("query gate_runs")
        .expect("at least one gate run")
}

fn run_gate(tmp: &tempfile::TempDir) -> assert_cmd::assert::Assert {
    driftwatch().arg("gate").current_dir(tmp.path()).assert()
}

fn write_gate_manifest(tmp: &tempfile::TempDir, body: &str) {
    std::fs::write(tmp.path().join("gate.toml"), body).unwrap();
}

fn make_envelope_file(tmp: &tempfile::TempDir, name: &str, body: &str) -> std::path::PathBuf {
    let path = tmp.path().join(name);
    std::fs::write(&path, body).unwrap();
    path
}

#[test]
fn product_profile_passes_with_passing_envelope() {
    let tmp = init_dir();
    let envelope = make_envelope_file(
        &tmp,
        "boundary.json",
        r#"{"version":1,"status":"PASS","findings":[],"diagnostic":"ok"}"#,
    );
    write_gate_manifest(
        &tmp,
        &format!(
            r#"
profile = "product"
[[checks]]
id = "product-code-boundary"
command = "cat {envelope} ; exit 0"
[[checks]]
id = "placeholder-threshold"
command = "cat {envelope} ; exit 0"
"#,
            envelope = envelope.display()
        ),
    );
    let assert = run_gate(&tmp);
    assert.success();
    let run = latest_run(&tmp);
    assert_eq!(run.status, "PASS");
    assert!(!run.blocked);
    assert!(run.manifest_digest.starts_with("sha256:"));
}

#[test]
fn product_profile_fails_with_failing_envelope() {
    let tmp = init_dir();
    let envelope = make_envelope_file(
        &tmp,
        "boundary.json",
        r#"{"version":1,"status":"FAIL","severity":"error","findings":[{"title":"test in src","severity":"error","location":"src/foo.rs","rule":"no-tests-in-product"}],"diagnostic":"see findings","remediation":"move tests"}"#,
    );
    write_gate_manifest(
        &tmp,
        &format!(
            r#"
profile = "product"
[[checks]]
id = "product-code-boundary"
command = "cat {envelope} ; exit 1"
[[checks]]
id = "placeholder-threshold"
command = "exit 0"
"#,
            envelope = envelope.display()
        ),
    );
    let assert = run_gate(&tmp);
    assert.failure();
    let run = latest_run(&tmp);
    assert_eq!(run.status, "FAIL");
    assert!(run.blocked);
    assert!(run.results_json.contains("test in src"));
    assert!(run.results_json.contains("src/foo.rs"));
}

#[test]
fn product_profile_contradictory_envelope_is_review() {
    // Command exits 1 but envelope claims PASS: REVIEW_REQUIRED.
    let tmp = init_dir();
    let envelope = make_envelope_file(
        &tmp,
        "boundary.json",
        r#"{"version":1,"status":"PASS","findings":[]}"#,
    );
    write_gate_manifest(
        &tmp,
        &format!(
            r#"
profile = "product"
[[checks]]
id = "product-code-boundary"
command = "cat {envelope} ; exit 1"
[[checks]]
id = "placeholder-threshold"
command = "exit 0"
"#,
            envelope = envelope.display()
        ),
    );
    let assert = run_gate(&tmp);
    assert.failure();
    let run = latest_run(&tmp);
    assert!(run.blocked);
    assert!(run.results_json.contains("REVIEW_REQUIRED"));
    assert!(run.results_json.contains("exit code is authoritative"));
}

#[test]
fn product_profile_malformed_envelope_with_output_is_text_pass() {
    // No envelope, exit 0, both commands emit output: text-mode pass
    // with envelope-missing diagnostic. Either side without output
    // would be REVIEW_REQUIRED on its own.
    let tmp = init_dir();
    write_gate_manifest(
        &tmp,
        r#"
profile = "product"
[[checks]]
id = "product-code-boundary"
command = "echo legacy; exit 0"
[[checks]]
id = "placeholder-threshold"
command = "echo legacy-threshold; exit 0"
"#,
    );
    let assert = run_gate(&tmp);
    assert.success();
    let run = latest_run(&tmp);
    assert_eq!(run.status, "PASS");
    assert!(!run.blocked);
    assert!(run
        .results_json
        .contains("no product-quality JSON envelope"));
}

#[test]
fn product_profile_empty_command_output_is_review() {
    // No stdout, no stderr, no envelope: REVIEW_REQUIRED.
    let tmp = init_dir();
    write_gate_manifest(
        &tmp,
        r#"
profile = "product"
[[checks]]
id = "product-code-boundary"
command = "exit 0"
[[checks]]
id = "placeholder-threshold"
command = "exit 0"
"#,
    );
    let assert = run_gate(&tmp);
    assert.failure();
    let run = latest_run(&tmp);
    assert!(run.blocked);
    assert!(run.results_json.contains("REVIEW_REQUIRED"));
    assert!(run.results_json.contains(":output"));
}

#[test]
fn product_profile_required_command_missing_is_review() {
    // No command bound for a required product-quality concern:
    // REVIEW_REQUIRED (aggregate path).
    let tmp = init_dir();
    write_gate_manifest(
        &tmp,
        r#"
profile = "product"
"#,
    );
    let assert = run_gate(&tmp);
    assert.failure();
    let run = latest_run(&tmp);
    assert!(run.blocked);
    assert!(run.results_json.contains("product-code-boundary:command"));
    assert!(run.results_json.contains("placeholder-threshold:command"));
}

#[test]
fn product_profile_optional_concern_without_command_passes() {
    // Explicitly relax the placeholder-threshold to optional; no
    // command bound: the aggregate must pass (NOT_APPLICABLE is
    // visible but never blocks).
    let tmp = init_dir();
    let envelope = make_envelope_file(
        &tmp,
        "boundary.json",
        r#"{"version":1,"status":"PASS","findings":[]}"#,
    );
    write_gate_manifest(
        &tmp,
        &format!(
            r#"
profile = "product"
[[checks]]
id = "placeholder-threshold"
required = false
[[checks]]
id = "product-code-boundary"
command = "cat {envelope} ; exit 0"
"#,
            envelope = envelope.display()
        ),
    );
    let assert = run_gate(&tmp);
    assert.success();
    let run = latest_run(&tmp);
    assert_eq!(run.status, "PASS");
    assert!(!run.blocked);
    assert!(run.results_json.contains("NOT_APPLICABLE"));
}

#[test]
fn product_profile_dry_run_does_not_persist() {
    let tmp = init_dir();
    write_gate_manifest(
        &tmp,
        r#"
profile = "product"
[[checks]]
id = "product-code-boundary"
command = "exit 1"
[[checks]]
id = "placeholder-threshold"
command = "exit 1"
"#,
    );
    let output = driftwatch()
        .arg("gate")
        .arg("--dry-run")
        .current_dir(tmp.path())
        .output()
        .expect("gate --dry-run for product profile");
    assert!(output.status.success());
    let path = tmp.path().join(".driftwatch/state.db");
    let db = Db::open_read_only(&path).expect("read-only db");
    assert_eq!(Gates::new(&db).count().unwrap(), 0);
}

#[test]
fn product_profile_json_output_reflects_status() {
    let tmp = init_dir();
    let envelope = make_envelope_file(
        &tmp,
        "boundary.json",
        r#"{"version":1,"status":"FAIL","severity":"error","findings":[{"title":"x","severity":"error"}]}"#,
    );
    write_gate_manifest(
        &tmp,
        &format!(
            r#"
profile = "product"
[[checks]]
id = "product-code-boundary"
command = "cat {envelope} ; exit 1"
[[checks]]
id = "placeholder-threshold"
command = "exit 0"
"#,
            envelope = envelope.display()
        ),
    );
    let output = driftwatch()
        .arg("gate")
        .arg("--format")
        .arg("json")
        .current_dir(tmp.path())
        .output()
        .expect("run gate --format json");
    assert!(!output.status.success(), "exit nonzero on FAIL");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let doc: serde_json::Value = serde_json::from_str(&stdout).expect("json output");
    assert_eq!(doc["status"], "FAIL");
    assert_eq!(doc["blocked"], true);
    assert!(doc["results"]
        .as_array()
        .unwrap()
        .iter()
        .any(|r| r["gate_id"] == "product-code-boundary" && r["status"] == "FAIL"));
}

#[test]
fn rust_product_profile_is_alias_for_product() {
    let tmp = init_dir();
    let envelope = make_envelope_file(
        &tmp,
        "boundary.json",
        r#"{"version":1,"status":"PASS","findings":[]}"#,
    );
    write_gate_manifest(
        &tmp,
        &format!(
            r#"
profile = "rust-product"
[[checks]]
id = "product-code-boundary"
command = "cat {envelope} ; exit 0"
[[checks]]
id = "placeholder-threshold"
command = "cat {envelope} ; exit 0"
"#,
            envelope = envelope.display()
        ),
    );
    let assert = run_gate(&tmp);
    assert.success();
    let run = latest_run(&tmp);
    assert_eq!(run.status, "PASS");
    assert!(!run.blocked);
}

#[test]
fn product_profile_unknown_envelope_status_falls_back_to_text() {
    let tmp = init_dir();
    let envelope = make_envelope_file(
        &tmp,
        "boundary.json",
        r#"{"version":1,"status":"MAYBE","findings":[]}"#,
    );
    write_gate_manifest(
        &tmp,
        &format!(
            r#"
profile = "product"
[[checks]]
id = "product-code-boundary"
command = "cat {envelope} ; exit 0"
[[checks]]
id = "placeholder-threshold"
command = "cat {envelope} ; exit 0"
"#,
            envelope = envelope.display()
        ),
    );
    // Unknown status means parse_envelope returns None, so we fall
    // back to text-mode pass with envelope-missing diagnostic.
    let assert = run_gate(&tmp);
    assert.success();
    let run = latest_run(&tmp);
    assert!(run
        .results_json
        .contains("no product-quality JSON envelope"));
}

#[test]
fn product_profile_wrong_envelope_version_falls_back_to_text() {
    let tmp = init_dir();
    let envelope = make_envelope_file(
        &tmp,
        "boundary.json",
        r#"{"version":99,"status":"PASS","findings":[]}"#,
    );
    write_gate_manifest(
        &tmp,
        &format!(
            r#"
profile = "product"
[[checks]]
id = "product-code-boundary"
command = "cat {envelope} ; exit 0"
[[checks]]
id = "placeholder-threshold"
command = "cat {envelope} ; exit 0"
"#,
            envelope = envelope.display()
        ),
    );
    let assert = run_gate(&tmp);
    assert.success();
    let run = latest_run(&tmp);
    assert!(run
        .results_json
        .contains("no product-quality JSON envelope"));
}

#[test]
fn product_profile_review_envelope_blocks_when_required() {
    let tmp = init_dir();
    let envelope = make_envelope_file(
        &tmp,
        "boundary.json",
        r#"{"version":1,"status":"REVIEW_REQUIRED","severity":"warning","findings":[],"missing_evidence":["checker:run"]}"#,
    );
    write_gate_manifest(
        &tmp,
        &format!(
            r#"
profile = "product"
[[checks]]
id = "product-code-boundary"
command = "cat {envelope} ; exit 2"
[[checks]]
id = "placeholder-threshold"
command = "exit 0"
"#,
            envelope = envelope.display()
        ),
    );
    let assert = run_gate(&tmp);
    assert.failure();
    let run = latest_run(&tmp);
    assert!(run.blocked);
    assert!(run.results_json.contains("REVIEW_REQUIRED"));
    assert!(run.results_json.contains("checker:run"));
}

#[test]
fn product_profile_review_relaxes_when_blocking_disabled() {
    // Set the blocking policy to allow REVIEW_REQUIRED to remain
    // visible but not block: the gate should still record the review
    // but the run is unblocked.
    let tmp = init_dir();
    let envelope = make_envelope_file(
        &tmp,
        "boundary.json",
        r#"{"version":1,"status":"REVIEW_REQUIRED","severity":"warning","findings":[]}"#,
    );
    write_gate_manifest(
        &tmp,
        &format!(
            r#"
profile = "product"
[blocking]
review_required_blocks = false
[[checks]]
id = "product-code-boundary"
command = "cat {envelope} ; exit 2"
[[checks]]
id = "placeholder-threshold"
command = "exit 0"
"#,
            envelope = envelope.display()
        ),
    );
    let assert = run_gate(&tmp);
    assert.success();
    let run = latest_run(&tmp);
    assert!(!run.blocked);
    assert!(run.results_json.contains("REVIEW_REQUIRED"));
}

#[test]
fn product_profile_dry_run_renders_new_profile_concerns() {
    let tmp = init_dir();
    write_gate_manifest(
        &tmp,
        r#"
profile = "product"
[[checks]]
id = "product-code-boundary"
command = "exit 0"
[[checks]]
id = "placeholder-threshold"
command = "exit 0"
"#,
    );
    let output = driftwatch()
        .arg("gate")
        .arg("--dry-run")
        .current_dir(tmp.path())
        .output()
        .expect("gate --dry-run");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("profile: product"), "stdout: {stdout}");
    assert!(stdout.contains("product-code-boundary"), "stdout: {stdout}");
    assert!(stdout.contains("placeholder-threshold"), "stdout: {stdout}");
}
