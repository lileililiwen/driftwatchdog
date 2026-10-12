//! Unit tests for `src/commands/gate.rs`.
//!
//! Split out with `#[path = "gate_tests.rs"] mod tests;` — the same
//! separation `src/gate/platform_result_tests.rs` uses — so the command
//! module stays under the 1,000-line `source-file-size` boundary instead of
//! carrying its own tests. It is a pure move: every assertion below is the
//! one that was inline, unchanged, including the per-check-timeout negative
//! controls that this file is still the home of.

use super::*;
use crate::cli::GateArgs;
use crate::gate::dto::GateRunResultsDocument;
use crate::project::root::ProjectRoot;

fn init_project(dir: &Path) {
    std::fs::create_dir_all(dir.join(".driftwatch")).unwrap();
    let mut db = crate::repo::Db::open(&dir.join(".driftwatch/state.db")).unwrap();
    crate::storage::migrations::apply(db.conn_mut()).unwrap();
    std::fs::write(dir.join("driftwatch.toml"), "").unwrap();
}

fn gate_args(dry_run: bool) -> GateArgs {
    GateArgs {
        dry_run,
        format: GateOutputFormat::Human,
        subcommand: None,
    }
}

#[test]
fn gate_without_manifest_exits_zero_without_persistence() {
    let tmp = tempfile::tempdir().unwrap();
    init_project(tmp.path());
    let code = gate(gate_args(false), tmp.path()).unwrap();
    assert_eq!(code, EXIT_OK);
    let db = crate::repo::Db::open_read_only(&tmp.path().join(".driftwatch/state.db")).unwrap();
    assert_eq!(Gates::new(&db).count().unwrap(), 0);
}

#[test]
fn gate_passing_change_persists_snapshot_and_exits_zero() {
    let tmp = tempfile::tempdir().unwrap();
    init_project(tmp.path());
    std::fs::write(
        tmp.path().join("gate.toml"),
        "profile = \"minimal\"\n[[checks]]\nid = \"governance-quality\"\ncommand = \"echo '{\\\"findings\\\":[],\\\"summary\\\":{\\\"errors\\\":0,\\\"warnings\\\":0}}'\"\n[[checks]]\nid = \"smoke\"\ncommand = \"true\"\n",
    )
    .unwrap();
    let code = gate(gate_args(false), tmp.path()).unwrap();
    assert_eq!(code, EXIT_OK);
    let db = crate::repo::Db::open_read_only(&tmp.path().join(".driftwatch/state.db")).unwrap();
    let latest = Gates::new(&db).latest().unwrap().unwrap();
    assert_eq!(latest.status, "PASS");
    assert!(!latest.blocked);
    assert!(latest.manifest_digest.starts_with("sha256:"));
}

#[test]
fn gate_blocking_failure_persists_and_exits_nonzero() {
    let tmp = tempfile::tempdir().unwrap();
    init_project(tmp.path());
    std::fs::write(
        tmp.path().join("gate.toml"),
        "profile = \"minimal\"\n[[checks]]\nid = \"smoke\"\ncommand = \"false\"\n",
    )
    .unwrap();
    let code = gate(gate_args(false), tmp.path()).unwrap();
    assert_eq!(code, EXIT_BLOCKED);
    let db = crate::repo::Db::open_read_only(&tmp.path().join(".driftwatch/state.db")).unwrap();
    let latest = Gates::new(&db).latest().unwrap().unwrap();
    assert!(latest.blocked);
    assert_eq!(latest.status, "FAIL");
}

#[test]
fn gate_review_without_command_blocks_but_names_evidence() {
    let tmp = tempfile::tempdir().unwrap();
    init_project(tmp.path());
    std::fs::write(
        tmp.path().join("gate.toml"),
        "profile = \"minimal\"\n[[checks]]\nid = \"ux-review\"\n",
    )
    .unwrap();
    let code = gate(gate_args(false), tmp.path()).unwrap();
    assert_eq!(code, EXIT_BLOCKED);
    let db = crate::repo::Db::open_read_only(&tmp.path().join(".driftwatch/state.db")).unwrap();
    let latest = Gates::new(&db).latest().unwrap().unwrap();
    assert!(latest.blocked);
    assert!(latest.results_json.contains("ux-review:command"));
}

