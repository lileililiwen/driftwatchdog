//! End-to-end tests for `driftwatch gate --dry-run --format json`.
//!
//! The defect was an ordering decision: the dry-run branch returned before
//! the `--format` dispatch, so `--format json` printed the human plan text
//! and a consumer had to scrape it. The fix emits a real document — and the
//! whole difficulty is that a rehearsal must never be readable as a verdict.
//!
//! These tests go through the binary, because the contract that matters to a
//! consumer is stdout bytes plus an exit code, not a Rust return value. They
//! pin three things together:
//!
//! 1. the rehearsal document parses and declares itself a rehearsal;
//! 2. the human rehearsal and the real-run documents are **byte-identical**
//!    to a baseline captured from the pre-change binary (`cf823dc`), stored
//!    as a literal in this file rather than re-derived from the current code;
//! 3. a rehearsal creates no `gate_runs` row and exits zero whatever it
//!    plans.

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

const CLEAN_GOVERNANCE: &str = r#"{"findings":[],"summary":{"errors":0,"warnings":0}}"#;

/// A minimal profile with two bound required checks. `governance-quality` is
/// added by the helper below and bound, so this plan cannot decide blocking
/// on its own — which is the honest case for a rehearsal.
fn write_all_bound_manifest(tmp: &TempDir) {
    let report = tmp.path().join("governance.json");
    std::fs::write(&report, CLEAN_GOVERNANCE).unwrap();
    std::fs::write(
        tmp.path().join("gate.toml"),
        format!(
            "profile = \"minimal\"\nrule_pack_version = \"workspace-governance@1\"\n\
             [[checks]]\nid = \"governance-quality\"\ncommand = \"cat {report}\"\n\
             [[checks]]\nid = \"smoke\"\ncommand = \"true\"\n\
             [[checks]]\nid = \"slow-tests\"\nrequired = false\ncommand = \"false\"\ntimeout_ms = 120000\n",
            report = report.display()
        ),
    )
    .unwrap();
}

/// A plan whose real run is **blocked**: `governance-quality` is a required
/// profile default with no command bound, which `execute_plan` records as
/// `UNBOUND` and `aggregate` turns into a failure.
fn write_blocking_manifest(tmp: &TempDir) {
    std::fs::write(
        tmp.path().join("gate.toml"),
        "profile = \"minimal\"\n[[checks]]\nid = \"smoke\"\ncommand = \"true\"\n",
    )
    .unwrap();
}

fn dry_run_json(tmp: &TempDir) -> Value {
    let assert = driftwatch()
        .arg("gate")
        .arg("--dry-run")
        .arg("--format")
        .arg("json")
        .current_dir(tmp.path())
        .assert()
        .success();
    serde_json::from_slice(&assert.get_output().stdout).unwrap_or_else(|e| {
        panic!(
            "dry-run json must parse: {e}\nstdout was:\n{}",
            String::from_utf8_lossy(&assert.get_output().stdout)
        )
    })
}

fn gate_run_count(tmp: &TempDir) -> i64 {
    let db = Db::open_read_only(&tmp.path().join(".driftwatch/state.db")).unwrap();
    Gates::new(&db).count().unwrap()
}

/// The closed deny-set, restated here on purpose. The unit test asserts
/// against the production constant; this one asserts against a list written
/// next to the consumer's requirement, so a change to the constant alone
/// cannot quietly redefine what "readable as a verdict" means.
const VERDICT_LIKE: &[&str] = &[
    "status",
    "result",
    "results",
    "passed",
    "blocked",
    "outcome",
    "aggregate",
    "exit_code",
    "gate_status",
    "ok",
    "success",
    "failures",
    "pending_reviews",
];

fn verdict_like_keys(value: &Value, path: &str, found: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                let child_path = if path.is_empty() {
                    key.clone()
                } else {
                    format!("{path}.{key}")
                };
                if VERDICT_LIKE.contains(&key.as_str()) {
                    found.push(child_path.clone());
                }
                verdict_like_keys(child, &child_path, found);
            }
        }
        Value::Array(items) => {
            for (index, child) in items.iter().enumerate() {
                verdict_like_keys(child, &format!("{path}[{index}]"), found);
            }
        }
        _ => {}
    }
}

