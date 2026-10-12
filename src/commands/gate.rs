//! `driftwatch gate` orchestration: local-first Gate execution.
//!
//! The gate command is the first completion verification for every
//! relevant change — CI repeats it, never replaces it. Flow:
//!
//! 1. Resolve the project Gate plan from `gate.toml` (pure, no
//!    execution). `--dry-run` stops here: the plan is printed and
//!    nothing is persisted. `--dry-run --format json` emits the
//!    machine-readable **rehearsal** document from
//!    [`crate::gate::dry_run_plan`] — a plan, never a verdict.
//! 2. Execute each planned check locally. The built-in
//!    `source-file-size` concern runs the in-process repository scanner
//!    (raw newline counting over a Git-aware boundary, no project
//!    command). A check with a declared `command` runs through the
//!    project-runtime text adapter (`sh -c` over the declared string,
//!    bounded capture, timeout, process-group kill, secret redaction); a
//!    check without a command records `NOT_APPLICABLE` so missing
//!    coverage is explicit, never silent. One failure never aborts the
//!    rest.
//! 3. When `[ai]` is enabled in `gate.toml`, one additional
//!    `ai-review` evaluation runs through the provider-neutral AI
//!    boundary (opt-in, redacted, bounded, fail-closed). Disabled AI
//!    makes no provider call and adds no check.
//! 4. Aggregate with the manifest blocking policy, persist one
//!    `gate_runs` row transactionally, print remediation, and exit
//!    nonzero when blocked.
//!
//! Three output formats sit on top of the same run. `--format human` and
//! `--format json` are driftwatchdog's own; `--format platform` emits the
//! portfolio's `platform.gate-result` contract at major 1.0.0 through
//! [`crate::gate::platform_result`] and exits `2` — writing nothing to
//! stdout — when an identifier or length has no honest representation in that
//! contract. All three read the same persisted run.
//!
//! `driftwatch check` remains the compatibility entry point for legacy
//! checker-only projects and is untouched.

use std::collections::BTreeMap;
use std::path::Path;

use crate::cli::{GateArgs, GateOutputFormat};
use crate::error::Error;
use crate::gate::adapters::{
    default_registry, run_capability_conformance_adapter, run_product_quality_adapter,
    run_release_evidence_adapter, AdapterInput,
};
use crate::gate::aggregate::{aggregate, AggregateOutcome, GatePlan, PlannedCheck};
use crate::gate::ai::{self, AiEvalInput, AiRule};
use crate::gate::concerns::{
    is_capability_conformance_concern, is_governance_quality_concern, is_product_quality_concern,
    is_release_evidence_concern, is_source_file_size_concern,
};
use crate::gate::dry_run_plan;
use crate::gate::dto::{to_gate_run_json, MAX_DIAGNOSTIC_BYTES, MAX_REMEDIATION_BYTES};
use crate::gate::governance::run_governance_quality_adapter;
use crate::gate::manifest::{self, ExemptionVerdict, ResolvedGatePlan, MAX_EXEMPTION_REASON_BYTES};
use crate::gate::platform_result::{self, PlatformRun, PLATFORM_PIPELINE};
use crate::gate::redact::bound_text;
use crate::gate::types::{Finding, GateResult, GateSeverity, GateStatus};
use crate::project::{git, ProjectRoot};
use crate::repo::evidence::Artifacts;
use crate::repo::gates::{Gates, NewGateRun};
use crate::util::truncate_char_boundary;

const EXIT_OK: i32 = 0;
const EXIT_BLOCKED: i32 = 1;
/// The run could not be published as a `platform.gate-result` document
/// because some identifier or length has no honest representation in that
/// contract and driftwatchdog will not truncate it to fit. Reachable only
/// through `--format platform`; no existing path can produce it, so `0` and
/// `1` keep their meaning everywhere else.
const EXIT_UNREPRESENTABLE: i32 = 2;
/// Deadline applied to a check that declares no `timeout_ms` of its
/// own. Unchanged from the constant that used to serve every check in
/// the fleet, so no existing manifest needs an edit to keep its
/// behaviour. The per-check value is resolved on the plan
/// (`ResolvedGatePlan::check_timeouts`); this is only the default.
const CHECK_TIMEOUT_MS: u64 = manifest::DEFAULT_CHECK_TIMEOUT_MS;
const CHECK_MAX_OUTPUT_BYTES: u64 = 1024 * 1024;
const MAX_RESULTS_JSON_BYTES: usize = 256 * 1024;

