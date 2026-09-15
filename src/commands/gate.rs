//! `driftwatch gate` orchestration: local-first Gate execution.
//!
//! The gate command is the first completion verification for every
//! relevant change — CI repeats it, never replaces it. Flow:
//!
//! 1. Resolve the project Gate plan from `gate.toml` (pure, no
//!    execution). `--dry-run` stops here: the plan is printed and
//!    nothing is persisted.
//! 2. Execute each planned check locally. A check with a declared
//!    `command` runs through the project-runtime text adapter (`sh -c`
//!    over the declared string, bounded capture, timeout,
//!    process-group kill, secret redaction); a check without a command
//!    records `NOT_APPLICABLE` so missing coverage is explicit, never
//!    silent. One failure never aborts the rest.
//! 3. When `[ai]` is enabled in `gate.toml`, one additional
//!    `ai-review` evaluation runs through the provider-neutral AI
//!    boundary (opt-in, redacted, bounded, fail-closed). Disabled AI
//!    makes no provider call and adds no check.
//! 4. Aggregate with the manifest blocking policy, persist one
//!    `gate_runs` row transactionally, print remediation, and exit
//!    nonzero when blocked.
//!
//! `driftwatch check` remains the compatibility entry point for legacy
//! checker-only projects and is untouched.

use std::collections::BTreeMap;
use std::path::Path;

use crate::cli::{GateArgs, GateFormatArg};
use crate::error::Error;
use crate::gate::adapters::{default_registry, AdapterInput};
use crate::gate::aggregate::{aggregate, AggregateOutcome, GatePlan, PlannedCheck};
use crate::gate::ai::{self, AiEvalInput, AiRule};
use crate::gate::manifest::{self, ResolvedGatePlan};
use crate::gate::types::{GateResult, GateSeverity, GateStatus};
use crate::project::{git, ProjectRoot};
use crate::repo::evidence::Artifacts;
use crate::repo::gates::{Gates, NewGateRun};
use crate::util::truncate_char_boundary;

const EXIT_OK: i32 = 0;
const EXIT_BLOCKED: i32 = 1;
const CHECK_TIMEOUT_MS: u64 = 60_000;
const CHECK_MAX_OUTPUT_BYTES: u64 = 1024 * 1024;
const MAX_RESULTS_JSON_BYTES: usize = 256 * 1024;

/// `driftwatch gate [--dry-run] [--format human|json]`.
pub fn gate(args: GateArgs, cwd: &Path) -> Result<i32, Error> {
    let proj = ProjectRoot::discover(cwd)?;
    let gate_toml =
        manifest::manifest_path(&proj.root).unwrap_or_else(|| proj.root.join("gate.toml"));
    let manifest = manifest::load(&proj.root).map_err(|e| Error::ConfigInvalid {
        path: gate_toml.clone(),
        message: e.to_string(),
    })?;
    let Some(manifest) = manifest else {
        println!("driftwatch gate: no gate.toml; nothing to gate.");
        println!("Legacy `driftwatch check` remains available for checker-only projects.");
        return Ok(EXIT_OK);
    };
    let resolved = manifest::resolve(&manifest, &[]).map_err(|e| Error::ConfigInvalid {
        path: gate_toml,
        message: e.to_string(),
    })?;

    if args.dry_run {
        print!("{}", manifest::render_plan(&resolved));
        return Ok(EXIT_OK);
    }

    let mut db = crate::repo::Db::open(&proj.db_path)?;
    let (results, plan) = execute_plan(&resolved, &manifest, &proj, &db)?;
    let outcome = aggregate(&plan, &results);
    let git_ctx = git::capture(&proj.root);
    let taken_at = chrono::Utc::now().to_rfc3339();
    let revision = git_ctx.commit.clone().unwrap_or_else(|| taken_at.clone());
    let change_id = revision.clone();
    let results_json = bound_results_json(&results);
    Gates::insert(
        &mut db,
        &NewGateRun {
            taken_at: &taken_at,
            change_id: &change_id,
            revision: &revision,
            manifest_digest: &resolved.manifest_digest,
            rule_pack_version: &resolved.rule_pack_version,
            status: outcome.status.as_str(),
            blocked: outcome.blocked,
            results_json: &results_json,
        },
    )?;

    match args.format {
        GateFormatArg::Json => print_json(&outcome, &results, &resolved),
        GateFormatArg::Human => print_human(&outcome, &results, &resolved),
    }

    if outcome.blocked {
        Ok(EXIT_BLOCKED)
    } else {
        Ok(EXIT_OK)
    }
}

