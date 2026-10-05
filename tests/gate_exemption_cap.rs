//! End-to-end tests for the per-repository exemption cap.
//!
//! `not_applicable` is the honest way to say a gate does not exist on a
//! project's stack. Unbounded, it is also the way to delete a gate: a repo
//! could exempt all of its checks and hold a green run. The owner's rule
//! (2026-10-02) is a per-manifest allowance of
//! `min(floor(0.25 x required_checks), 5)`, a warning at the allowance, a
//! blocking failure above it, and four concerns that are never exempable at
//! all. These tests pin the verdict on every surface: the stored run, the
//! human and JSON output, and the dry-run plan.

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
    let report = tmp.path().join("governance.json");
    std::fs::write(
        &report,
        r#"{"findings":[],"summary":{"errors":0,"warnings":0}}"#,
    )
    .unwrap();
    let manifest = format!(
        "{body}\n[[checks]]\nid = \"governance-quality\"\nrequired = false\ncommand = \"cat {}\"\n",
        report.display()
    );
    std::fs::write(tmp.path().join("gate.toml"), manifest).unwrap();
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

/// A `minimal` manifest with `required` bound required checks, then the
/// named exemptions. Exempted ids are declared without a command, which
/// is the only way to write an exemption.
fn manifest(required: usize, exempt: &[&str]) -> String {
    let mut out = String::from("profile = \"minimal\"\n");
    for i in 0..required {
        out.push_str(&format!(
            "\n[[checks]]\nid = \"c{i}\"\ncommand = \"true\"\n"
        ));
    }
    for id in exempt {
        out.push_str(&format!("\n[[checks]]\nid = \"{id}\"\n"));
    }
    if !exempt.is_empty() {
        out.push_str("\n[not_applicable]\n");
        for id in exempt {
            out.push_str(&format!(
                "{id} = \"no implementation of {id} on this stack\"\n"
            ));
        }
    }
    out
}

/// 8 required checks -> an allowance of 2. Named so each fixture states
/// the budget it is testing against.
const EIGHT_REQUIRED_CAP: usize = 2;

#[test]
fn exemptions_below_the_cap_are_acknowledged_and_change_nothing() {
    assert_eq!(
        driftwatchdog::gate::gate_exemption_cap(8),
        EIGHT_REQUIRED_CAP
    );
    let tmp = init_dir();
    write_manifest(&tmp, &manifest(8, &["opt-one"]));

    driftwatch()
        .arg("gate")
        .current_dir(tmp.path())
        .assert()
        .success();

    let run = latest_run(&tmp);
    assert_eq!(run.status, "PASS");
    let doc = run_doc(&tmp);
    let r = result_of(&doc, "opt-one");
    assert_eq!(r.status, GateStatus::NotApplicable);
    assert!(r.findings[0].title.contains("no implementation of opt-one"));
    // Below the cap there is nothing to report about the budget itself.
    assert!(
        doc.results.iter().all(|r| r.gate_id != "exemption-budget"),
        "an unused allowance must not add noise: {:?}",
        doc.results
    );
}

#[test]
fn exemptions_exactly_at_the_cap_warn_without_blocking() {
    // 8 required -> cap 2; two exemptions sit exactly on the ceiling.
    let tmp = init_dir();
    write_manifest(&tmp, &manifest(8, &["opt-one", "opt-two"]));

    driftwatch()
        .arg("gate")
        .current_dir(tmp.path())
        .assert()
        .success();

    let run = latest_run(&tmp);
    assert_eq!(run.status, "PASS", "the ceiling warns, it does not block");
    assert!(!run.blocked);

    let doc = run_doc(&tmp);
    let budget = result_of(&doc, "exemption-budget");
    assert_eq!(budget.status, GateStatus::ReviewRequired);
    assert_eq!(budget.severity, GateSeverity::Warning);
    let diag = budget.diagnostic.as_deref().unwrap();
    assert!(diag.contains("headroom is exhausted"), "{diag}");
    assert!(diag.contains("2 of 2 allowances used"), "{diag}");
    assert!(diag.contains("0 left"), "{diag}");
    // Both declarations remain acknowledged exemptions.
    assert_eq!(result_of(&doc, "opt-one").status, GateStatus::NotApplicable);
    assert_eq!(result_of(&doc, "opt-two").status, GateStatus::NotApplicable);
}

