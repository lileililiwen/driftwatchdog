//! End-to-end tests for the release-evidence and capability-conformance
//! Gate concerns.
//!
//! All tests are Unix-only because the project-runtime adapter launches
//! its target via `sh -c`. The unit tests in `src/gate/` cover the
//! adapter pieces (envelope parsing, mapping rules, exit-code
//! authority, staleness, required-evidence guards).

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

fn release_pass_envelope() -> String {
    r#"{"version":1,"status":"PASS","severity":"info","revision":"abc123","product_version":"1.2.3","artifacts":["binary.tar.gz"],"provenance":{"type":"slsa-provenance/v0.2","digest":"sha256:deadbeef"},"findings":[]}"#.to_string()
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

#[test]
fn release_profile_passes_with_complete_envelopes() {
    let tmp = init_dir();
    let release_env = make_envelope_file(&tmp, "release.json", &release_pass_envelope());
    let cap_env = make_envelope_file(
        &tmp,
        "capability.json",
        &capability_pass_envelope(&["release-evidence", "capability-conformance"]),
    );
    write_gate_manifest(
        &tmp,
        &format!(
            r#"
profile = "release"
[[checks]]
id = "release-evidence"
command = "cat {release_env} ; exit 0"
[[checks]]
id = "capability-conformance"
command = "cat {cap_env} ; exit 0"
"#,
            release_env = release_env.display(),
            cap_env = cap_env.display(),
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
fn release_profile_fails_with_failing_envelope() {
    let tmp = init_dir();
    let envelope = make_envelope_file(
        &tmp,
        "release.json",
        r#"{"version":1,"status":"FAIL","severity":"error","findings":[{"title":"missing SBOM","severity":"error","rule":"sbom-required"}],"diagnostic":"see findings"}"#,
    );
    write_gate_manifest(
        &tmp,
        &format!(
            r#"
profile = "release"
[[checks]]
id = "release-evidence"
command = "cat {envelope} ; exit 1"
[[checks]]
id = "capability-conformance"
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
    assert!(run.results_json.contains("missing SBOM"));
    assert!(run.results_json.contains("sbom-required"));
}

#[test]
fn release_profile_contradictory_envelope_is_review() {
    // Command exits 1 but envelope claims PASS: REVIEW_REQUIRED.
    let tmp = init_dir();
    let envelope = make_envelope_file(&tmp, "release.json", &release_pass_envelope());
    write_gate_manifest(
        &tmp,
        &format!(
            r#"
profile = "release"
[[checks]]
id = "release-evidence"
command = "cat {envelope} ; exit 1"
[[checks]]
id = "capability-conformance"
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
fn release_profile_missing_required_evidence_becomes_review() {
    // PASS with no revision/product_version/artifacts/provenance:
    // REVIEW_REQUIRED via the required-evidence guard.
    let tmp = init_dir();
    let envelope = make_envelope_file(
        &tmp,
        "release.json",
        r#"{"version":1,"status":"PASS","findings":[]}"#,
    );
    write_gate_manifest(
        &tmp,
        &format!(
            r#"
profile = "release"
[[checks]]
id = "release-evidence"
command = "cat {envelope} ; exit 0"
[[checks]]
id = "capability-conformance"
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
    assert!(run.results_json.contains("missing required fields"));
    assert!(run.results_json.contains("revision"));
    assert!(run.results_json.contains("product_version"));
    assert!(run.results_json.contains("artifacts"));
    assert!(run.results_json.contains("provenance"));
}

#[test]
fn release_profile_malformed_envelope_becomes_review() {
    // No envelope, output present: REVIEW_REQUIRED.
    let tmp = init_dir();
    write_gate_manifest(
        &tmp,
        r#"
profile = "release"
[[checks]]
id = "release-evidence"
command = "echo legacy; exit 0"
[[checks]]
id = "capability-conformance"
command = "echo legacy-cap; exit 0"
"#,
    );
    let assert = run_gate(&tmp);
    assert.failure();
    let run = latest_run(&tmp);
    assert!(run.blocked);
    assert!(run.results_json.contains("REVIEW_REQUIRED"));
    assert!(run.results_json.contains("no release-gate JSON envelope"));
}

#[test]
fn release_profile_empty_output_is_review() {
    let tmp = init_dir();
    write_gate_manifest(
        &tmp,
        r#"
profile = "release"
[[checks]]
id = "release-evidence"
command = "exit 0"
[[checks]]
id = "capability-conformance"
command = "exit 0"
"#,
    );
    let assert = run_gate(&tmp);
    assert.failure();
    let run = latest_run(&tmp);
    assert!(run.blocked);
    assert!(run.results_json.contains("REVIEW_REQUIRED"));
}

#[test]
fn release_profile_required_command_missing_is_review() {
    // No command bound for a required release-gate concern:
    // REVIEW_REQUIRED via the aggregate path.
    let tmp = init_dir();
    write_gate_manifest(
        &tmp,
        r#"
profile = "release"
"#,
    );
    let assert = run_gate(&tmp);
    assert.failure();
    let run = latest_run(&tmp);
    assert!(run.blocked);
    assert!(run.results_json.contains("release-evidence:command"));
    assert!(run.results_json.contains("capability-conformance:command"));
}

#[test]
fn release_profile_optional_concern_without_command_passes() {
    // Explicitly relax capability-conformance to optional; no
    // command bound: aggregate must pass for the release-evidence
    // half (NOT_APPLICABLE is visible but never blocks).
    let tmp = init_dir();
    let envelope = make_envelope_file(&tmp, "release.json", &release_pass_envelope());
    write_gate_manifest(
        &tmp,
        &format!(
            r#"
profile = "release"
[[checks]]
id = "capability-conformance"
required = false
[[checks]]
id = "release-evidence"
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
fn release_profile_dry_run_renders_release_concerns() {
    let tmp = init_dir();
    write_gate_manifest(
        &tmp,
        r#"
profile = "release"
[[checks]]
id = "release-evidence"
command = "exit 0"
[[checks]]
id = "capability-conformance"
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
    assert!(stdout.contains("profile: release"), "stdout: {stdout}");
    assert!(stdout.contains("release-evidence"), "stdout: {stdout}");
    assert!(
        stdout.contains("capability-conformance"),
        "stdout: {stdout}"
    );
}

#[test]
fn release_profile_dry_run_does_not_persist() {
    let tmp = init_dir();
    write_gate_manifest(
        &tmp,
        r#"
profile = "release"
[[checks]]
id = "release-evidence"
command = "exit 1"
[[checks]]
id = "capability-conformance"
command = "exit 1"
"#,
    );
    let output = driftwatch()
        .arg("gate")
        .arg("--dry-run")
        .current_dir(tmp.path())
        .output()
        .expect("gate --dry-run");
    assert!(output.status.success());
    let path = tmp.path().join(".driftwatch/state.db");
    let db = Db::open_read_only(&path).expect("read-only db");
    assert_eq!(Gates::new(&db).count().unwrap(), 0);
}

#[test]
fn release_profile_json_output_reflects_status() {
    let tmp = init_dir();
    let envelope = make_envelope_file(
        &tmp,
        "release.json",
        r#"{"version":1,"status":"FAIL","severity":"error","findings":[{"title":"x","severity":"error"}]}"#,
    );
    write_gate_manifest(
        &tmp,
        &format!(
            r#"
profile = "release"
[[checks]]
id = "release-evidence"
command = "cat {envelope} ; exit 1"
[[checks]]
id = "capability-conformance"
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
    assert!(!output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let doc: serde_json::Value = serde_json::from_str(&stdout).expect("json output");
    assert_eq!(doc["status"], "FAIL");
    assert_eq!(doc["blocked"], true);
    assert!(doc["results"]
        .as_array()
        .unwrap()
        .iter()
        .any(|r| r["gate_id"] == "release-evidence" && r["status"] == "FAIL"));
}

#[test]
fn release_profile_capability_pass_with_verified_subset_passes() {
    // The capability-conformance check reports verified ids that
    // are a subset of the resolved plan: PASS.
    let tmp = init_dir();
    let release_env = make_envelope_file(&tmp, "release.json", &release_pass_envelope());
    let cap_env = make_envelope_file(
        &tmp,
        "capability.json",
        &capability_pass_envelope(&["release-evidence"]),
    );
    write_gate_manifest(
        &tmp,
        &format!(
            r#"
profile = "release"
[[checks]]
id = "release-evidence"
command = "cat {release_env} ; exit 0"
[[checks]]
id = "capability-conformance"
command = "cat {cap_env} ; exit 0"
"#,
            release_env = release_env.display(),
            cap_env = cap_env.display(),
        ),
    );
    let assert = run_gate(&tmp);
    assert.success();
    let run = latest_run(&tmp);
    assert_eq!(run.status, "PASS");
    assert!(!run.blocked);
}

#[test]
fn release_profile_capability_with_out_of_scope_verified_becomes_review() {
    // The capability-conformance envelope claims a capability
    // outside the resolved plan: REVIEW_REQUIRED via the
    // out-of-scope guard.
    let tmp = init_dir();
    let release_env = make_envelope_file(&tmp, "release.json", &release_pass_envelope());
    let cap_env = make_envelope_file(
        &tmp,
        "capability.json",
        &capability_pass_envelope(&["ui-review", "release-evidence"]),
    );
    write_gate_manifest(
        &tmp,
        &format!(
            r#"
profile = "release"
[[checks]]
id = "release-evidence"
command = "cat {release_env} ; exit 0"
[[checks]]
id = "capability-conformance"
command = "cat {cap_env} ; exit 0"
"#,
            release_env = release_env.display(),
            cap_env = cap_env.display(),
        ),
    );
    let assert = run_gate(&tmp);
    assert.failure();
    let run = latest_run(&tmp);
    assert!(run.blocked);
    assert!(run.results_json.contains("not in the resolved plan"));
    assert!(run.results_json.contains("ui-review"));
}

#[test]
fn release_profile_capability_with_empty_verified_becomes_review() {
    let tmp = init_dir();
    let envelope = make_envelope_file(
        &tmp,
        "release.json",
        r#"{"version":1,"status":"PASS","capabilities":{"declared":[],"configured":[],"verified":[],"unverified":[]},"findings":[]}"#,
    );
    write_gate_manifest(
        &tmp,
        &format!(
            r#"
profile = "release"
[[checks]]
id = "release-evidence"
command = "cat {envelope} ; exit 0"
[[checks]]
id = "capability-conformance"
command = "cat {envelope} ; exit 0"
"#,
            envelope = envelope.display()
        ),
    );
    let assert = run_gate(&tmp);
    assert.failure();
    let run = latest_run(&tmp);
    assert!(run.blocked);
    assert!(run.results_json.contains("`verified` is empty"));
}

#[test]
fn release_profile_stale_revision_in_git_repo_becomes_review() {
    // Initialize a real git repo with one commit, then make the
    // release-evidence envelope claim a *different* revision.
    // The current revision captured by `driftwatch gate` will not
    // match the envelope's claim, so the staleness guard must
    // downgrade PASS to REVIEW_REQUIRED.
    let tmp = init_dir();
    let project = tmp.path();
    // Initialise a repo and make a single commit so git rev-parse HEAD works.
    std::process::Command::new("git")
        .args(["init", "-q"])
        .current_dir(project)
        .output()
        .expect("git init");
    std::process::Command::new("git")
        .args(["config", "user.email", "test@example.com"])
        .current_dir(project)
        .output()
        .expect("git config email");
    std::process::Command::new("git")
        .args(["config", "user.name", "test"])
        .current_dir(project)
        .output()
        .expect("git config name");
    std::fs::write(project.join("README.md"), "test\n").unwrap();
    std::process::Command::new("git")
        .args(["add", "README.md"])
        .current_dir(project)
        .output()
        .expect("git add");
    std::process::Command::new("git")
        .args(["commit", "-q", "-m", "init"])
        .current_dir(project)
        .output()
        .expect("git commit");
    let real_rev = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(project)
        .output()
        .expect("git rev-parse")
        .stdout;
    let real_rev = String::from_utf8_lossy(&real_rev).trim().to_string();
    assert!(!real_rev.is_empty(), "git rev-parse must yield a hash");
    // Envelope claims a stale (different) revision.
    let envelope = make_envelope_file(
        &tmp,
        "release.json",
        r#"{"version":1,"status":"PASS","severity":"info","revision":"stale-rev","product_version":"1.0.0","artifacts":["a.tar.gz"],"provenance":{"type":"x"},"findings":[]}"#,
    );
    write_gate_manifest(
        &tmp,
        &format!(
            r#"
profile = "release"
[[checks]]
id = "release-evidence"
command = "cat {envelope} ; exit 0"
[[checks]]
id = "capability-conformance"
command = "exit 0"
"#,
            envelope = envelope.display()
        ),
    );
    let assert = run_gate(&tmp);
    assert.failure();
    let run = latest_run(&tmp);
    assert!(run.blocked);
    assert!(run.results_json.contains("stale"));
    assert!(run.results_json.contains("stale-rev"));
    assert!(run.results_json.contains(&real_rev));
}

#[test]
fn release_profile_wrong_envelope_version_becomes_review() {
    let tmp = init_dir();
    let envelope = make_envelope_file(
        &tmp,
        "release.json",
        r#"{"version":99,"status":"PASS","findings":[]}"#,
    );
    write_gate_manifest(
        &tmp,
        &format!(
            r#"
profile = "release"
[[checks]]
id = "release-evidence"
command = "cat {envelope} ; exit 0"
[[checks]]
id = "capability-conformance"
command = "exit 0"
"#,
            envelope = envelope.display()
        ),
    );
    let assert = run_gate(&tmp);
    assert.failure();
    let run = latest_run(&tmp);
    assert!(run.blocked);
    assert!(run.results_json.contains("no release-gate JSON envelope"));
}

#[test]
fn release_profile_not_applicable_envelope_for_optional_concern_passes() {
    // A NOT_APPLICABLE envelope for an optional capability-conformance
    // check (exit 0) is the project-owned opt-out: aggregate passes
    // (NOT_APPLICABLE is visible but never blocks because the concern
    // is explicitly relaxed).
    let tmp = init_dir();
    let release_env = make_envelope_file(&tmp, "release.json", &release_pass_envelope());
    let cap_env = make_envelope_file(
        &tmp,
        "capability.json",
        r#"{"version":1,"status":"NOT_APPLICABLE","severity":"info","capabilities":{"declared":[],"configured":[],"verified":[],"unverified":[]},"findings":[]}"#,
    );
    write_gate_manifest(
        &tmp,
        &format!(
            r#"
profile = "release"
[[checks]]
id = "release-evidence"
command = "cat {release_env} ; exit 0"
[[checks]]
id = "capability-conformance"
required = false
command = "cat {cap_env} ; exit 0"
"#,
            release_env = release_env.display(),
            cap_env = cap_env.display(),
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
fn release_profile_not_applicable_envelope_for_required_concern_blocks() {
    // A NOT_APPLICABLE envelope for a required concern is treated
    // as missing coverage (consistent with the existing aggregate
    // rule): the gate blocks with REVIEW_REQUIRED.
    let tmp = init_dir();
    let release_env = make_envelope_file(&tmp, "release.json", &release_pass_envelope());
    let cap_env = make_envelope_file(
        &tmp,
        "capability.json",
        r#"{"version":1,"status":"NOT_APPLICABLE","severity":"info","capabilities":{"declared":[],"configured":[],"verified":[],"unverified":[]},"findings":[]}"#,
    );
    write_gate_manifest(
        &tmp,
        &format!(
            r#"
profile = "release"
[[checks]]
id = "release-evidence"
command = "cat {release_env} ; exit 0"
[[checks]]
id = "capability-conformance"
command = "cat {cap_env} ; exit 0"
"#,
            release_env = release_env.display(),
            cap_env = cap_env.display(),
        ),
    );
    let assert = run_gate(&tmp);
    assert.failure();
    let run = latest_run(&tmp);
    assert!(run.blocked);
    assert!(run.results_json.contains("NOT_APPLICABLE"));
    assert!(run.results_json.contains("capability-conformance"));
}

#[test]
fn release_profile_history_records_gate_run() {
    // doctor / gate.history integration: after a passing release
    // profile run, the persisted `gate_runs` row is queryable.
    let tmp = init_dir();
    let release_env = make_envelope_file(&tmp, "release.json", &release_pass_envelope());
    let cap_env = make_envelope_file(
        &tmp,
        "capability.json",
        &capability_pass_envelope(&["release-evidence", "capability-conformance"]),
    );
    write_gate_manifest(
        &tmp,
        &format!(
            r#"
profile = "release"
[[checks]]
id = "release-evidence"
command = "cat {release_env} ; exit 0"
[[checks]]
id = "capability-conformance"
command = "cat {cap_env} ; exit 0"
"#,
            release_env = release_env.display(),
            cap_env = cap_env.display(),
        ),
    );
    let assert = run_gate(&tmp);
    assert.success();
    let path = tmp.path().join(".driftwatch/state.db");
    let db = Db::open_read_only(&path).expect("read-only db");
    let count = Gates::new(&db).count().unwrap();
    assert!(count >= 1, "at least one gate run persisted");
    let latest = latest_run(&tmp);
    assert!(latest.manifest_digest.starts_with("sha256:"));
}