/// `driftwatch gate [--dry-run] [--format human|json]`.
///
/// The `gate` command also accepts a subcommand (e.g.
/// `driftwatch gate evidence-export`); when a subcommand is given
/// the top-level flags are ignored and the subcommand takes over.
pub fn gate(args: GateArgs, cwd: &Path) -> Result<i32, Error> {
    if let Some(sub) = args.subcommand {
        return match sub {
            crate::cli::GateSubcommand::EvidenceExport(args) => {
                crate::commands::evidence_export::evidence_export(args, cwd)
            }
        };
    }
    let proj = ProjectRoot::discover(cwd)?;
    let gate_toml =
        manifest::manifest_path(&proj.root).unwrap_or_else(|| proj.root.join("gate.toml"));
    let manifest = match manifest::load_for_runtime(&proj.root) {
        Ok(manifest::LoadOutcome::Manifest(m)) => *m,
        Ok(manifest::LoadOutcome::NoManifest) => {
            println!("driftwatch gate: no gate.toml or .ai-gate/gate.yaml; nothing to gate.");
            println!("Legacy `driftwatch check` remains available for checker-only projects.");
            return Ok(EXIT_OK);
        }
        Ok(manifest::LoadOutcome::ForeignRuntime { runtime }) => {
            println!(
                "driftwatch gate: .ai-gate/gate.yaml targets runtime `{runtime}`, not driftwatchdog."
            );
            println!("Nothing was executed and no gate run was recorded.");
            return Ok(EXIT_OK);
        }
        Err(e) => {
            return Err(Error::ConfigInvalid {
                path: gate_toml.clone(),
                message: e.to_string(),
            })
        }
    };
    let resolved = manifest::resolve(&manifest, &[]).map_err(|e| Error::ConfigInvalid {
        path: gate_toml,
        message: e.to_string(),
    })?;

    if args.dry_run {
        dry_run_plan::print_dry_run(&args.format, &resolved, &manifest);
        return Ok(EXIT_OK);
    }

    let mut db = crate::repo::Db::open(&proj.db_path)?;
    let (results, plan) = execute_plan(&resolved, &manifest, &proj, &db)?;
    let outcome = aggregate(&plan, &results);
    let git_ctx = git::capture(&proj.root);
    let taken_at = chrono::Utc::now().to_rfc3339();
    let revision = git_ctx.commit.clone().unwrap_or_else(|| taken_at.clone());
    let change_id = revision.clone();
    let results_json = bound_results_json(&results, &outcome.unbound);
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
        GateOutputFormat::Json => print_json(&outcome, &results, &resolved),
        GateOutputFormat::Human => print_human(&outcome, &results, &resolved),
        GateOutputFormat::Platform => {
            // The run is already persisted above: a refusal is a statement
            // about emission, never about the run.
            return Ok(print_platform(
                &outcome, &results, &resolved, &taken_at, &revision,
            ));
        }
    }

    if outcome.blocked {
        Ok(EXIT_BLOCKED)
    } else {
        Ok(EXIT_OK)
    }
}