#[test]
fn exemptions_over_the_cap_fail_the_run() {
    // 8 bound + 3 exempted = 11 required -> cap 2; three exemptions is
    // one too many. The exempted checks are themselves required and
    // selected, so they count in the denominator — an exemption never
    // deselects a check.
    let tmp = init_dir();
    write_manifest(&tmp, &manifest(8, &["opt-one", "opt-two", "opt-three"]));

    driftwatch()
        .arg("gate")
        .current_dir(tmp.path())
        .assert()
        .failure();

    let run = latest_run(&tmp);
    assert_eq!(run.status, "FAIL");
    assert!(run.blocked);

    let doc = run_doc(&tmp);
    let budget = result_of(&doc, "exemption-budget");
    assert_eq!(budget.status, GateStatus::Fail);
    assert_eq!(budget.severity, GateSeverity::Error);
    let diag = budget.diagnostic.as_deref().unwrap();
    assert!(diag.contains("3 exemptions declared but only 2"), "{diag}");
    assert!(diag.contains("1 over"), "{diag}");
    assert!(
        diag.contains("min(floor(0.25 x 11 required checks), 5) = 2"),
        "{diag}"
    );
    // The other checks really did pass: this run is red on the budget
    // alone, which is the rule doing its job.
    assert_eq!(result_of(&doc, "c0").status, GateStatus::Pass);
}

#[test]
fn the_cap_does_not_depend_on_the_blocking_list() {
    let tmp = init_dir();
    write_manifest(
        &tmp,
        &format!(
            "{}[blocking]\nreview_required_blocks = false\n",
            manifest(8, &["opt-one", "opt-two", "opt-three"])
        ),
    );

    driftwatch()
        .arg("gate")
        .current_dir(tmp.path())
        .assert()
        .failure();
    let run = latest_run(&tmp);
    assert_eq!(run.status, "FAIL");
    assert!(run.blocked);
    assert!(
        run.results_json.contains("exemption-budget"),
        "the budget verdict is on the record, not just the exit code"
    );
}

/// The spine is closed for the canonical ids *and* for the workspace
/// alias `repository_integrity`. A project that spells the
/// repository-integrity concern differently must not be able to exempt
/// the concern the spine exists to protect.
///
/// Each entry is (id as the manifest writes it, the concern name the
/// diagnostic must report it under), because those differ for the one
/// aliased id.
#[test]
fn a_spine_check_may_never_be_exempted() {
    for (id, concern) in [
        ("build", "build"),
        ("tests", "tests"),
        ("security", "security"),
        ("repository", "repository"),
        ("repository_integrity", "repository"),
    ] {
        let tmp = init_dir();
        // 8 required -> cap 2, and exactly one exemption: inside the
        // numeric allowance, and still a hard failure.
        write_manifest(&tmp, &manifest(8, &[id]));

        driftwatch()
            .arg("gate")
            .current_dir(tmp.path())
            .assert()
            .failure();

        let run = latest_run(&tmp);
        assert_eq!(run.status, "FAIL", "{id} must not be exemptable");
        assert!(run.blocked, "{id} must block");

        let doc = run_doc(&tmp);
        // The exempted check is never recorded as an acknowledged
        // exemption: no surface may claim this absence is approved.
        let r = result_of(&doc, id);
        assert_eq!(r.status, GateStatus::Fail, "{id}");
        assert_eq!(r.severity, GateSeverity::Error, "{id}");
        assert_ne!(r.status, GateStatus::NotApplicable, "{id}");
        assert!(
            r.findings[0].title.contains("may never be exempted"),
            "{id}"
        );

        let budget = result_of(&doc, "exemption-budget");
        assert_eq!(budget.status, GateStatus::Fail, "{id}");
        let diag = budget.diagnostic.as_deref().unwrap();
        // The diagnostic names the offending id as written and the
        // concern it belongs to.
        assert!(diag.contains(id), "the budget names the offending id: {id}");
        // The diagnostic opens with the closed concern list, so the
        // concern name must appear as a whole element of that list --
        // not merely as a substring somewhere in the sentence.
        let listed = diag
            .split(" may never be exempted")
            .next()
            .unwrap_or_default()
            .split(", ")
            .collect::<Vec<_>>();
        assert!(
            listed.contains(&concern),
            "{id} is reported under its concern name: {diag}"
        );
    }
}

/// The alias is not cosmetic: a manifest that uses
/// `repository_integrity` and nothing else would be green if the matcher
/// compared only the canonical id. This pins the *absence* of that hole.
#[test]
fn the_repository_alias_is_not_exemptable_even_when_it_is_the_only_declaration() {
    let tmp = init_dir();
    // 4 required checks -> allowance 1, so a single declaration is
    // inside the cap. The spine, not the cap, is what fails this.
    write_manifest(&tmp, &manifest(4, &["repository_integrity"]));
    driftwatch()
        .arg("gate")
        .current_dir(tmp.path())
        .assert()
        .failure();

    let run = latest_run(&tmp);
    assert_eq!(run.status, "FAIL");
    assert!(run.blocked);
    let doc = run_doc(&tmp);
    let budget = result_of(&doc, "exemption-budget");
    assert_eq!(budget.status, GateStatus::Fail);
    assert_eq!(
        budget.findings[0].location.as_deref(),
        Some("not_applicable"),
        "the finding points at the exemption key"
    );
    assert_eq!(
        result_of(&doc, "repository_integrity").status,
        GateStatus::Fail
    );
}