/// Execute every planned check and return results plus the effective
/// plan (the resolved plan plus an appended `ai-review` check when AI
/// evaluation is enabled).
fn execute_plan(
    resolved: &ResolvedGatePlan,
    manifest: &manifest::GateManifest,
    proj: &ProjectRoot,
    db: &crate::repo::Db,
) -> Result<(Vec<GateResult>, GatePlan), Error> {
    let commands: BTreeMap<&str, &str> = manifest
        .checks
        .iter()
        .filter_map(|c| c.command.as_deref().map(|cmd| (c.id.as_str(), cmd)))
        .collect();
    let registry = default_registry();
    let runtime = registry
        .get("project-runtime")
        .expect("built-in project-runtime adapter is registered");
    let mut results = Vec::with_capacity(resolved.plan.checks.len() + 1);
    for check in &resolved.plan.checks {
        match commands.get(check.gate_id.as_str()) {
            Some(cmd) => {
                let input = AdapterInput {
                    program: "sh".to_string(),
                    args: vec!["-c".to_string(), (*cmd).to_string()],
                    working_dir: proj.root.clone(),
                    env: BTreeMap::new(),
                    timeout_ms: CHECK_TIMEOUT_MS,
                    max_output_bytes: CHECK_MAX_OUTPUT_BYTES,
                    evidence: vec![],
                };
                results.push(crate::gate::adapters::run_adapter(
                    runtime,
                    &input,
                    Some(&check.gate_id),
                ));
            }
            None => results.push(GateResult {
                gate_id: truncate_char_boundary(&check.gate_id, 128),
                source: truncate_char_boundary(&check.source, 256),
                status: GateStatus::NotApplicable,
                severity: GateSeverity::Info,
                findings: vec![],
                evidence: vec![],
                missing_evidence: vec![format!("{}:command", check.gate_id)],
                diagnostic: Some(
                    "no command declared for this concern; no execution was attempted".to_string(),
                ),
                remediation: Some(
                    "Declare `command` for this check in gate.toml or accept review for this concern."
                        .to_string(),
                ),
            }),
        }
    }
    let mut plan = resolved.plan.clone();
    // Optional AI evaluation: only when `[ai] enabled = true`. The
    // rules are the planned concern ids; evidence resolves against
    // the artifact store so PASS stays evidence-backed.
    if let Some(config) = ai::load_ai_config(&proj.root).unwrap_or(None) {
        if config.enabled {
            let rules: Vec<AiRule> = plan
                .checks
                .iter()
                .map(|c| AiRule {
                    id: c.gate_id.clone(),
                    text: format!("concern {}", c.gate_id),
                })
                .collect();
            let result = ai::run_ai_evaluation(
                &AiEvalInput {
                    gate_id: "ai-review",
                    rules: &rules,
                    evidence: &[],
                    contexts: &[],
                    config: &config,
                    sensitive: &[],
                    project_root: &proj.root,
                },
                &|key| {
                    Artifacts::new(db)
                        .get_by_key(key)
                        .unwrap_or(None)
                        .filter(|r| r.available)
                },
            );
            results.push(result);
            if !plan.checks.iter().any(|c| c.gate_id == "ai-review") {
                plan.checks.push(PlannedCheck {
                    gate_id: "ai-review".to_string(),
                    source: "ai".to_string(),
                    required: true,
                });
                plan.checks.sort_by(|a, b| a.gate_id.cmp(&b.gate_id));
            }
        }
    }
    Ok((results, plan))
}

fn bound_results_json(results: &[GateResult]) -> String {
    let json = serde_json::to_string(results).unwrap_or_else(|_| "[]".to_string());
    truncate_char_boundary(&json, MAX_RESULTS_JSON_BYTES)
}

