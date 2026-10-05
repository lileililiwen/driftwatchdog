//! End-to-end tests for the unbound-required-check gate.
//!
//! A check that asserts coverage but binds no command is a
//! never-attempted absence. It must be reported as `UNBOUND`, block
//! the gate independently of the project's `blocking:` list, and stay
//! distinct from a proven or declared exemption on every surface:
//! the stored run, the human and JSON output, and the dry-run plan.

#![cfg(unix)]

use assert_cmd::Command;
use driftwatchdog::gate::dto::GateRunResultsDocument;
use driftwatchdog::gate::types::{GateSeverity, GateStatus};
use driftwatchdog::repo::{gates::Gates, Db};
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

fn write_manifest(tmp: &TempDir, body: &str) {
    let body = if body.contains("id = \"governance-quality\"") {
        body.to_string()
    } else {
        let report = tmp.path().join("governance.json");
        std::fs::write(
            &report,
            r#"{"findings":[],"summary":{"errors":0,"warnings":0}}"#,
        )
        .unwrap();
        format!(
            "{body}\n[[checks]]\nid = \"governance-quality\"\ncommand = \"cat {}\"\n",
            report.display()
        )
    };
    std::fs::write(tmp.path().join("gate.toml"), body).unwrap();
}

fn write_yaml_manifest(tmp: &TempDir, body: &str) {
    std::fs::create_dir_all(tmp.path().join(".ai-gate")).unwrap();
    let body = if body.contains("governance-quality") {
        body.to_string()
    } else {
        let report = tmp.path().join("governance.json");
        std::fs::write(
            &report,
            r#"{"findings":[],"summary":{"errors":0,"warnings":0}}"#,
        )
        .unwrap();
        let mut body = body.replace("checks:\n", "checks:\n  governance-quality: true\n");
        let command = format!("cat {}", report.display());
        if body.contains("commands:\n") {
            body = body.replace(
                "commands:\n",
                &format!("commands:\n  governance-quality: {:?}\n", command),
            );
        } else {
            body.push_str(&format!(
                "\ncommands:\n  governance-quality: {:?}\n",
                command
            ));
        }
        body
    };
    std::fs::write(tmp.path().join(".ai-gate/gate.yaml"), body).unwrap();
}

fn latest_run(tmp: &TempDir) -> driftwatchdog::repo::gates::GateRunRow {
    let db = Db::open_read_only(&tmp.path().join(".driftwatch/state.db")).unwrap();
    Gates::new(&db)
        .latest()
        .unwrap()
        .expect("a gate run was persisted")
}

fn run_doc(tmp: &TempDir) -> GateRunResultsDocument {
    let run = latest_run(tmp);
    serde_json::from_str(&run.results_json).expect("results_json is a run document")
}

fn result_of<'a>(
    doc: &'a GateRunResultsDocument,
    id: &str,
) -> &'a driftwatchdog::gate::types::GateResult {
    doc.results
        .iter()
        .find(|r| r.gate_id == id)
        .unwrap_or_else(|| panic!("result for {id} missing"))
}

/// Build a `minimal` manifest with `bound` bound required checks, so a
/// fixture that declares an exemption has an allowance to declare it
/// inside. The exemption cap (owner decision, 2026-10-02) floors to zero
/// below four required checks, so a one- or two-check manifest can no
/// longer demonstrate any exemption behaviour at all.
fn with_bound_checks(bound: usize) -> String {
    let mut out = String::from("profile = \"minimal\"\n");
    out.push_str(
        r#"
[[checks]]
id = "governance-quality"
required = false
command = "echo '{\"findings\":[],\"summary\":{\"errors\":0,\"warnings\":0}}'"
"#,
    );
    for i in 0..bound {
        out.push_str(&format!(
            "\n[[checks]]\nid = \"c{i}\"\ncommand = \"true\"\n"
        ));
    }
    out
}

#[test]
fn required_check_without_command_fails_the_run_as_unbound() {
    let tmp = init_dir();
    write_manifest(
        &tmp,
        "profile = \"minimal\"\n[[checks]]\nid = \"ux-review\"\n",
    );

    driftwatch()
        .arg("gate")
        .current_dir(tmp.path())
        .assert()
        .failure();

    let run = latest_run(&tmp);
    assert_eq!(run.status, "FAIL");
    assert!(run.blocked);

    let doc = run_doc(&tmp);
    let r = result_of(&doc, "ux-review");
    assert_eq!(r.status, GateStatus::Unbound);
    assert_eq!(r.severity, GateSeverity::Error);
    // The finding names the check and the absent command; evidence is
    // empty because nothing ran, and the absent command is recorded as
    // missing evidence.
    assert_eq!(r.findings.len(), 1);
    assert_eq!(
        r.findings[0].location.as_deref(),
        Some("commands.ux-review")
    );
    assert!(r.findings[0].title.contains("no bound command"));
    assert!(r.evidence.is_empty());
    assert_eq!(r.missing_evidence, vec!["ux-review:command".to_string()]);
}