/// Inverts `characterisation_required_unbound_is_not_applicable_info_zero_findings`
/// (task 1.2): a required check with no command is now `UNBOUND` +
/// `Error` with a named finding, and the gate FAILs.
#[test]
fn required_unbound_check_fails_with_error_severity_and_a_finding() {
    let tmp = tempfile::tempdir().unwrap();
    init_project(tmp.path());
    std::fs::write(
        tmp.path().join("gate.toml"),
        "profile = \"minimal\"\n[[checks]]\nid = \"ux-review\"\n",
    )
    .unwrap();
    let code = gate(gate_args(false), tmp.path()).unwrap();
    assert_eq!(code, EXIT_BLOCKED);
    let db = crate::repo::Db::open_read_only(&tmp.path().join(".driftwatch/state.db")).unwrap();
    let run = Gates::new(&db).latest().unwrap().unwrap();
    assert_eq!(run.status, "FAIL");
    assert!(run.blocked);
    let doc: GateRunResultsDocument = serde_json::from_str(&run.results_json).unwrap();
    assert_eq!(
        doc.unbound,
        vec!["governance-quality".to_string(), "ux-review".to_string()]
    );
    let r = doc
        .results
        .iter()
        .find(|r| r.gate_id == "ux-review")
        .unwrap();
    assert_eq!(r.status, GateStatus::Unbound);
    assert_eq!(r.severity, GateSeverity::Error);
    assert_eq!(r.findings.len(), 1);
    assert_eq!(
        r.findings[0].rule.as_deref(),
        Some("unbound-required-check")
    );
    assert!(r.findings[0].title.contains("no bound command"));
    assert_eq!(
        r.findings[0].location.as_deref(),
        Some("commands.ux-review"),
        "the finding names the check and the absent command"
    );
    assert_eq!(r.evidence.len(), 0);
    assert_eq!(r.missing_evidence, vec!["ux-review:command".to_string()]);
}

/// An *optional* unbound check is recorded, named, and never blocks.
#[test]
fn optional_unbound_check_is_recorded_without_blocking() {
    let tmp = tempfile::tempdir().unwrap();
    init_project(tmp.path());
    std::fs::write(
        tmp.path().join("gate.toml"),
        "profile = \"minimal\"\n[[checks]]\nid = \"governance-quality\"\ncommand = \"echo '{\\\"findings\\\":[],\\\"summary\\\":{\\\"errors\\\":0,\\\"warnings\\\":0}}'\"\n[[checks]]\nid = \"ux-review\"\nrequired = false\n",
    )
    .unwrap();
    let code = gate(gate_args(false), tmp.path()).unwrap();
    assert_eq!(code, EXIT_OK);
    let db = crate::repo::Db::open_read_only(&tmp.path().join(".driftwatch/state.db")).unwrap();
    let run = Gates::new(&db).latest().unwrap().unwrap();
    assert_eq!(run.status, "PASS");
    assert!(!run.blocked);
    let doc: GateRunResultsDocument = serde_json::from_str(&run.results_json).unwrap();
    assert_eq!(doc.unbound, vec!["optional:ux-review".to_string()]);
    let r = doc
        .results
        .iter()
        .find(|r| r.gate_id == "ux-review")
        .unwrap();
    assert_eq!(r.status, GateStatus::Unbound);
    assert_eq!(r.severity, GateSeverity::Warning);
    assert_eq!(
        r.findings[0].rule.as_deref(),
        Some("unbound-optional-check")
    );
}