/// Execute every planned check and return results plus the effective
/// plan (the resolved plan plus an appended `ai-review` check when AI
/// evaluation is enabled). The built-in `source-file-size` concern is
/// dispatched to the in-process repository scanner; every other concern
/// follows the command/adaptor path below.
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
    // Capture Git metadata once for the release-evidence staleness
    // check. The capture is bounded and never aborts execution; a
    // missing Git context is recorded as no-revision so the
    // release-evidence adapter simply skips the staleness rule
    // (still applies the required-evidence rule).
    let git_ctx = git::capture(&proj.root);
    let current_revision: Option<String> = git_ctx.commit.clone();
    // Scheduled concern ids in the resolved plan, used by the
    // capability-conformance adapter to reject verified claims
    // outside the gate's vocabulary.
    let scheduled_ids: Vec<String> = resolved
        .plan
        .checks
        .iter()
        .map(|c| c.gate_id.clone())
        .collect();
    let mut results = Vec::with_capacity(resolved.plan.checks.len() + 1);
    for check in &resolved.plan.checks {
        // The built-in `source-file-size` concern is executed in-process
        // by the repository scanner; it never consults a project command
        // and never routes through the project-runtime text adapter.
        if is_source_file_size_concern(&check.gate_id) {
            results.push(crate::gate::source_size::scan(
                &proj.root,
                &resolved.source_size,
            ));
            continue;
        }
        match commands.get(check.gate_id.as_str()) {
            Some(cmd) => {
                // The deadline is resolved per check, so one check's
                // timeout can no longer decide another check's outcome.
                // A check absent from the map falls back to the default
                // rather than to 0, which would kill every command
                // instantly and report it as a timeout.
                let timeout_ms = resolved
                    .check_timeouts
                    .get(check.gate_id.as_str())
                    .copied()
                    .unwrap_or(CHECK_TIMEOUT_MS);
                let input = AdapterInput {
                    program: "sh".to_string(),
                    args: vec!["-c".to_string(), (*cmd).to_string()],
                    working_dir: proj.root.clone(),
                    env: BTreeMap::new(),
                    timeout_ms,
                    max_output_bytes: CHECK_MAX_OUTPUT_BYTES,
                    evidence: vec![],
                };
                // Concern-specific adapters (governance-quality,
                // product-quality, release-evidence,
                // capability-conformance) parse a versioned report or
                // envelope, apply their own mapping, and downgrade
                // malformed/missing/contradictory output to
                // REVIEW_REQUIRED. Every other concern keeps the
                // project-runtime text adapter unchanged.
                //
                // `governance-quality` is matched first: it is a
                // product-profile concern, but it consumes a governance
                // report scoped to a code set, not the product-quality
                // envelope, so it must not fall into that adapter.
                let result = if is_governance_quality_concern(&check.gate_id) {
                    run_governance_quality_adapter(
                        "project-runtime",
                        Some(&check.gate_id),
                        &input,
                        &resolved.governance_quality.codes,
                    )
                } else if is_product_quality_concern(&check.gate_id) {
                    run_product_quality_adapter("project-runtime", Some(&check.gate_id), &input)
                } else if is_release_evidence_concern(&check.gate_id) {
                    run_release_evidence_adapter(
                        "project-runtime",
                        Some(&check.gate_id),
                        &input,
                        current_revision.as_deref(),
                    )
                } else if is_capability_conformance_concern(&check.gate_id) {
                    run_capability_conformance_adapter(
                        "project-runtime",
                        Some(&check.gate_id),
                        &input,
                        &scheduled_ids,
                    )
                } else {
                    crate::gate::adapters::run_adapter(runtime, &input, Some(&check.gate_id))
                };
                results.push(result);
            }
            // No project command is bound to this check, so nothing ran.
            // The four cases are distinct claims and must stay distinct:
            // a forbidden exemption, an acknowledged exemption, a
            // required absence that blocks, and an optional absence that
            // never blocks.
            None => {
                let (status, severity, rule, title, remediation) = if check.exempt
                    && manifest::is_non_exempable_concern(&check.gate_id)
                {
                    // The spine is never exempable, so this manifest's
                    // claim is refused on the record rather than
                    // acknowledged: no surface may show `NOT_APPLICABLE`
                    // for a check the owner ruled can never be exempted.
                    (
                        GateStatus::Fail,
                        GateSeverity::Error,
                        "non-exempable-concern",
                        format!(
                            "not_applicable exempts {}, which may never be exempted",
                            check.gate_id
                        ),
                        format!(
                            "Bind `command` for `{}`, or drop the check. {} are never \
                             exempable: an exemption there is a reviewed workspace decision, \
                             not a manifest decision.",
                            check.gate_id,
                            manifest::NON_EXEMPTABLE_CONCERNS.join(", ")
                        ),
                    )
                } else if check.exempt {
                    let reason = manifest
                        .not_applicable
                        .get(&check.gate_id)
                        .map(|r| bound_text(r, MAX_EXEMPTION_REASON_BYTES))
                        .unwrap_or_default();
                    (
                        GateStatus::NotApplicable,
                        GateSeverity::Info,
                        "declared-exemption",
                        format!("check is exempt by manifest declaration: {reason}"),
                        "Bind `command` for this check when the project's stack grows one."
                            .to_string(),
                    )
                } else if check.required {
                    (
                        GateStatus::Unbound,
                        GateSeverity::Error,
                        "unbound-required-check",
                        "required check has no bound command; no coverage exists".to_string(),
                        "Bind `command` for this check in the gate manifest, or declare a \
                         `not_applicable` exemption with a reason."
                            .to_string(),
                    )
                } else {
                    (
                        GateStatus::Unbound,
                        GateSeverity::Warning,
                        "unbound-optional-check",
                        "optional check has no bound command; it was never attempted".to_string(),
                        "Bind `command` for this optional check to gain real coverage.".to_string(),
                    )
                };
                results.push(GateResult {
                    gate_id: truncate_char_boundary(&check.gate_id, 128),
                    source: truncate_char_boundary(&check.source, 256),
                    status,
                    severity,
                    findings: vec![Finding {
                        title: bound_text(&title, MAX_DIAGNOSTIC_BYTES),
                        severity,
                        location: Some(if check.exempt {
                            truncate_char_boundary(
                                &format!("not_applicable.{}", check.gate_id),
                                256,
                            )
                        } else {
                            truncate_char_boundary(&format!("commands.{}", check.gate_id), 256)
                        }),
                        rule: Some(rule.to_string()),
                    }],
                    evidence: vec![],
                    missing_evidence: vec![format!("{}:command", check.gate_id)],
                    diagnostic: Some(bound_text(
                        "no command declared for this concern; no execution was attempted",
                        MAX_DIAGNOSTIC_BYTES,
                    )),
                    remediation: Some(bound_text(&remediation, MAX_REMEDIATION_BYTES)),
                });
            }
        }
    }
    let mut plan = resolved.plan.clone();
    // Exemption budget: one plan-level result for the whole manifest,
    // carrying the owner's per-repo allowance. A failing budget becomes a
    // required plan check so it blocks the aggregate independently of the
    // project's `blocking:` list and of whether the exempted concerns were
    // themselves marked required; an exhausted budget becomes an optional
    // one so the warning is recorded and never blocks. Existing statuses
    // are reused deliberately, so no render, persistence, or evidence
    // consumer needs a new arm.
    let budget_verdict = resolved.exemption_budget.verdict();
    if budget_verdict != ExemptionVerdict::Within {
        // Never collide with a project-declared concern of the same
        // name: a repo that declares its own `exemption-budget` check
        // must not be able to hide its budget verdict behind it.
        let mut budget_id = manifest::EXEMPTION_BUDGET_CHECK.to_string();
        let mut suffix = 2;
        while plan.checks.iter().any(|c| c.gate_id == budget_id) {
            budget_id = format!("{}-{suffix}", manifest::EXEMPTION_BUDGET_CHECK);
            suffix += 1;
        }
        let blocking = matches!(
            budget_verdict,
            ExemptionVerdict::OverCap | ExemptionVerdict::HardFail
        );
        let (status, severity, rule) = if blocking {
            (
                GateStatus::Fail,
                GateSeverity::Error,
                "exemption-budget-exceeded",
            )
        } else {
            (
                GateStatus::ReviewRequired,
                GateSeverity::Warning,
                "exemption-budget-exhausted",
            )
        };
        let diagnostic = resolved.exemption_budget.diagnostic();
        let remediation = if blocking {
            format!(
                "Bind a command for the exempted checks, drop the checks, or take the \
                 exemption through review: a manifest may declare at most {} of its {} \
                 required checks not applicable.",
                resolved.exemption_budget.cap, resolved.exemption_budget.required_checks
            )
        } else {
            "No headroom is left for another exemption: bind a command for the next check \
             that would need one, or review the manifest before widening the hole."
                .to_string()
        };
        results.push(GateResult {
            gate_id: budget_id.clone(),
            source: "manifest".to_string(),
            status,
            severity,
            findings: vec![Finding {
                title: bound_text(&diagnostic, MAX_DIAGNOSTIC_BYTES),
                severity,
                location: Some("not_applicable".to_string()),
                rule: Some(rule.to_string()),
            }],
            evidence: vec![],
            missing_evidence: vec![],
            diagnostic: Some(bound_text(&diagnostic, MAX_DIAGNOSTIC_BYTES)),
            remediation: Some(bound_text(&remediation, MAX_REMEDIATION_BYTES)),
        });
        plan.checks.push(PlannedCheck {
            gate_id: budget_id,
            source: "manifest".to_string(),
            required: blocking,
            exempt: false,
        });
        plan.checks.sort_by(|a, b| a.gate_id.cmp(&b.gate_id));
    }
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
                    exempt: false,
                });
                plan.checks.sort_by(|a, b| a.gate_id.cmp(&b.gate_id));
            }
        }
    }
    Ok((results, plan))
}