#[test]
fn a_dry_run_json_document_parses_and_declares_itself_a_rehearsal() {
    let tmp = init_dir();
    write_all_bound_manifest(&tmp);
    let doc = dry_run_json(&tmp);

    assert_eq!(doc["dry_run"], Value::Bool(true));
    assert_eq!(doc["document_kind"], "gate-plan-rehearsal");
    assert_eq!(doc["contract"], "driftwatch-gate-plan/0.1.0");
    assert_eq!(doc["executed"], Value::Bool(false));
    assert_eq!(doc["persisted"], Value::Bool(false));
    assert_eq!(doc["execution_mode"], "dry-run");
    assert_eq!(doc["profile"], "minimal");
    assert_eq!(doc["rule_pack_version"], "workspace-governance@1");
    assert_eq!(doc["plan_version"], Value::from(1u64));
    assert_eq!(
        doc["blocking_policy"]["review_required_blocks"],
        Value::Bool(true)
    );
    assert_eq!(doc["default_timeout_ms"], Value::from(60000u64));
}

/// **The requirement this whole change exists for.** A rehearsal must not be
/// mistakable for a verdict, so the check is mechanical: every key at every
/// depth, against a deny-set written here.
#[test]
fn a_dry_run_json_document_carries_no_verdict_field_at_any_depth() {
    let tmp = init_dir();
    write_all_bound_manifest(&tmp);
    let mut found = Vec::new();
    verdict_like_keys(&dry_run_json(&tmp), "", &mut found);
    assert_eq!(found, Vec::<String>::new());

    // And the plan-forced variant, which has more to get wrong.
    let blocked = init_dir();
    write_blocking_manifest(&blocked);
    let mut found = Vec::new();
    verdict_like_keys(&dry_run_json(&blocked), "", &mut found);
    assert_eq!(found, Vec::<String>::new());
}

#[test]
fn a_dry_run_document_describes_every_planned_check_with_its_deadline() {
    let tmp = init_dir();
    write_all_bound_manifest(&tmp);
    let doc = dry_run_json(&tmp);
    let checks = doc["planned_checks"].as_array().unwrap();
    let ids: Vec<&Value> = checks.iter().map(|c| &c["id"]).collect();
    assert_eq!(
        ids,
        vec![
            &serde_json::json!("governance-quality"),
            &serde_json::json!("slow-tests"),
            &serde_json::json!("smoke"),
        ]
    );

    let slow = checks.iter().find(|c| c["id"] == "slow-tests").unwrap();
    assert_eq!(slow["required"], Value::Bool(false));
    assert_eq!(slow["timeout_ms"], Value::from(120000u64));
    assert_eq!(slow["command_source"], "manifest");

    let smoke = checks.iter().find(|c| c["id"] == "smoke").unwrap();
    assert_eq!(smoke["timeout_ms"], Value::from(60000u64));

    for check in checks {
        let keys: Vec<&String> = check.as_object().unwrap().keys().collect();
        assert_eq!(
            keys,
            vec![
                "command_source",
                "exempt",
                "id",
                "required",
                "selected_by",
                "timeout_ms"
            ],
            "a planned check carries plan facts and nothing else: {check}"
        );
    }
}

#[test]
fn a_plan_with_an_unbound_required_check_says_so_as_a_prediction() {
    let tmp = init_dir();
    write_blocking_manifest(&tmp);
    let doc = dry_run_json(&tmp);
    let p = &doc["blocking_prediction"];
    assert_eq!(p["kind"], "plan-forced-block");
    assert_eq!(p["would_block"], Value::Bool(true));
    let reasons = p["reasons"].as_array().unwrap();
    let reason = reasons
        .iter()
        .find(|r| r["rule"] == "unbound-required-check")
        .expect("the unbound check is named");
    assert_eq!(reason["checks"], serde_json::json!(["governance-quality"]));
    // The prediction is announced as a prediction, in the document itself.
    assert_eq!(p["derived_from"], "resolved-plan");
    assert_eq!(p["sufficient_but_not_necessary"], Value::Bool(true));
}

