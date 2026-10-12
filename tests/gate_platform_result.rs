//! End-to-end tests for `driftwatch gate --format platform`.
//!
//! The contract being emitted (`platform.gate-result` major 1.0.0) is not
//! driftwatchdog's vocabulary: it constrains identifiers driftwatchdog leaves
//! to a project manifest, and it bounds fields driftwatchdog bounds far more
//! loosely. The document declares its own contract version, and every gate
//! status maps onto one of the contract's six values. A run that still cannot
//! be represented must produce **no document at all**: empty stdout, one typed
//! diagnostic on stderr, and a non-zero exit code.
//!
//! These tests go through the binary so the stdout/exit-code contract is
//! observed the way a consumer observes it, not merely asserted on a return
//! value.

#![cfg(unix)]

use assert_cmd::Command;
use driftwatchdog::repo::{gates::Gates, Db};
use serde_json::Value;
use tempfile::TempDir;

fn driftwatch() -> Command {
    Command::cargo_bin("driftwatchdog").expect("compiled driftwatch binary")
}

fn init_dir() -> TempDir {
    let tmp = tempfile::tempdir().unwrap();
    driftwatch()
        .arg("init")
        .current_dir(tmp.path())
        .assert()
        .success();
    tmp
}

/// A governance report every governance-quality concern accepts as clean.
const CLEAN_GOVERNANCE: &str = r#"{"findings":[],"summary":{"errors":0,"warnings":0}}"#;

fn write_manifest(tmp: &TempDir, body: &str) {
    let body = if body.contains("id = \"governance-quality\"") {
        body.to_string()
    } else {
        let report = tmp.path().join("governance.json");
        std::fs::write(&report, CLEAN_GOVERNANCE).unwrap();
        format!(
            "{body}\n[[checks]]\nid = \"governance-quality\"\ncommand = \"cat {}\"\n",
            report.display()
        )
    };
    std::fs::write(tmp.path().join("gate.toml"), body).unwrap();
}

fn gate_with_format(tmp: &TempDir, format: &str) -> assert_cmd::assert::Assert {
    let mut cmd = driftwatch();
    cmd.arg("gate").arg("--format").arg(format);
    cmd.current_dir(tmp.path()).assert()
}

/// Assert the refusal contract: stdout empty, exit non-zero, and the
/// diagnostic on stderr naming `needle`.
fn assert_refused(tmp: &TempDir, needles: &[&str]) -> assert_cmd::assert::Assert {
    let assert = gate_with_format(tmp, "platform").failure().stdout("");
    let stderr = String::from_utf8_lossy(&assert.get_output().stderr).to_string();
    for needle in needles {
        assert!(
            stderr.contains(needle),
            "stderr does not name `{needle}`:\n{stderr}"
        );
    }
    assert
}

fn latest_run(tmp: &TempDir) -> driftwatchdog::repo::gates::GateRunRow {
    let db = Db::open_read_only(&tmp.path().join(".driftwatch/state.db")).unwrap();
    Gates::new(&db)
        .latest()
        .unwrap()
        .expect("a gate run was recorded")
}

// --- the emitted document -------------------------------------------------

#[test]
fn a_clean_pass_emits_a_conforming_document() {
    let tmp = init_dir();
    write_manifest(
        &tmp,
        "profile = \"minimal\"\n[[checks]]\nid = \"tests\"\ncommand = \"true\"\n",
    );

    let assert = gate_with_format(&tmp, "platform").success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout).to_string();
    let doc: Value = serde_json::from_str(&stdout).expect("stdout is one JSON document");

    // The four required contract fields, present and well-typed.
    for required in ["gate_id", "pipeline", "evaluated_at", "result"] {
        assert!(doc.get(required).is_some(), "missing {required}: {doc}");
    }
    // The producer declares which contract version it claims, so a consumer
    // never has to infer it from the values present.
    assert_eq!(doc["contract"], "platform.gate-result/1.0.0");
    assert_eq!(doc["result"], "passed");
    assert_eq!(doc["pipeline"], "driftwatchdog-gate");
    assert!(
        doc["gate_id"].as_str().unwrap().starts_with("sha256:"),
        "gate_id is the run's manifest digest: {doc}"
    );
    assert_eq!(doc["driftwatch_profile"], "minimal");
    // RFC 3339: parseable by the same parser the emitter uses.
    chrono::DateTime::parse_from_rfc3339(doc["evaluated_at"].as_str().unwrap())
        .expect("evaluated_at is RFC 3339");

    let checks = doc["checks"].as_array().expect("checks is an array");
    assert!(checks.iter().all(|c| c["status"] == "passed"), "{doc}");
    let tests = checks
        .iter()
        .find(|c| c["check_id"] == "tests")
        .expect("the scheduled check is emitted");
    // The identity round trip: identical, and the driftwatchdog id is kept.
    assert_eq!(tests["driftwatch_gate_id"], "tests");
    // Never emitted: driftwatchdog has no tenant concept.
    assert!(doc.get("tenant_id").is_none(), "{doc}");
}