/// The whole point of the change: a project must not be able to make a
/// never-attempted required check pass by trimming its blocking list.
#[test]
fn required_unbound_blocks_even_when_review_required_does_not_block() {
    // Control: with `review_required_blocks = false` and every required
    // check actually bound, the gate passes — the trimmed list is
    // honoured for reviews.
    let bound_only = init_dir();
    write_manifest(
        &bound_only,
        r#"profile = "minimal"
[blocking]
review_required_blocks = false

[[checks]]
id = "ux-review"
command = "true"
"#,
    );
    driftwatch()
        .arg("gate")
        .current_dir(bound_only.path())
        .assert()
        .success();
    assert_eq!(latest_run(&bound_only).status, "PASS");

    // Same trimmed list, plus one required check that binds no command.
    let tmp = init_dir();
    write_manifest(
        &tmp,
        r#"profile = "minimal"
[blocking]
review_required_blocks = false

[[checks]]
id = "ux-review"
command = "true"

[[checks]]
id = "a11y"
"#,
    );
    driftwatch()
        .arg("gate")
        .current_dir(tmp.path())
        .assert()
        .failure();

    let run = latest_run(&tmp);
    assert_eq!(run.status, "FAIL", "a trimmed blocking list must not pass");
    assert!(run.blocked);
    assert_eq!(run_doc(&tmp).unbound, vec!["a11y".to_string()]);
}

#[test]
fn optional_unbound_check_is_recorded_and_never_blocks() {
    let tmp = init_dir();
    write_manifest(
        &tmp,
        "profile = \"minimal\"\n[[checks]]\nid = \"ux-review\"\nrequired = false\n",
    );

    driftwatch()
        .arg("gate")
        .current_dir(tmp.path())
        .assert()
        .success();

    let run = latest_run(&tmp);
    assert_eq!(run.status, "PASS");
    assert!(!run.blocked);
    let doc = run_doc(&tmp);
    assert_eq!(doc.unbound, vec!["optional:ux-review".to_string()]);
    let r = result_of(&doc, "ux-review");
    assert_eq!(r.status, GateStatus::Unbound);
    assert_eq!(r.severity, GateSeverity::Warning);
}

#[test]
fn a_bound_command_is_not_unbound() {
    let tmp = init_dir();
    write_manifest(
        &tmp,
        r#"profile = "minimal"
[[checks]]
id = "ux-review"
command = "true"
"#,
    );

    driftwatch()
        .arg("gate")
        .current_dir(tmp.path())
        .assert()
        .success();

    let doc = run_doc(&tmp);
    assert!(doc.unbound.is_empty());
    assert_eq!(result_of(&doc, "ux-review").status, GateStatus::Pass);
}

#[test]
fn a_declared_exemption_does_not_block_and_records_its_reason() {
    let tmp = init_dir();
    // 7 bound + 1 exempted = 8 required, so the allowance is 2 and the
    // single declaration sits inside it.
    write_manifest(
        &tmp,
        &format!(
            "{}\n[[checks]]\nid = \"ux-review\"\n\n[not_applicable]\n\
             ux-review = \"no UI surface in this project\"\n",
            with_bound_checks(7)
        ),
    );

    driftwatch()
        .arg("gate")
        .current_dir(tmp.path())
        .assert()
        .success();

    let run = latest_run(&tmp);
    assert_eq!(run.status, "PASS");
    let doc = run_doc(&tmp);
    assert!(doc.unbound.is_empty());
    let r = result_of(&doc, "ux-review");
    assert_eq!(r.status, GateStatus::NotApplicable);
    assert!(r.findings[0]
        .title
        .contains("no UI surface in this project"));
}