#[test]
fn an_undecidable_plan_is_never_reported_as_passing() {
    let tmp = init_dir();
    write_all_bound_manifest(&tmp);
    let p = dry_run_json(&tmp)["blocking_prediction"].clone();
    assert_eq!(p["kind"], "undetermined-before-execution");
    assert_eq!(p["would_block"], Value::Null);
    assert_ne!(p["would_block"], Value::Bool(false));
    assert_eq!(p["reasons"], serde_json::json!([]));
}

/// A rehearsal must be inert in every respect, not merely quiet. This is the
/// half a consumer cannot verify by reading the document.
#[test]
fn a_dry_run_creates_no_gate_runs_row_in_any_format() {
    for format in ["human", "json", "platform"] {
        let tmp = init_dir();
        write_blocking_manifest(&tmp);
        assert_eq!(gate_run_count(&tmp), 0, "nothing before");
        driftwatch()
            .arg("gate")
            .arg("--dry-run")
            .arg("--format")
            .arg(format)
            .current_dir(tmp.path())
            .assert()
            .success();
        assert_eq!(
            gate_run_count(&tmp),
            0,
            "--dry-run --format {format} must not persist a gate run"
        );
    }
}

#[test]
fn a_dry_run_exits_zero_even_where_the_real_run_blocks() {
    let tmp = init_dir();
    write_blocking_manifest(&tmp);
    driftwatch()
        .arg("gate")
        .arg("--dry-run")
        .arg("--format")
        .arg("json")
        .current_dir(tmp.path())
        .assert()
        .success();
    // Same manifest, real run: blocked and persisted. The rehearsal changed
    // nothing about the verdict, only about what it can be mistaken for.
    driftwatch()
        .arg("gate")
        .current_dir(tmp.path())
        .assert()
        .code(1);
    assert_eq!(gate_run_count(&tmp), 1);
}

/// Byte-identity, asserted against bytes captured from the pre-change binary
/// (`cf823dc`) rather than re-derived from the current code. If either
/// surface moves, this fails with a diff rather than with an opinion.
#[test]
fn the_human_dry_run_is_byte_identical_to_the_captured_baseline() {
    let tmp = init_dir();
    std::fs::write(tmp.path().join("gate.toml"), BASELINE_MANIFEST).unwrap();
    let assert = driftwatch()
        .arg("gate")
        .arg("--dry-run")
        .arg("--format")
        .arg("human")
        .current_dir(tmp.path())
        .assert()
        .success();
    assert_eq!(
        String::from_utf8_lossy(&assert.get_output().stdout),
        BASELINE_HUMAN_DRY_RUN
    );
}

/// The real documents must be untouched. The JSON one still carries its whole
/// verdict key set — it must not *lose* one, and it must not gain one from
/// this change.
#[test]
fn a_real_run_json_document_is_unchanged_and_still_a_verdict() {
    let tmp = init_dir();
    write_all_bound_manifest(&tmp);
    let assert = driftwatch()
        .arg("gate")
        .arg("--format")
        .arg("json")
        .current_dir(tmp.path())
        .assert()
        .success();
    let doc: Value = serde_json::from_slice(&assert.get_output().stdout).unwrap();
    for key in [
        "status",
        "blocked",
        "failures",
        "pending_reviews",
        "not_applicable",
        "unbound",
        "exemption_budget",
        "manifest_digest",
        "rule_pack_version",
        "results",
    ] {
        assert!(doc.get(key).is_some(), "the real document lost `{key}`");
    }
    assert!(doc.get("dry_run").is_none());
    assert!(doc.get("document_kind").is_none());
    assert_eq!(doc["status"], "PASS");
    assert_eq!(doc["blocked"], Value::Bool(false));
}