#[test]
fn a_clean_fail_emits_failed_and_exits_one() {
    let tmp = init_dir();
    write_manifest(
        &tmp,
        "profile = \"minimal\"\n[[checks]]\nid = \"tests\"\ncommand = \"false\"\n",
    );

    let assert = gate_with_format(&tmp, "platform").code(1);
    let doc: Value = serde_json::from_str(&String::from_utf8_lossy(&assert.get_output().stdout))
        .expect("stdout is one JSON document");
    assert_eq!(doc["result"], "failed");
    let checks = doc["checks"].as_array().unwrap();
    let tests = checks.iter().find(|c| c["check_id"] == "tests").unwrap();
    assert_eq!(tests["status"], "failed");
}

#[test]
fn a_declared_exemption_is_emitted_as_skipped() {
    let tmp = init_dir();
    // Seven scheduled checks give the manifest a two-exemption allowance,
    // so the single declared exemption sits inside its budget and produces
    // no plan-level budget verdict of its own.
    let mut manifest = String::from("profile = \"minimal\"\n");
    for i in 0..7 {
        manifest.push_str(&format!("[[checks]]\nid = \"c{i}\"\ncommand = \"true\"\n"));
    }
    manifest.push_str(
        "[[checks]]\nid = \"ux-review\"\n\
         [not_applicable]\n\
         ux-review = \"no UI surface in this project\"\n",
    );
    write_manifest(&tmp, &manifest);

    let assert = gate_with_format(&tmp, "platform").success();
    let doc: Value = serde_json::from_str(&String::from_utf8_lossy(&assert.get_output().stdout))
        .expect("stdout is one JSON document");
    assert_eq!(doc["result"], "passed");
    let checks = doc["checks"].as_array().unwrap();
    let exempt = checks
        .iter()
        .find(|c| c["check_id"] == "ux-review")
        .unwrap();
    assert_eq!(
        exempt["status"], "skipped",
        "a proven absence is the one `skipped` can honestly name: {doc}"
    );
}

// --- the statuses 1.0.0 added, at both levels --------------------------

#[test]
fn a_required_unbound_check_is_emitted_as_unbound() {
    let tmp = init_dir();
    // Required, selected, no command: the never-attempted absence. The
    // aggregate blocks, so the root result is `failed` and the check itself
    // says `unbound` — two facts, both true, neither merged into the other.
    write_manifest(
        &tmp,
        "profile = \"minimal\"\n[[checks]]\nid = \"ux-review\"\n",
    );

    let assert = gate_with_format(&tmp, "platform").code(1);
    let doc: Value = serde_json::from_str(&String::from_utf8_lossy(&assert.get_output().stdout))
        .expect("stdout is one JSON document");
    assert_eq!(doc["result"], "failed");
    let checks = doc["checks"].as_array().unwrap();
    let unbound = checks
        .iter()
        .find(|c| c["check_id"] == "ux-review")
        .unwrap();
    assert_eq!(
        unbound["status"], "unbound",
        "an unbound check is reported, not omitted and not skipped: {doc}"
    );
    assert_eq!(latest_run(&tmp).status, "FAIL");
}

#[test]
fn an_optional_unbound_check_is_emitted_even_though_it_does_not_block() {
    let tmp = init_dir();
    // Optional, so the aggregate is `PASS` and the document is emitted with a
    // `passed` root and one `unbound` entry. At contract major 0 this run was
    // refused outright.
    write_manifest(
        &tmp,
        "profile = \"minimal\"\n[[checks]]\nid = \"ux-review\"\nrequired = false\n",
    );

    let assert = gate_with_format(&tmp, "platform").success();
    let doc: Value = serde_json::from_str(&String::from_utf8_lossy(&assert.get_output().stdout))
        .expect("stdout is one JSON document");
    assert_eq!(doc["result"], "passed");
    let checks = doc["checks"].as_array().unwrap();
    let unbound = checks
        .iter()
        .find(|c| c["check_id"] == "ux-review")
        .unwrap();
    assert_eq!(unbound["status"], "unbound");
    assert_eq!(latest_run(&tmp).status, "PASS");
}

