//! Unit tests for [`crate::gate::dry_run_plan`] — the `--dry-run`
//! rehearsal document.
//!
//! Split out with `#[path = "dry_run_plan_tests.rs"] mod tests;` for the
//! same reason `platform_result_tests.rs` exists: the source-file-size
//! boundary. The tests that matter most here are the negative controls — a
//! document that could be read as a verdict is the failure this change
//! exists to prevent, so "no verdict field at any depth" is asserted
//! mechanically rather than by reading the JSON.

use super::*;
use crate::gate::manifest::{GateManifest, ManifestCheck, EXEMPTION_BUDGET_CHECK};

/// Write a manifest and resolve it, the same two steps `gate()` takes.
fn resolve(toml: &str) -> (ResolvedGatePlan, GateManifest) {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(dir.path().join("gate.toml"), toml).expect("write manifest");
    let manifest: GateManifest =
        toml::from_str(toml).expect("manifest parses (checked by manifest tests)");
    let resolved = crate::gate::manifest::resolve(&manifest, &[]).expect("plan resolves");
    (resolved, manifest)
}

fn doc(toml: &str) -> serde_json::Value {
    let (resolved, manifest) = resolve(toml);
    build_document(&resolved, &manifest)
}

/// Three required checks, two bound, `governance-quality` (a profile default
/// and never exempable) with no command — the shape that forces a block.
const ONE_UNBOUND: &str = r#"
profile = "minimal"
[[checks]]
id = "smoke"
command = "true"
[[checks]]
id = "repo-hygiene"
command = "true"
"#;

/// Everything bound and inside the allowance: the plan cannot decide.
const ALL_BOUND: &str = r#"
profile = "minimal"
[[checks]]
id = "governance-quality"
command = "echo ok"
[[checks]]
id = "smoke"
command = "true"
"#;

#[test]
fn a_rehearsal_document_round_trips_and_declares_itself_three_ways() {
    let value = doc(ONE_UNBOUND);
    let parsed: serde_json::Value =
        serde_json::from_str(&serde_json::to_string(&value).unwrap()).expect("round-trips");

    assert_eq!(parsed["dry_run"], serde_json::json!(true));
    assert_eq!(parsed["document_kind"], DRY_RUN_DOCUMENT_KIND);
    assert_eq!(parsed["contract"], DRY_RUN_PLAN_CONTRACT);
    assert_eq!(parsed["executed"], serde_json::json!(false));
    assert_eq!(parsed["persisted"], serde_json::json!(false));
    assert_eq!(parsed["execution_mode"], "dry-run");
    // The declaration is one constant, not a literal repeated at the site.
    assert_eq!(DRY_RUN_PLAN_CONTRACT, "driftwatch-gate-plan/0.1.0");
    assert_eq!(DRY_RUN_DOCUMENT_KIND, "gate-plan-rehearsal");
}

#[test]
fn a_rehearsal_document_carries_the_identity_a_rehearsal_is_for() {
    let value = doc(ONE_UNBOUND);
    assert_eq!(value["profile"], "minimal");
    assert_eq!(value["plan_version"], serde_json::json!(1));
    assert_eq!(value["rule_pack_version"], "local");
    assert!(value["manifest_digest"]
        .as_str()
        .unwrap()
        .starts_with("sha256:"));
    assert_eq!(
        value["blocking_policy"]["review_required_blocks"],
        serde_json::json!(true)
    );
    assert_eq!(value["default_timeout_ms"], serde_json::json!(60000));
}

/// **The negative control that matters.** Every key, at every depth, checked
/// against the closed deny-set. Sabotage the composition by adding a
/// `"status"` or a `"blocked"` anywhere and this goes red with the path.
#[test]
fn a_rehearsal_document_carries_no_verdict_field_at_any_depth() {
    for (name, toml) in [
        ("one unbound", ONE_UNBOUND),
        ("all bound", ALL_BOUND),
        ("profile with an in-process concern", SOURCE_SIZE_MANIFEST),
    ] {
        let value = doc(toml);
        assert_eq!(
            verdict_like_keys(&value),
            Vec::<String>::new(),
            "the {name} rehearsal document is readable as a verdict"
        );
    }
}

/// The walk itself must have teeth, or the assertion above is vacuous.
#[test]
fn the_verdict_key_walk_finds_a_verdict_key_at_any_depth() {
    let planted = serde_json::json!({
        "contract": "x",
        "planned_checks": [ { "id": "a", "status": "PASS" } ],
        "nested": { "deep": [ { "deeper": { "blocked": true } } ] }
    });
    let mut found = verdict_like_keys(&planted);
    found.sort();
    assert_eq!(
        found,
        vec![
            "nested.deep[0].deeper.blocked".to_string(),
            "planned_checks[0].status".to_string(),
        ]
    );
    assert!(VERDICT_LIKE_KEYS.contains(&"passed"));
    assert!(VERDICT_LIKE_KEYS.contains(&"outcome"));
    assert!(
        !VERDICT_LIKE_KEYS.contains(&"would_block"),
        "the one blocking field is deliberately outside the deny-set and must stay so"
    );
}