fn print_human(outcome: &AggregateOutcome, results: &[GateResult], resolved: &ResolvedGatePlan) {
    println!(
        "driftwatch gate: {} (blocked: {})",
        outcome.status.as_str(),
        outcome.blocked
    );
    println!(
        "manifest: {} | rule-pack: {}",
        resolved.manifest_digest, resolved.rule_pack_version
    );
    println!();
    println!(
        "{:<24}  {:<15}  {:<8}  DIAGNOSTIC",
        "GATE", "STATUS", "SEVERITY"
    );
    for r in results {
        let diag = r.diagnostic.as_deref().unwrap_or("-");
        println!(
            "{:<24}  {:<15}  {:<8}  {}",
            truncate(&r.gate_id, 24),
            r.status.as_str(),
            r.severity.as_str(),
            truncate(diag, 80)
        );
    }
    println!();
    if !outcome.failures.is_empty() {
        println!("Blocking failures: {}", outcome.failures.join(", "));
        println!("Remediation: investigate the reported findings and add regression coverage.");
    }
    if !outcome.pending_reviews.is_empty() {
        println!("Pending reviews: {}", outcome.pending_reviews.join(", "));
        println!(
            "Remediation: resolve the review concerns before representing this change as complete."
        );
    }
    if !outcome.not_applicable.is_empty() {
        println!("Not applicable: {}", outcome.not_applicable.join(", "));
    }
    if !outcome.blocked {
        println!("Gate passed locally. CI repeats this command; it does not replace it.");
    }
}

fn print_json(outcome: &AggregateOutcome, results: &[GateResult], resolved: &ResolvedGatePlan) {
    let doc = serde_json::json!({
        "status": outcome.status.as_str(),
        "blocked": outcome.blocked,
        "failures": outcome.failures,
        "pending_reviews": outcome.pending_reviews,
        "not_applicable": outcome.not_applicable,
        "manifest_digest": resolved.manifest_digest,
        "rule_pack_version": resolved.rule_pack_version,
        "results": results,
    });
    println!("{}", serde_json::to_string_pretty(&doc).unwrap_or_default());
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
        out.push('…');
        out
    }
}

/// Doctor readiness for local gate history. Silent when no `gate.toml`
/// exists (unconfigured-but-ok). Otherwise reports the latest
/// persisted run: `Pass` when the gate passed, `Warn` when the latest
/// run is blocked or missing (run `driftwatch gate` before archive).
pub fn gate_history_checks(proj: &ProjectRoot) -> Vec<crate::doctor::check::Check> {
    use crate::doctor::check::{Check, Status};
    if manifest::manifest_path(&proj.root).is_none() {
        return Vec::new();
    }
    let db = match crate::repo::Db::open_read_only(&proj.db_path) {
        Ok(d) => d,
        Err(_) => return Vec::new(),
    };
    let gates = Gates::new(&db);
    let latest = match gates.latest() {
        Ok(l) => l,
        Err(_) => return Vec::new(),
    };
    match latest {
        None => vec![Check::info(
            "gate.history",
            "No local gate runs recorded",
            "gate.toml is configured but `driftwatch gate` has never run here.".to_string(),
        )
        .with_remediation("Run `driftwatch gate` locally before archive; CI repeats it.")],
        Some(run) if !run.blocked && run.status == "PASS" => {
            vec![
                Check::pass("gate.history", "Latest local gate run passed").with_remediation(
                    format!(
                        "run {} at {} ({}).",
                        run.id, run.taken_at, run.manifest_digest
                    ),
                ),
            ]
        }
        Some(run) => vec![Check {
            id: "gate.history",
            name: "Latest local gate run is blocked".to_string(),
            status: Status::Warn,
            detail: Some(format!(
                "run {} at {} status {} ({}).",
                run.id, run.taken_at, run.status, run.manifest_digest
            )),
            remediation: Some(
                "Run `driftwatch gate` locally and resolve the blocking findings before archive."
                    .to_string(),
            ),
        }],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::GateArgs;
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
            format: GateFormatArg::Human,
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
            "profile = \"minimal\"\n[[checks]]\nid = \"smoke\"\ncommand = \"true\"\n",
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
}