#[test]
fn a_pending_review_is_emitted_as_review_required() {
    let tmp = init_dir();
    // Optional and slow: it lands in `pending_reviews` as `optional:<id>` and
    // never blocks, so the aggregate is `PASS` and the check carries
    // `review_required`. The 1s deadline is the point; the sleep is the honest
    // part of the fixture, because an ignored deadline would let the check
    // pass and the document would say `passed`.
    write_manifest(
        &tmp,
        "profile = \"minimal\"\n[[checks]]\nid = \"slow\"\nrequired = false\n\
         command = \"sleep 5\"\ntimeout_ms = 1000\n",
    );

    let assert = gate_with_format(&tmp, "platform").success();
    let doc: Value = serde_json::from_str(&String::from_utf8_lossy(&assert.get_output().stdout))
        .expect("stdout is one JSON document");
    assert_eq!(doc["result"], "passed");
    let checks = doc["checks"].as_array().unwrap();
    let slow = checks.iter().find(|c| c["check_id"] == "slow").unwrap();
    assert_eq!(slow["status"], "review_required");
    assert_eq!(latest_run(&tmp).status, "PASS");
}

#[test]
fn a_blocking_pending_review_is_emitted_as_an_aggregate_review_required() {
    let tmp = init_dir();
    write_manifest(
        &tmp,
        "profile = \"minimal\"\n[[checks]]\nid = \"slow\"\n\
         command = \"sleep 5\"\ntimeout_ms = 1000\n",
    );

    let assert = gate_with_format(&tmp, "platform").code(1);
    let doc: Value = serde_json::from_str(&String::from_utf8_lossy(&assert.get_output().stdout))
        .expect("stdout is one JSON document");
    assert_eq!(
        doc["result"], "review_required",
        "an open human decision is its own result, not a failure: {doc}"
    );
    let checks = doc["checks"].as_array().unwrap();
    let slow = checks.iter().find(|c| c["check_id"] == "slow").unwrap();
    assert_eq!(slow["status"], "review_required");
}

/// `errored` is never emitted by this runtime: every execution failure is
/// already a `REVIEW_REQUIRED`, which now has a value of its own.
#[test]
fn a_timed_out_check_is_never_reported_as_errored() {
    let tmp = init_dir();
    write_manifest(
        &tmp,
        "profile = \"minimal\"\n[[checks]]\nid = \"slow\"\n\
         command = \"sleep 5\"\ntimeout_ms = 1000\n",
    );

    let assert = gate_with_format(&tmp, "platform").code(1);
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout).to_string();
    assert!(
        !stdout.contains("\"errored\""),
        "errored has no driftwatchdog source: {stdout}"
    );
}

// --- refusals -------------------------------------------------------------

#[test]
fn a_non_conforming_check_id_refuses_instead_of_being_slugged() {
    let tmp = init_dir();
    // Valid driftwatchdog (any non-empty id parses) and not representable
    // as a platform `check_id`: the leading `L` is outside `^[a-z]`. The
    // command is bound so the refusal is the identifier, not the coverage.
    write_manifest(
        &tmp,
        "profile = \"minimal\"\n[[checks]]\nid = \"L1-configuration\"\ncommand = \"true\"\n",
    );

    let assert = assert_refused(
        &tmp,
        &[
            "refusing to emit",
            "L1-configuration",
            "^[a-z][a-z0-9_.-]{0,63}$",
            "no reversible slug",
        ],
    );
    assert!(String::from_utf8_lossy(&assert.get_output().stdout).is_empty());
}

#[test]
fn an_over_length_diagnostic_refuses_instead_of_being_truncated() {
    let tmp = init_dir();
    // The command prints well past the platform's 512-character `detail`
    // limit and fails, so the normalized diagnostic exceeds it.
    let noise = "x".repeat(900);
    write_manifest(
        &tmp,
        &format!("profile = \"minimal\"\n[[checks]]\nid = \"tests\"\ncommand = \"printf '%s' {noise}; false\"\n"),
    );

    assert_refused(
        &tmp,
        &[
            "refusing to emit",
            "check `tests`",
            "character diagnostic",
            "512",
        ],
    );
}