/// A spine exemption fails even when the exempted concern was marked
/// optional, so `required = false` is not a way around the floor. The
/// verdict is carried by the plan-level budget check, which is required
/// regardless of how the exempted concern itself was declared.
#[test]
fn an_optional_spine_exemption_still_fails() {
    let tmp = init_dir();
    let mut body = manifest(8, &[]);
    body.push_str("\n[[checks]]\nid = \"security\"\nrequired = false\n");
    body.push_str("\n[not_applicable]\nsecurity = \"no security gate on this stack\"\n");
    write_manifest(&tmp, &body);

    driftwatch()
        .arg("gate")
        .current_dir(tmp.path())
        .assert()
        .failure();
    let run = latest_run(&tmp);
    assert_eq!(run.status, "FAIL");
    assert!(run.blocked);
    let doc = run_doc(&tmp);
    assert_eq!(result_of(&doc, "security").status, GateStatus::Fail);
    assert_eq!(result_of(&doc, "exemption-budget").status, GateStatus::Fail);
}

/// Fewer than four required checks floors to zero: a small manifest may
/// not exempt anything at all. Here that is 2 bound + 1 exempted = 3
/// required checks, so the allowance is zero and the single exemption is
/// over it.
#[test]
fn a_small_manifest_has_no_allowance() {
    assert_eq!(driftwatchdog::gate::gate_exemption_cap(3), 0);

    let tmp = init_dir();
    write_manifest(&tmp, &manifest(2, &["opt-one"]));

    driftwatch()
        .arg("gate")
        .current_dir(tmp.path())
        .assert()
        .failure();
    let run = latest_run(&tmp);
    assert_eq!(run.status, "FAIL");
    assert!(run.blocked);
    let diag = result_of(&run_doc(&tmp), "exemption-budget")
        .diagnostic
        .as_deref()
        .unwrap()
        .to_string();
    assert!(diag.contains("1 exemptions declared but only 0"), "{diag}");
}

/// Twenty or more required checks buys no wider hole: the absolute cap
/// binds, so a 40-check manifest has exactly the same allowance as a
/// 20-check one.
#[test]
fn a_large_manifest_buys_no_wider_hole() {
    assert_eq!(driftwatchdog::gate::gate_exemption_cap(20), 5);
    assert_eq!(driftwatchdog::gate::gate_exemption_cap(24), 5);
    assert_eq!(driftwatchdog::gate::gate_exemption_cap(40), 5);

    // 40 required -> cap 5; five sit on the ceiling (warning), six fail.
    let at_cap = init_dir();
    write_manifest(&at_cap, &manifest(40, &["e1", "e2", "e3", "e4", "e5"]));
    driftwatch()
        .arg("gate")
        .current_dir(at_cap.path())
        .assert()
        .success();
    assert_eq!(latest_run(&at_cap).status, "PASS");
    let at_cap_doc = run_doc(&at_cap);
    let budget = result_of(&at_cap_doc, "exemption-budget");
    assert_eq!(budget.status, GateStatus::ReviewRequired);
    assert!(budget
        .diagnostic
        .as_deref()
        .unwrap()
        .contains("min(floor(0.25 x 45 required checks), 5) = 5"));

    let over_cap = init_dir();
    write_manifest(
        &over_cap,
        &manifest(40, &["e1", "e2", "e3", "e4", "e5", "e6"]),
    );
    driftwatch()
        .arg("gate")
        .current_dir(over_cap.path())
        .assert()
        .failure();
    assert_eq!(latest_run(&over_cap).status, "FAIL");
}

/// A repo that has never used the exemption vocabulary is untouched:
/// same outcome, and no budget result anywhere.
#[test]
fn a_repo_with_no_exemptions_is_unchanged() {
    let tmp = init_dir();
    write_manifest(&tmp, &manifest(20, &[]));

    driftwatch()
        .arg("gate")
        .current_dir(tmp.path())
        .assert()
        .success();

    let run = latest_run(&tmp);
    assert_eq!(run.status, "PASS");
    assert!(!run.blocked);
    let doc = run_doc(&tmp);
    assert!(doc.unbound.is_empty());
    assert!(doc.results.iter().all(|r| r.gate_id != "exemption-budget"));
    assert_eq!(doc.results.len(), 21, "20 budget checks plus governance");
}