/// Persisted run payload: the versioned result list plus the unbound
/// collection, so a stored run alone distinguishes a proven
/// `NOT_APPLICABLE` from a never-attempted `UNBOUND` and coverage debt
/// is queryable without re-running the gate. No schema change: the
/// existing `gate_runs.results_json` column carries both.
fn bound_results_json(results: &[GateResult], unbound: &[String]) -> String {
    let json = to_gate_run_json(results, unbound);
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
    // Unbound is its own line, never merged into the not-applicable
    // list: these are checks whose coverage was asserted and never
    // attempted, not proven exemptions.
    let required_unbound: Vec<&str> = outcome
        .unbound
        .iter()
        .filter(|id| !id.starts_with("optional:"))
        .map(|id| id.as_str())
        .collect();
    if !required_unbound.is_empty() {
        println!("Unbound required checks: {}", required_unbound.join(", "));
        println!(
            "Remediation: bind `command` for each, or declare a `not_applicable` exemption \
             with a reason. No command ran, so this coverage does not exist."
        );
    }
    let optional_unbound: Vec<&str> = outcome
        .unbound
        .iter()
        .filter_map(|id| id.strip_prefix("optional:"))
        .collect();
    if !optional_unbound.is_empty() {
        println!("Unbound optional checks: {}", optional_unbound.join(", "));
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
        "unbound": outcome.unbound,
        "exemption_budget": resolved.exemption_budget,
        "manifest_digest": resolved.manifest_digest,
        "rule_pack_version": resolved.rule_pack_version,
        "results": results,
    });
    println!("{}", serde_json::to_string_pretty(&doc).unwrap_or_default());
}