/// A declared exemption is acknowledged on the record: reported as
/// `NOT_APPLICABLE` with the written reason, never as unbound, and
/// it does not block even though the check is required.
///
/// The fixture carries eight required checks so the manifest has an
/// allowance of two. Before the exemption cap landed, this test ran a
/// single-check manifest whose one exemption was unconditionally
/// accepted; the same property is now asserted inside a budget rather
/// than outside one, and every assertion below is unchanged.
#[test]
fn declared_exemption_does_not_block_and_carries_its_reason() {
    let tmp = tempfile::tempdir().unwrap();
    init_project(tmp.path());
    let mut manifest = String::from("profile = \"minimal\"\n");
    for i in 0..7 {
        manifest.push_str(&format!("[[checks]]\nid = \"c{i}\"\ncommand = \"true\"\n"));
    }
    manifest.push_str(
        "[[checks]]\nid = \"governance-quality\"\ncommand = \"echo '{\\\"findings\\\":[],\\\"summary\\\":{\\\"errors\\\":0,\\\"warnings\\\":0}}'\"\n\
         [[checks]]\nid = \"ux-review\"\n\
         [not_applicable]\nux-review = \"no UI surface in this project\"\n",
    );
    std::fs::write(tmp.path().join("gate.toml"), manifest).unwrap();
    let code = gate(gate_args(false), tmp.path()).unwrap();
    assert_eq!(code, EXIT_OK);
    let db = crate::repo::Db::open_read_only(&tmp.path().join(".driftwatch/state.db")).unwrap();
    let run = Gates::new(&db).latest().unwrap().unwrap();
    assert_eq!(run.status, "PASS");
    let doc: GateRunResultsDocument = serde_json::from_str(&run.results_json).unwrap();
    assert!(doc.unbound.is_empty());
    // One exemption inside an allowance of two: no budget verdict.
    assert!(doc.results.iter().all(|r| r.gate_id != "exemption-budget"));
    let r = doc
        .results
        .iter()
        .find(|r| r.gate_id == "ux-review")
        .unwrap();
    assert_eq!(r.status, GateStatus::NotApplicable);
    assert_eq!(r.findings[0].rule.as_deref(), Some("declared-exemption"));
    assert_eq!(
        r.findings[0].location.as_deref(),
        Some("not_applicable.ux-review")
    );
    assert!(
        r.findings[0]
            .title
            .contains("no UI surface in this project"),
        "the exemption records its reason: {}",
        r.findings[0].title
    );
}

#[test]
fn gate_dry_run_shows_plan_without_execution_or_persistence() {
    let tmp = tempfile::tempdir().unwrap();
    init_project(tmp.path());
    std::fs::write(
        tmp.path().join("gate.toml"),
        "profile = \"minimal\"\n[[checks]]\nid = \"smoke\"\ncommand = \"false\"\n",
    )
    .unwrap();
    // Even though the check would fail, dry-run exits zero and
    // persists nothing.
    let code = gate(gate_args(true), tmp.path()).unwrap();
    assert_eq!(code, EXIT_OK);
    let db = crate::repo::Db::open_read_only(&tmp.path().join(".driftwatch/state.db")).unwrap();
    assert_eq!(Gates::new(&db).count().unwrap(), 0);
}

#[test]
fn gate_history_reports_blocked_latest_as_warn() {
    let tmp = tempfile::tempdir().unwrap();
    init_project(tmp.path());
    std::fs::write(tmp.path().join("gate.toml"), "profile = \"minimal\"\n").unwrap();
    // No runs yet: honest Info.
    let proj = ProjectRoot::at(tmp.path());
    let checks = gate_history_checks(&proj);
    assert_eq!(checks.len(), 1);
    assert_eq!(checks[0].id, "gate.history");
}