#[test]
fn a_rehearsal_never_predicts_a_pass() {
    // `WouldBlock` has no third variant and `false` has no serialization, so
    // this is a property of the type. Assert the serialization anyway, so a
    // future variant cannot slip in unnoticed.
    assert_eq!(WouldBlock::Forced.as_value(), serde_json::json!(true));
    assert_eq!(WouldBlock::Undetermined.as_value(), serde_json::Value::Null);
    for toml in [ONE_UNBOUND, ALL_BOUND, SOURCE_SIZE_MANIFEST] {
        let value = doc(toml);
        assert_ne!(
            value["blocking_prediction"]["would_block"],
            serde_json::json!(false),
            "a rehearsal must never claim the gate will not block"
        );
    }
}

#[test]
fn a_required_unbound_check_forces_a_block_prediction() {
    let value = doc(ONE_UNBOUND);
    let p = &value["blocking_prediction"];
    assert_eq!(p["kind"], "plan-forced-block");
    assert_eq!(p["would_block"], serde_json::json!(true));
    assert_eq!(p["derived_from"], "resolved-plan");
    assert_eq!(p["sufficient_but_not_necessary"], serde_json::json!(true));
    let reason = p["reasons"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["rule"] == "unbound-required-check")
        .expect("the unbound check is named as a forcing reason");
    assert_eq!(reason["checks"], serde_json::json!(["governance-quality"]));
}

#[test]
fn an_all_bound_plan_is_undetermined_not_passing() {
    let value = doc(ALL_BOUND);
    let p = &value["blocking_prediction"];
    assert_eq!(p["kind"], "undetermined-before-execution");
    assert_eq!(p["would_block"], serde_json::Value::Null);
    assert_eq!(p["reasons"], serde_json::json!([]));
    assert!(p["note"]
        .as_str()
        .unwrap()
        .contains("unknown, never a pass"));
}

#[test]
fn an_over_cap_exemption_budget_forces_a_block_prediction() {
    // Six required checks give an allowance of `min(floor(0.25 x 6), 5) = 1`.
    // Three *optional* checks are declared, unbound and exempted: they
    // consume the allowance without becoming coverage debt, so the only
    // forcing reason this plan has is the budget.
    let mut manifest = String::from(
        "profile = \"minimal\"\n[[checks]]\nid = \"governance-quality\"\ncommand = \"echo ok\"\n",
    );
    for i in 0..5 {
        manifest.push_str(&format!("[[checks]]\nid = \"c{i}\"\ncommand = \"true\"\n"));
    }
    for id in ["o1", "o2", "o3"] {
        manifest.push_str(&format!("[[checks]]\nid = \"{id}\"\nrequired = false\n"));
    }
    manifest.push_str("[not_applicable]\no1 = \"nothing to run\"\no2 = \"nothing to run\"\no3 = \"nothing to run\"\n");
    let value = doc(&manifest);
    assert_eq!(value["exemption_budget"]["declared"], serde_json::json!(3));
    assert_eq!(value["exemption_budget"]["cap"], serde_json::json!(1));
    assert_eq!(value["exemption_budget"]["verdict"], "over-cap");
    // The other arm is silent here, so the prediction below is the budget's.
    assert_eq!(value["unbound_required_checks"], serde_json::json!([]));
    let p = &value["blocking_prediction"];
    assert_eq!(p["would_block"], serde_json::json!(true));
    let rule = p["reasons"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["rule"] == "exemption-budget-exceeded")
        .expect("the budget is named");
    assert_eq!(rule["checks"], serde_json::json!([EXEMPTION_BUDGET_CHECK]));
}