#[test]
fn unbound_and_not_applicable_stay_separate_in_one_run() {
    // One run holding an exempt check and an unbound check: the exempt
    // id appears only as a proven/acknowledged absence, the unbound id
    // only in the unbound collection. The bound checks give the manifest
    // an exemption allowance, so the run is red for the unbound check
    // alone and not also for the budget.
    let tmp = init_dir();
    write_manifest(
        &tmp,
        &format!(
            "{}\n[[checks]]\nid = \"a11y\"\n\n[not_applicable]\n\
             a11y = \"no UI surface in this project\"\n\n[[checks]]\nid = \"ux-review\"\n",
            with_bound_checks(7)
        ),
    );

    driftwatch()
        .arg("gate")
        .current_dir(tmp.path())
        .assert()
        .failure();

    let doc = run_doc(&tmp);
    assert_eq!(
        doc.unbound,
        vec!["ux-review".to_string()],
        "the exempt check must not appear as unbound"
    );
    assert_eq!(result_of(&doc, "ux-review").status, GateStatus::Unbound);
    assert_eq!(result_of(&doc, "a11y").status, GateStatus::NotApplicable);
    assert_ne!(result_of(&doc, "a11y").status, GateStatus::Unbound);
}

#[test]
fn reject_policy_refuses_before_execution_and_persists_nothing() {
    let tmp = init_dir();
    write_manifest(
        &tmp,
        r#"profile = "minimal"
binding_policy = "reject"

[[checks]]
id = "ux-review"
"#,
    );

    let out = driftwatch()
        .arg("gate")
        .current_dir(tmp.path())
        .output()
        .unwrap();
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("ux-review"), "{stderr}");
    assert!(stderr.contains("binding_policy"), "{stderr}");

    // No run was persisted: resolution failed before execution.
    let db = Db::open_read_only(&tmp.path().join(".driftwatch/state.db")).unwrap();
    assert_eq!(Gates::new(&db).count().unwrap(), 0);
}

#[test]
fn reject_policy_resolves_once_every_required_check_is_bound_or_exempt() {
    let tmp = init_dir();
    // `binding_policy` is a top-level key, so it goes in the header
    // rather than after the last `[[checks]]` table.
    let manifest = with_bound_checks(7).replace(
        "profile = \"minimal\"\n",
        "profile = \"minimal\"\nbinding_policy = \"reject\"\n",
    );
    write_manifest(
        &tmp,
        &format!(
            "{manifest}\n[[checks]]\nid = \"ux-review\"\n\
             command = \"true\"\n\n[[checks]]\nid = \"a11y\"\n\n[not_applicable]\n\
             a11y = \"no UI surface in this project\"\n"
        ),
    );

    driftwatch()
        .arg("gate")
        .current_dir(tmp.path())
        .assert()
        .success();
    assert_eq!(latest_run(&tmp).status, "PASS");
}

#[test]
fn yaml_manifest_honours_the_same_command_binding_rules() {
    let tmp = init_dir();
    write_yaml_manifest(
        &tmp,
        "version: 1\nruntime: driftwatchdog\nprofile: minimal\nchecks:\n  build: true\n",
    );
    driftwatch()
        .arg("gate")
        .current_dir(tmp.path())
        .assert()
        .failure();
    let doc = run_doc(&tmp);
    assert_eq!(doc.unbound, vec!["build".to_string()]);
    assert_eq!(result_of(&doc, "build").status, GateStatus::Unbound);

    // The same rules apply to the YAML form: seven bound checks give the
    // manifest an exemption allowance, so the single declaration is
    // acknowledged and the run is green.
    let mut checks = String::new();
    for i in 0..7 {
        checks.push_str(&format!("  c{i}: true\n"));
    }
    let tmp2 = init_dir();
    write_yaml_manifest(
        &tmp2,
        &format!(
            "version: 1\nruntime: driftwatchdog\nprofile: minimal\nchecks:\n{checks}  a11y: true\n\
             commands:\n  c0: \"true\"\n  c1: \"true\"\n  c2: \"true\"\n  c3: \"true\"\n  \
             c4: \"true\"\n  c5: \"true\"\n  c6: \"true\"\n\
             not_applicable:\n  a11y: \"no UI surface in this project\"\n"
        ),
    );
    driftwatch()
        .arg("gate")
        .current_dir(tmp2.path())
        .assert()
        .success();
    assert!(run_doc(&tmp2).unbound.is_empty());
}