#[test]
fn gate_resolves_business_yaml_manifest_and_persists_identity() {
    let tmp = tempfile::tempdir().unwrap();
    init_project(tmp.path());
    std::fs::create_dir_all(tmp.path().join(".ai-gate")).unwrap();
    std::fs::write(
        tmp.path().join(".ai-gate/gate.yaml"),
        "version: 1\nruntime: driftwatchdog\nprofile: browser-extension\nrule_pack: browser-extension@0.1.0\nchecks:\n  smoke: true\n  governance-quality: true\ncommands:\n  smoke: \"true\"\n  governance-quality: \"echo '{\\\"findings\\\":[],\\\"summary\\\":{\\\"errors\\\":0,\\\"warnings\\\":0}}'\"\n",
    )
    .unwrap();
    let code = gate(gate_args(false), tmp.path()).unwrap();
    assert_eq!(code, EXIT_OK);
    let db = crate::repo::Db::open_read_only(&tmp.path().join(".driftwatch/state.db")).unwrap();
    let latest = Gates::new(&db).latest().unwrap().unwrap();
    assert_eq!(latest.status, "PASS");
    assert_eq!(latest.rule_pack_version, "browser-extension@0.1.0");
}

#[test]
fn gate_foreign_runtime_exits_zero_without_persistence() {
    let tmp = tempfile::tempdir().unwrap();
    init_project(tmp.path());
    std::fs::create_dir_all(tmp.path().join(".ai-gate")).unwrap();
    std::fs::write(
        tmp.path().join(".ai-gate/gate.yaml"),
        "version: 1\nruntime: some-other-tool\nprofile: minimal\n",
    )
    .unwrap();
    let code = gate(gate_args(false), tmp.path()).unwrap();
    assert_eq!(code, EXIT_OK);
    let db = crate::repo::Db::open_read_only(&tmp.path().join(".driftwatch/state.db")).unwrap();
    assert_eq!(Gates::new(&db).count().unwrap(), 0);
}

#[test]
fn gate_prefers_gate_toml_over_business_yaml() {
    let tmp = tempfile::tempdir().unwrap();
    init_project(tmp.path());
    std::fs::create_dir_all(tmp.path().join(".ai-gate")).unwrap();
    // The YAML would pass; the TOML fails. Precedence means TOML wins.
    std::fs::write(
        tmp.path().join(".ai-gate/gate.yaml"),
        "version: 1\nruntime: driftwatchdog\nprofile: minimal\nchecks:\n  smoke: true\ncommands:\n  smoke: \"true\"\n",
    )
    .unwrap();
    std::fs::write(
        tmp.path().join("gate.toml"),
        "profile = \"minimal\"\n[[checks]]\nid = \"smoke\"\ncommand = \"false\"\n",
    )
    .unwrap();
    let code = gate(gate_args(false), tmp.path()).unwrap();
    assert_eq!(code, EXIT_BLOCKED);
}

// --- negative controls for the per-check timeout -----------------------

/// **Negative control 2 — the field is honored, not ignored.**
///
/// A declared `timeout_ms` of 1000ms against `sleep 5` must actually
/// kill the command and report the timeout. If the field were ignored
/// the 60s default would apply, the sleep would finish, exit 0 would
/// be recorded and this test would see `PASS` — so the assertion
/// below fails whenever the wiring breaks. The elapsed-time check
/// closes the other door: a run that waited out the full 5s and then
/// reported a timeout for some other reason also fails.
///
/// The `sleep 5` is the honest part of the control. It is the reason
/// an ignored field would visibly produce a different result.
#[test]
fn declared_check_timeout_kills_the_command_and_reports_a_timeout() {
    let tmp = tempfile::tempdir().unwrap();
    init_project(tmp.path());
    std::fs::write(
        tmp.path().join("gate.toml"),
        "profile = \"minimal\"\n[[checks]]\nid = \"governance-quality\"\ncommand = \"echo '{\\\"findings\\\":[],\\\"summary\\\":{\\\"errors\\\":0,\\\"warnings\\\":0}}'\"\n[[checks]]\nid = \"slow\"\ncommand = \"sleep 5\"\ntimeout_ms = 1000\n",
    )
    .unwrap();

    let started = std::time::Instant::now();
    let code = gate(gate_args(false), tmp.path()).unwrap();
    let elapsed = started.elapsed();

    // A timeout is REVIEW_REQUIRED, which blocks under the default
    // policy — distinct from a non-zero exit's FAIL, and not a PASS.
    assert_eq!(code, EXIT_BLOCKED);
    // Killed near the 1s deadline, nowhere near the 5s the command
    // asked for. Generous upper bound so a loaded machine does not
    // make this flaky; still far below the command's own runtime.
    assert!(
        elapsed < std::time::Duration::from_secs(4),
        "took {elapsed:?}; the 1000ms deadline did not take effect"
    );

    let db = crate::repo::Db::open_read_only(&tmp.path().join(".driftwatch/state.db")).unwrap();
    let run = Gates::new(&db).latest().unwrap().unwrap();
    assert_eq!(run.status, "REVIEW_REQUIRED");
    assert!(run.blocked);
    let doc: GateRunResultsDocument = serde_json::from_str(&run.results_json).unwrap();
    let r = doc
        .results
        .iter()
        .find(|r| r.gate_id == "slow")
        .expect("the check reported");
    assert_eq!(r.status, GateStatus::ReviewRequired);
    assert!(
        r.missing_evidence
            .iter()
            .any(|k| k == "project-runtime:timeout"),
        "timeout evidence missing from {:?}",
        r.missing_evidence
    );
}