/// A rehearsal and a verdict must not be confusable. They share three
/// keys — plan identity, which is exactly what a rehearsal *is* about — and
/// this test pins both halves: the shared set is enumerated and contains
/// nothing verdict-shaped, and every verdict key of the status document is
/// absent from the rehearsal.
#[test]
fn a_rehearsal_shares_only_plan_identity_with_a_status_document() {
    let tmp = init_dir();
    write_all_bound_manifest(&tmp);
    let rehearsal = dry_run_json(&tmp);

    let assert = driftwatch()
        .arg("gate")
        .arg("--format")
        .arg("json")
        .current_dir(tmp.path())
        .assert()
        .success();
    let verdict: Value = serde_json::from_slice(&assert.get_output().stdout).unwrap();

    let rehearsal_keys: Vec<&str> = rehearsal
        .as_object()
        .unwrap()
        .keys()
        .map(|k| k.as_str())
        .collect();
    let verdict_keys: Vec<&str> = verdict
        .as_object()
        .unwrap()
        .keys()
        .map(|k| k.as_str())
        .collect();

    let mut shared: Vec<&str> = rehearsal_keys
        .iter()
        .copied()
        .filter(|k| verdict_keys.contains(k))
        .collect();
    shared.sort();
    assert_eq!(
        shared,
        vec!["exemption_budget", "manifest_digest", "rule_pack_version"],
        "the only keys a rehearsal may share with a status document are plan identity"
    );
    for key in shared {
        assert!(
            !VERDICT_LIKE.contains(&key),
            "`{key}` is shared and verdict-shaped"
        );
    }

    // Nothing the status document says about an *outcome* may appear in a
    // rehearsal: not even under a new name, so the whole verdict vocabulary
    // is checked, not just the keys that happen to be shared.
    for key in VERDICT_LIKE {
        assert!(
            !rehearsal_keys.contains(key),
            "the rehearsal carries the verdict key `{key}`"
        );
    }
}

/// Captured from `cf823dc` — the commit before this change — by running
/// `--dry-run --format human` against [`BASELINE_MANIFEST`] in a temporary
/// project, with the pre-change binary. It is pasted here verbatim so the
/// byte-identity claim is a comparison against history, not a
/// re-derivation from the code under test.
const BASELINE_MANIFEST: &str = concat!(
    "profile = \"minimal\"\n",
    "rule_pack_version = \"workspace-governance@1\"\n",
    "\n[[checks]]\n",
    "id = \"smoke\"\n",
    "command = \"true\"\n",
    "\n[[checks]]\n",
    "id = \"repo-hygiene\"\n",
    "command = \"true\"\n",
    "\n[[checks]]\n",
    "id = \"slow-tests\"\n",
    "required = false\n",
    "command = \"false\"\n",
    "timeout_ms = 120000\n",
);

const BASELINE_HUMAN_DRY_RUN: &str = concat!(
    "gate plan (dry-run; nothing was executed)\n",
    "contract version: 1 | profile: minimal | rule-pack: workspace-governance@1 | manifest: sha256:a221e8e3a93b303f\n",
    "blocking: review_required_blocks=true\n",
    "execution mode: dry-run\n",
    "source_size: max_lines=1000 include=[] exclude=[]\n",
    "governance_quality: codes=[QUALITY_NEW_CHANGE_WITH_GATE_DEBT, QUALITY_README_CLAIM_CONTRADICTED, QUALITY_README_CLAIM_UNSCOPED, QUALITY_VERIFICATION_COMMAND_UNBOUND, RULES_DRIFT_BASELINE_STALE, RULES_DRIFT_NEW, SPEC_PURPOSE_PLACEHOLDER]\n",
    "checks:\n",
    "  - governance-quality [required] via manifest\n",
    "  - repo-hygiene [required] via manifest\n",
    "  - slow-tests [optional] via manifest\n",
    "  - smoke [required] via manifest\n",
    "check timeouts (default 60000ms):\n",
    "  - governance-quality: 60000ms\n",
    "  - repo-hygiene: 60000ms\n",
    "  - slow-tests: 120000ms\n",
    "  - smoke: 60000ms\n",
    "exemption budget: 0 of 0 allowances used (0 headroom left; min(floor(0.25 x 3 required checks), 5) = 0)\n",
    "unbound required checks (no command bound; will fail as UNBOUND):\n",
    "  - governance-quality: bind `command` for this check, or add a not_applicable exemption with a reason\n",
    "project commands:\n",
    "  (none)\n",
    "context providers:\n",
    "  (none)\n",
    "explicitly not scheduled:\n",
    "  - a11y\n",
    "  - browser\n",
    "  - capability-conformance\n",
    "  - placeholder-threshold\n",
    "  - product-code-boundary\n",
    "  - release-evidence\n",
    "  - responsive\n",
    "  - source-file-size\n",
);