/// Emit the `platform.gate-result` document, or refuse.
///
/// The document is built completely in memory before anything is printed,
/// so there is no partial or coerced output on any path. On refusal stdout
/// stays empty, one typed diagnostic naming the exact offending value goes to
/// stderr, and the process exits [`EXIT_UNREPRESENTABLE`].
///
/// The refusal set is not a defensive edge. Every status has an honest value
/// in contract major 1.0.0, but the contract still bounds `check_id`, the
/// diagnostic detail, and every evidence reference — and driftwatchdog check
/// ids come from a project manifest that bounds them only as non-empty. A run
/// carrying an identifier or a diagnostic that does not fit is declined rather
/// than shortened, and the diagnostic names exactly which value caused it.
fn print_platform(
    outcome: &AggregateOutcome,
    results: &[GateResult],
    resolved: &ResolvedGatePlan,
    evaluated_at: &str,
    subject_ref: &str,
) -> i32 {
    match platform_result::build(PlatformRun {
        gate_id: &resolved.manifest_digest,
        pipeline: PLATFORM_PIPELINE,
        profile: &resolved.profile,
        evaluated_at,
        subject_ref: Some(subject_ref),
        aggregate: outcome.status,
        results,
    }) {
        Ok(doc) => {
            println!("{}", serde_json::to_string_pretty(&doc).unwrap_or_default());
            if outcome.blocked {
                EXIT_BLOCKED
            } else {
                EXIT_OK
            }
        }
        Err(refusal) => {
            eprintln!(
                "driftwatch gate: refusing to emit a platform gate-result document: {refusal}"
            );
            eprintln!(
                "driftwatch gate: nothing was written to stdout; use --format json for \
                 driftwatchdog's own machine-readable status document."
            );
            EXIT_UNREPRESENTABLE
        }
    }
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
            "the gate manifest is configured but `driftwatch gate` has never run here.".to_string(),
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
#[path = "gate_tests.rs"]
mod tests;