/// Exempting a never-exempable concern is a `hard_fails` budget whatever the
/// counts say, so it must force a block through the same arm.
#[test]
fn exempting_a_never_exempable_concern_forces_a_block_prediction() {
    let value = doc(r#"
profile = "minimal"
[[checks]]
id = "governance-quality"
command = "echo ok"
[[checks]]
id = "smoke"
command = "true"
[[checks]]
id = "tests"
required = false
[not_applicable]
tests = "no test suite here"
"#);
    assert_eq!(value["exemption_budget"]["verdict"], "hard-fail");
    assert_eq!(
        value["exemption_budget"]["never_exempable"],
        serde_json::json!(["tests"])
    );
    assert_eq!(
        value["blocking_prediction"]["would_block"],
        serde_json::json!(true)
    );
}

/// The budget arm must not fire for an exhausted-but-not-exceeded budget,
/// because an `Exhausted` verdict becomes an *optional* review that never
/// blocks. If this test fails, the rehearsal is predicting a block the real
/// run would not report.
#[test]
fn an_exhausted_budget_does_not_force_a_block() {
    // Four required checks => allowance of one. One exemption is exactly at
    // the allowance: `exhausted`, never blocking.
    let mut manifest = String::from("profile = \"minimal\"\n");
    for id in ["c0", "c1"] {
        manifest.push_str(&format!("[[checks]]\nid = \"{id}\"\ncommand = \"true\"\n"));
    }
    manifest.push_str(
        "[[checks]]\nid = \"governance-quality\"\ncommand = \"echo ok\"\n\
         [[checks]]\nid = \"repo-hygiene\"\ncommand = \"true\"\n\
         [[checks]]\nid = \"ux-review\"\n[not_applicable]\nux-review = \"no UI surface\"\n",
    );
    let value = doc(&manifest);
    assert_eq!(value["exemption_budget"]["verdict"], "exhausted");
    assert_eq!(value["exemption_budget"]["headroom"], serde_json::json!(0));
    assert_eq!(
        value["blocking_prediction"]["would_block"],
        serde_json::Value::Null
    );
}

#[test]
fn planned_checks_carry_no_outcome_and_a_resolved_deadline() {
    let value = doc(r#"
profile = "minimal"
[[checks]]
id = "smoke"
command = "true"
timeout_ms = 120000
[[checks]]
id = "repo-hygiene"
required = false
command = "true"
"#);
    let checks = value["planned_checks"].as_array().unwrap();
    let smoke = checks.iter().find(|c| c["id"] == "smoke").unwrap();
    assert_eq!(smoke["required"], serde_json::json!(true));
    assert_eq!(smoke["exempt"], serde_json::json!(false));
    assert_eq!(smoke["command_source"], "manifest");
    assert_eq!(smoke["timeout_ms"], serde_json::json!(120000));

    let optional = checks.iter().find(|c| c["id"] == "repo-hygiene").unwrap();
    assert_eq!(optional["required"], serde_json::json!(false));
    // No declaration, so the plan default resolves — and the default is
    // still named at the root, so a reader sees it is a default.
    assert_eq!(optional["timeout_ms"], serde_json::json!(60000));

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
            "a planned check row must be exactly the plan facts"
        );
    }
}