/// **Negative control 1 — the default path, without a 60s wait.**
///
/// A manifest that declares no `timeout_ms` must still be bounded by
/// the 60s default. Spending a real 60 seconds to prove a constant
/// equals its own definition buys nothing: what is worth proving is
/// that the *default is still consulted* and that the number handed
/// to the adapter is the plan's, not a hardcoded one. Both are
/// asserted here, and the command runs fast so the wiring is
/// exercised for real.
///
/// The wall-clock proof that 60s is genuinely the ceiling is not
/// affordable in a unit test; the resolved-value proof is, and it is
/// what the plan actually reads. Control 2 above covers the same
/// code path with a value small enough to wait out.
#[test]
fn a_manifest_without_a_timeout_still_resolves_the_sixty_second_default() {
    let tmp = tempfile::tempdir().unwrap();
    init_project(tmp.path());
    std::fs::write(
        tmp.path().join("gate.toml"),
        "profile = \"minimal\"\n[[checks]]\nid = \"smoke\"\ncommand = \"true\"\n",
    )
    .unwrap();
    // Dry-run resolves and renders the plan without executing, so
    // this reads the default the runner would have used.
    let proj = ProjectRoot::discover(tmp.path()).unwrap();
    let manifest = match manifest::load_for_runtime(&proj.root).unwrap() {
        manifest::LoadOutcome::Manifest(m) => *m,
        _ => panic!("fixture has a native manifest"),
    };
    let resolved = manifest::resolve(&manifest, &[]).unwrap();
    assert_eq!(resolved.check_timeouts.get("smoke"), Some(&60_000));
    assert_eq!(CHECK_TIMEOUT_MS, 60_000);
}

/// The last link in the chain: the number carried on
/// `AdapterInput.timeout_ms` is the number the runner actually
/// enforces, for a declared value and for the default.
///
/// This asserts a real function (`AdapterInput::checker_spec`), not
/// a copy of the runner's lookup expression — an earlier version of
/// this test re-implemented the lookup inline and therefore passed
/// even when the production wiring was deliberately broken, which
/// is false confidence and worse than no test. The half that proves
/// the *plan's* value reaches `AdapterInput` is
/// `tests/gate_check_timeout.rs`, which goes through the binary and
/// was confirmed to fail when the lookup is sabotaged.
#[test]
fn the_adapter_enforces_exactly_the_declared_milliseconds() {
    for declared_ms in [4321u64, CHECK_TIMEOUT_MS, 1_000] {
        let input = AdapterInput {
            program: "sh".to_string(),
            args: vec!["-c".to_string(), "sleep 30".to_string()],
            working_dir: std::env::temp_dir(),
            env: BTreeMap::new(),
            timeout_ms: declared_ms,
            max_output_bytes: CHECK_MAX_OUTPUT_BYTES,
            evidence: vec![],
        };
        assert_eq!(
            input.checker_spec("project-runtime").timeout,
            std::time::Duration::from_millis(declared_ms)
        );
    }
}