#[test]
fn human_and_json_output_distinguish_unbound_from_not_applicable() {
    // An exemption naming an unselected check is refused outright.
    let refused = init_dir();
    write_manifest(
        &refused,
        r#"profile = "minimal"
[[checks]]
id = "ux-review"

[not_applicable]
a11y = "no UI surface in this project"
"#,
    );
    let out = driftwatch()
        .arg("gate")
        .current_dir(refused.path())
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("a11y"));

    // The unbound check renders as its own claim on both surfaces.
    let tmp = init_dir();
    write_manifest(
        &tmp,
        "profile = \"minimal\"\n[[checks]]\nid = \"ux-review\"\n",
    );
    let human = driftwatch()
        .arg("gate")
        .current_dir(tmp.path())
        .output()
        .unwrap();
    let human = String::from_utf8_lossy(&human.stdout);
    assert!(human.contains("UNBOUND"), "{human}");
    assert!(
        human.contains("Unbound required checks: ux-review"),
        "{human}"
    );
    assert!(!human.contains("Not applicable: ux-review"), "{human}");

    let json = driftwatch()
        .args(["gate", "--format", "json"])
        .current_dir(tmp.path())
        .output()
        .unwrap();
    let doc: serde_json::Value = serde_json::from_slice(&json.stdout).unwrap();
    assert_eq!(doc["status"], "FAIL");
    assert_eq!(doc["unbound"][0], "ux-review");
    assert!(doc["not_applicable"].as_array().unwrap().is_empty());
}

#[test]
fn dry_run_plan_names_unbound_checks_without_executing() {
    let tmp = init_dir();
    write_manifest(
        &tmp,
        r#"profile = "minimal"
[[checks]]
id = "ux-review"
"#,
    );

    let out = driftwatch()
        .args(["gate", "--dry-run"])
        .current_dir(tmp.path())
        .output()
        .unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("unbound required checks"), "{stdout}");
    assert!(stdout.contains("ux-review"), "{stdout}");
    // Pure: nothing was executed, so no run was persisted.
    let db = Db::open_read_only(&tmp.path().join(".driftwatch/state.db")).unwrap();
    assert_eq!(Gates::new(&db).count().unwrap(), 0);
}

#[test]
fn stored_run_payload_keeps_legacy_shapes_readable() {
    // A run persisted before this change (bare JSON array) still parses,
    // so stored history stays exportable.
    let legacy =
        r#"[{"gate_id":"a11y","source":"manifest","status":"NOT_APPLICABLE","severity":"info"}]"#;
    let parsed: Vec<driftwatchdog::gate::types::GateResult> = serde_json::from_str(legacy).unwrap();
    assert_eq!(parsed[0].status, GateStatus::NotApplicable);
}

#[test]
fn exemption_reason_is_bounded_and_redacted_in_the_stored_run() {
    let tmp = init_dir();
    let long = "z".repeat(8192);
    write_manifest(
        &tmp,
        &format!(
            "{}\n[[checks]]\nid = \"ux-review\"\n\
             \n[not_applicable]\nux-review = \"api_key=supersecret123 {long}\"\n",
            with_bound_checks(7)
        ),
    );
    driftwatch()
        .arg("gate")
        .current_dir(tmp.path())
        .assert()
        .success();

    let run = latest_run(&tmp);
    assert!(
        !run.results_json.contains("supersecret123"),
        "credential-shaped token must not reach the stored run"
    );
    let doc = run_doc(&tmp);
    let r = result_of(&doc, "ux-review");
    assert!(r.findings[0].title.len() < 4096, "reason must be bounded");
}

/// Task 3.3: the rule is not a closed-vocabulary lookup. An invented,
/// single-repo check kind with no command fails the same way, so
/// renaming a concern cannot escape the rule.
#[test]
fn an_invented_check_kind_with_no_command_is_unbound() {
    let tmp = init_dir();
    write_manifest(
        &tmp,
        "profile = \"minimal\"\n[[checks]]\nid = \"a-concern-no-repo-has-heard-of\"\n",
    );
    driftwatch()
        .arg("gate")
        .current_dir(tmp.path())
        .assert()
        .failure();
    let doc = run_doc(&tmp);
    assert_eq!(
        doc.unbound,
        vec!["a-concern-no-repo-has-heard-of".to_string()]
    );
    assert_eq!(
        result_of(&doc, "a-concern-no-repo-has-heard-of").status,
        GateStatus::Unbound
    );
}

/// The mirror image: a check selected `false` is not scheduled, so it
/// produces no result and no `UNBOUND` — deselecting is the honest
/// alternative to binding or exempting.
#[test]
fn a_deselected_check_produces_no_result_and_no_unbound() {
    let tmp = init_dir();
    write_manifest(
        &tmp,
        r#"profile = "minimal"
[[checks]]
id = "a11y"
enabled = false

[[checks]]
id = "smoke"
command = "true"
"#,
    );
    driftwatch()
        .arg("gate")
        .current_dir(tmp.path())
        .assert()
        .success();
    let doc = run_doc(&tmp);
    assert!(doc.unbound.is_empty());
    assert!(
        doc.results.iter().all(|r| r.gate_id != "a11y"),
        "a deselected check must not run"
    );
}