/// A project that declares its own concern named `exemption-budget`
/// must not be able to hide its own budget verdict behind the id
/// collision: the runtime's result takes a suffixed id instead.
#[test]
fn a_project_named_exemption_budget_cannot_hide_its_own_verdict() {
    let tmp = init_dir();
    // 2 bound + 1 declared `exemption-budget` check, itself exempted:
    // 3 required checks, so the allowance is 0 and the declaration is
    // over it.
    let mut body = manifest(2, &[]);
    body.push_str("\n[[checks]]\nid = \"exemption-budget\"\n");
    body.push_str("\n[not_applicable]\nexemption-budget = \"no budget concept in this project\"\n");
    write_manifest(&tmp, &body);

    driftwatch()
        .arg("gate")
        .current_dir(tmp.path())
        .assert()
        .failure();
    let run = latest_run(&tmp);
    assert_eq!(
        run.status, "FAIL",
        "the collision must not hide the verdict"
    );
    assert!(run.blocked);
    let doc = run_doc(&tmp);
    // The project's own check keeps its own result...
    assert_eq!(
        result_of(&doc, "exemption-budget").status,
        GateStatus::NotApplicable
    );
    // ...and the budget verdict lands on a distinct, suffixed id.
    assert_eq!(
        result_of(&doc, "exemption-budget-2").status,
        GateStatus::Fail
    );
}

#[test]
fn json_output_carries_the_budget_alongside_the_verdict() {
    let tmp = init_dir();
    write_manifest(&tmp, &manifest(8, &["opt-one", "opt-two", "opt-three"]));

    let json = driftwatch()
        .args(["gate", "--format", "json"])
        .current_dir(tmp.path())
        .output()
        .unwrap();
    let doc: serde_json::Value = serde_json::from_slice(&json.stdout).unwrap();
    assert_eq!(doc["status"], "FAIL");
    assert_eq!(doc["blocked"], true);
    assert_eq!(doc["exemption_budget"]["required_checks"], 11);
    assert_eq!(doc["exemption_budget"]["cap"], 2);
    assert_eq!(doc["exemption_budget"]["declared"], 3);
    assert!(doc["exemption_budget"]["non_exempable"]
        .as_array()
        .unwrap()
        .is_empty());
}

#[test]
fn dry_run_names_the_budget_without_executing() {
    let tmp = init_dir();
    write_manifest(&tmp, &manifest(8, &["opt-one", "opt-two", "opt-three"]));

    let out = driftwatch()
        .args(["gate", "--dry-run"])
        .current_dir(tmp.path())
        .output()
        .unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("exemption budget: 3 of 2 allowances used"),
        "{stdout}"
    );
    assert!(
        stdout.contains("verdict: 3 exemptions declared but only 2 are allowed"),
        "{stdout}"
    );
    // Pure: nothing executed, so no run was persisted.
    let db = Db::open_read_only(&tmp.path().join(".driftwatch/state.db")).unwrap();
    assert_eq!(Gates::new(&db).count().unwrap(), 0);
}

/// The YAML business form carries the same rule, because both manifest
/// forms resolve through the same budget.
#[test]
fn yaml_manifests_honour_the_same_budget() {
    let tmp = init_dir();
    let report = tmp.path().join("governance.json");
    std::fs::write(
        &report,
        r#"{"findings":[],"summary":{"errors":0,"warnings":0}}"#,
    )
    .unwrap();
    std::fs::create_dir_all(tmp.path().join(".ai-gate")).unwrap();
    let mut body = format!("version: 1\nruntime: driftwatchdog\nprofile: minimal\nchecks:\n  c0: true\n  c1: true\n  c2: true\n  c3: true\n  c4: true\n  c5: true\n  c6: true\n  c7: true\n  e1: true\n  governance-quality: optional\ncommands:\n  c0: \"true\"\n  c1: \"true\"\n  c2: \"true\"\n  c3: \"true\"\n  c4: \"true\"\n  c5: \"true\"\n  c6: \"true\"\n  c7: \"true\"\n  governance-quality: \"cat {}\"\n", report.display());
    // 9 required -> cap 2. One exemption is below it; two is on it.
    body.push_str("not_applicable:\n  e1: \"no e1 on this stack\"\n");
    std::fs::write(tmp.path().join(".ai-gate/gate.yaml"), body).unwrap();
    driftwatch()
        .arg("gate")
        .current_dir(tmp.path())
        .assert()
        .success();
    let doc = run_doc(&tmp);
    assert_eq!(result_of(&doc, "e1").status, GateStatus::NotApplicable);
    assert!(doc.results.iter().all(|r| r.gate_id != "exemption-budget"));
    assert_eq!(
        latest_run(&tmp).status,
        "PASS",
        "one exemption under the cap is acknowledged"
    );
}