/// Not every retained refusal class is reachable through the CLI: `gate_id`,
/// `pipeline` and `evaluated_at` come from a digest, a constant, and the run's
/// own RFC 3339 clock, and `subject_ref` is a Git revision. Those five are
/// covered exhaustively by the unit tests in `src/gate/platform_result_tests.rs`
///; what the binary adds is the observable half — empty stdout, a non-zero
/// exit code, and a diagnostic naming the constraint. `check_id`, `detail` and
/// `evidence_ref` are the reachable ones, and they are observed here.
#[test]
fn a_refusal_names_the_constraint_and_writes_nothing_to_stdout() {
    let tmp = init_dir();
    write_manifest(
        &tmp,
        "profile = \"minimal\"\n[[checks]]\nid = \"Not Conforming\"\ncommand = \"true\"\n",
    );
    assert_refused(&tmp, &["refusing to emit", "Not Conforming"]);
}

// --- the refusal is about emission, not about the run ---------------------

#[test]
fn a_refusal_still_persists_the_run_it_declined_to_publish() {
    let tmp = init_dir();
    // The identifier refuses; the run behind it is still real and recorded.
    write_manifest(
        &tmp,
        "profile = \"minimal\"\n[[checks]]\nid = \"L1-configuration\"\ncommand = \"true\"\n",
    );

    gate_with_format(&tmp, "platform").failure().stdout("");

    let run = latest_run(&tmp);
    assert!(
        run.results_json.contains("L1-configuration"),
        "{}",
        run.results_json
    );
    assert!(run.manifest_digest.starts_with("sha256:"));
}

// --- the change is additive -----------------------------------------------

#[test]
fn human_and_json_formats_are_unchanged() {
    let tmp = init_dir();
    write_manifest(
        &tmp,
        "profile = \"minimal\"\n[[checks]]\nid = \"tests\"\ncommand = \"true\"\n",
    );

    let human = gate_with_format(&tmp, "human").success();
    let human_out = String::from_utf8_lossy(&human.get_output().stdout).to_string();
    assert!(
        human_out.starts_with("driftwatch gate: PASS"),
        "{human_out}"
    );
    assert!(human_out.contains("GATE"));

    let json = gate_with_format(&tmp, "json").success();
    let doc: Value =
        serde_json::from_str(&String::from_utf8_lossy(&json.get_output().stdout)).unwrap();
    assert_eq!(doc["status"], "PASS");
    assert_eq!(doc["blocked"], false);
    // driftwatchdog's own vocabulary, untouched by the new producer.
    assert!(doc.get("not_applicable").is_some());
    assert!(doc.get("unbound").is_some());
    assert!(
        doc.get("result").is_none(),
        "no contract field leaks into it"
    );
}

#[test]
fn the_exit_codes_of_the_existing_formats_are_unchanged() {
    let tmp = init_dir();
    write_manifest(
        &tmp,
        "profile = \"minimal\"\n[[checks]]\nid = \"tests\"\ncommand = \"false\"\n",
    );

    // 1 for blocked, never the new refusal code.
    gate_with_format(&tmp, "human").code(1);
    gate_with_format(&tmp, "json").code(1);
    // The blocking run is representable (`failed`), so it is a 1 and not a 2.
    gate_with_format(&tmp, "platform").code(1);
}

#[test]
fn the_default_format_is_still_human() {
    let tmp = init_dir();
    write_manifest(
        &tmp,
        "profile = \"minimal\"\n[[checks]]\nid = \"tests\"\ncommand = \"true\"\n",
    );

    let output = driftwatch()
        .arg("gate")
        .current_dir(tmp.path())
        .output()
        .expect("the gate ran");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    assert!(stdout.starts_with("driftwatch gate:"), "{stdout}");
}

#[test]
fn the_export_subcommand_did_not_gain_the_platform_value() {
    let tmp = init_dir();
    write_manifest(
        &tmp,
        "profile = \"minimal\"\n[[checks]]\nid = \"tests\"\ncommand = \"true\"\n",
    );
    gate_with_format(&tmp, "human").success();

    // `evidence-export` emits a different vocabulary and keeps two values.
    driftwatch()
        .arg("gate")
        .arg("evidence-export")
        .arg("--format")
        .arg("platform")
        .current_dir(tmp.path())
        .assert()
        .failure();
}

#[test]
fn a_project_without_a_manifest_still_exits_zero_under_the_platform_flag() {
    let tmp = init_dir();

    let assert = gate_with_format(&tmp, "platform").success();
    let stdout = String::from_utf8_lossy(&assert.get_output().stdout).to_string();
    // Same early-return text as `human` and `json`: the format only applies
    // once a run exists to describe.
    assert!(
        stdout.contains("no gate.toml or .ai-gate/gate.yaml"),
        "{stdout}"
    );
}