/// The in-process concern needs no command binding, and the document must say
/// so — otherwise a rehearsal would report coverage debt for a check the
/// runtime runs itself.
#[test]
fn an_in_process_concern_is_named_as_such_and_is_not_coverage_debt() {
    let value = doc(SOURCE_SIZE_MANIFEST);
    let row = value["planned_checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == "source-file-size")
        .expect("the profile schedules source-file-size");
    assert_eq!(row["command_source"], "in-process");
    let unbound = value["unbound_required_checks"].as_array().unwrap();
    assert!(
        !unbound.contains(&serde_json::json!("source-file-size")),
        "an in-process concern is not coverage debt: {unbound:?}"
    );
    assert!(
        unbound.contains(&serde_json::json!("placeholder-threshold")),
        "a concern the runtime runs through a project command with none bound \
         really is coverage debt: {unbound:?}"
    );
}

#[test]
fn exempted_checks_and_unscheduled_concerns_are_plan_inputs() {
    let mut manifest = String::from(
        "profile = \"minimal\"\n[[checks]]\nid = \"governance-quality\"\ncommand = \"echo ok\"\n",
    );
    for i in 0..7 {
        manifest.push_str(&format!("[[checks]]\nid = \"c{i}\"\ncommand = \"true\"\n"));
    }
    manifest.push_str("[[checks]]\nid = \"ux-review\"\n[not_applicable]\nux-review = \"no UI surface in this project\"\n");
    let value = doc(&manifest);
    assert_eq!(
        value["exempted_checks"]["ux-review"],
        "no UI surface in this project"
    );
    let not_scheduled = value["not_scheduled"].as_array().unwrap();
    assert!(not_scheduled.contains(&serde_json::json!("source-file-size")));
    assert!(!not_scheduled.contains(&serde_json::json!("smoke")));
}

#[test]
fn the_document_omits_command_text_project_command_values_and_a_clock() {
    let (resolved, manifest) = resolve(
        r#"
profile = "minimal"
[project_commands]
lint = "cargo clippy -- -D warnings"
[[checks]]
id = "smoke"
command = "echo a-very-recognisable-command-body"
"#,
    );
    let rendered = serde_json::to_string(&build_document(&resolved, &manifest)).unwrap();
    assert!(
        !rendered.contains("a-very-recognisable-command-body"),
        "a check command is project-authored text that can carry credentials; \
         the rehearsal names its source instead: {rendered}"
    );
    assert!(
        !rendered.contains("-- -D warnings"),
        "a project command's value is project-authored text: {rendered}"
    );
    // The name is still there — the document reports what exists, not its body.
    assert!(rendered.contains("lint"));
    assert!(!rendered.contains("evaluated_at"));
    assert!(!rendered.contains("timestamp"));
    assert!(!rendered.contains("revision"));
}

#[test]
fn source_size_and_governance_policies_are_carried_so_a_rehearsal_is_complete() {
    let value = doc(SOURCE_SIZE_MANIFEST);
    assert_eq!(
        value["source_size_policy"]["max_lines"],
        serde_json::json!(1000)
    );
    let codes = value["governance_quality_codes"].as_array().unwrap();
    assert!(
        codes.contains(&serde_json::json!("QUALITY_NEW_CHANGE_WITH_GATE_DEBT")),
        "the resolved code scope must be visible before anything runs"
    );
}

/// `--format platform` with `--dry-run` prints the plan text and stays silent
/// about the outcome contract. That combination is unchanged on purpose
/// (design §5) and this test is what stops it from being re-broken or
/// "fixed" into a new semantic by a later reader.
#[test]
fn platform_format_with_dry_run_still_renders_the_plan_text() {
    let (resolved, manifest) = resolve(ONE_UNBOUND);
    let expected = DryRunOutput::Plan(crate::gate::manifest::render_plan(&resolved));
    assert!(
        matches!(&expected, DryRunOutput::Plan(p) if p.starts_with("gate plan (dry-run; nothing was executed)"))
    );
    assert_eq!(
        dry_run_output(&GateOutputFormat::Platform, &resolved, &manifest),
        expected
    );
    assert_eq!(
        dry_run_output(&GateOutputFormat::Human, &resolved, &manifest),
        expected,
        "the human rehearsal is the plan render, exactly"
    );
}

/// The machine-readable surface must differ from the human one by exactly the
/// thing the defect was: `--format json` used to print the plan text. If this
/// fails, `print_dry_run` has routed `Json` to the human renderer again.
#[test]
fn the_json_rehearsal_is_not_the_human_plan_text() {
    let (resolved, manifest) = resolve(ONE_UNBOUND);
    let output = dry_run_output(&GateOutputFormat::Json, &resolved, &manifest);
    let rendered = match output {
        DryRunOutput::Document(doc) => serde_json::to_string(&doc).unwrap(),
        DryRunOutput::Plan(plan) => panic!("--format json must render a document, got: {plan}"),
    };
    assert_ne!(rendered, crate::gate::manifest::render_plan(&resolved));
    assert!(serde_json::from_str::<serde_json::Value>(&rendered).is_ok());
}

const SOURCE_SIZE_MANIFEST: &str = r#"
profile = "rust-product"
[[checks]]
id = "governance-quality"
command = "echo ok"
"#;

/// A declaration with `enabled = false` is removed from the plan entirely, so
/// the document must not schedule it and must not let its `command` bind
/// anything. The second half is what makes the check worth having: a disabled
/// declaration that still satisfied `command_source` would make a rehearsal
/// claim coverage the real run would not have.
#[test]
fn a_disabled_declaration_is_not_scheduled_and_binds_nothing() {
    let checks = vec![
        ManifestCheck {
            id: "repo-hygiene".to_string(),
            required: true,
            enabled: false,
            command: Some("true".to_string()),
            timeout_ms: None,
            source: None,
        },
        ManifestCheck {
            id: "smoke".to_string(),
            required: true,
            enabled: true,
            command: Some("true".to_string()),
            timeout_ms: None,
            source: None,
        },
    ];
    let manifest = GateManifest {
        profile: "minimal".to_string(),
        checks,
        ..minimal_manifest_template()
    };
    let resolved = crate::gate::manifest::resolve(&manifest, &[]).expect("resolves");
    let value = build_document(&resolved, &manifest);
    let ids: Vec<&serde_json::Value> = value["planned_checks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| &c["id"])
        .collect();
    assert!(
        !ids.contains(&&serde_json::json!("repo-hygiene")),
        "an enabled = false declaration is removed from the plan: {ids:?}"
    );
    assert!(ids.contains(&&serde_json::json!("smoke")));
}

/// `GateManifest` has more fields than a rehearsal reads; build the rest from
/// the TOML so no field is forgotten when it is added.
fn minimal_manifest_template() -> GateManifest {
    toml::from_str("profile = \"minimal\"\n").expect("a bare manifest parses")
}
