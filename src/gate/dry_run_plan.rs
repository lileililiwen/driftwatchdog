//! Machine-readable **rehearsal** of a `driftwatch gate --dry-run`.
//!
//! `--dry-run` resolves the plan, renders it, and stops. Until this module
//! existed it also ignored `--format`: the branch in
//! [`crate::commands::gate`] returned before the format dispatch, so
//! `--dry-run --format json` printed the same prose as `--format human` and
//! a consumer that asked for JSON had to scrape text.
//!
//! The obvious fix — move the branch below the dispatch and let the existing
//! JSON writer run — is impossible and would be dishonest anyway. That writer
//! composes its document from `AggregateOutcome` and `Vec<GateResult>`, which
//! are products of *execution*. A rehearsal has neither. Emitting it would
//! mean inventing the very values a rehearsal must never claim.
//!
//! So this is a **third document kind**, not a variant of the status
//! document:
//!
//! * It is composed from [`ResolvedGatePlan`] and [`GateManifest`] alone. It
//!   reads no clock, spawns no child, opens no database and writes no file.
//! * It **declares itself a rehearsal three times over**: a `dry_run: true`
//!   marker, a document-kind field, and a versioned root `contract`
//!   constant — the shape already used by `driftwatch-checker/0.1.0`
//!   ([`crate::checker::json`]) and `platform.gate-result/1.0.0`
//!   ([`crate::gate::platform_result`]).
//! * It **carries no verdict-shaped key at any depth**. Not `status`, not
//!   `result`, not `passed`, not `blocked`: those are the keys the real
//!   `--format json` document uses for its verdict, and a rehearsal must not
//!   be a near-miss of it. [`VERDICT_LIKE_KEYS`] is the closed deny-set and
//!   [`verdict_like_keys`] walks the emitted document against it; the build
//!   debug-asserts its own output, so a future edit that adds one of these
//!   keys fails the test suite instead of shipping a fake verdict.
//! * Planned checks carry an id, a required flag, an exemption flag, the
//!   manifest layer that selected them, where their command would come from,
//!   and their effective deadline. They carry **no status**, because nothing
//!   ran and there is nothing to report.
//!
//! The one judgement call is [`BlockingPrediction`]. A rehearsal that cannot
//! answer "would this plan block?" is not worth running, and the resolved
//! plan answers it in exactly one direction: see that type's documentation
//! for what is knowable before execution and what is not.

use serde::Serialize;

use crate::cli::GateOutputFormat;
use crate::gate::aggregate::PlannedCheck;
use crate::gate::concerns::is_source_file_size_concern;
use crate::gate::manifest::DEFAULT_CHECK_TIMEOUT_MS;
use crate::gate::manifest::{ExemptionVerdict, GateManifest, ResolvedGatePlan};

#[cfg(test)]
#[path = "dry_run_plan_tests.rs"]
mod tests;

/// Versioned contract id of the dry-run rehearsal document.
///
/// Shape follows the two constants already in this repository — a root
/// `contract` field carrying `name/major.minor.patch`. Bump only when the wire
/// shape changes in a non-additive way (a renamed, removed or retyped field,
/// or a change to the [`BlockingPrediction`] vocabulary, which a consumer
/// cannot interpret forward-compatibly).
pub const DRY_RUN_PLAN_CONTRACT: &str = "driftwatch-gate-plan/0.1.0";

/// The only value `document_kind` ever takes in a document this module
/// emits. It exists so a consumer that has learned to recognise *one*
/// rehearsal document does not have to infer it from the absence of fields.
pub const DRY_RUN_DOCUMENT_KIND: &str = "gate-plan-rehearsal";

/// Keys that state the **outcome of execution** and therefore MUST NOT appear
/// anywhere in a rehearsal document — not at the root, not inside
/// `planned_checks[]`, not nested in any object or array.
///
/// This is the closed list the no-verdict rule is enforced against. It is a
/// production constant rather than a literal inside a test so the rule has
/// exactly one owner, and it is deliberately broader than
/// [`crate::gate::aggregate::AggregateOutcome`]'s field names: the threat is a
/// future edit or a consumer's heuristic, not just today's struct.
///
/// `would_block` is intentionally absent — see [`BlockingPrediction`].
pub const VERDICT_LIKE_KEYS: &[&str] = &[
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

/// Every key in `value`, at every depth, that appears in
/// [`VERDICT_LIKE_KEYS`]. Empty means the document cannot be read as a
/// verdict. Paths are returned with the offending key named, so a failure says
/// where the hole is.
pub fn verdict_like_keys(value: &serde_json::Value) -> Vec<String> {
    let mut found = Vec::new();
    collect_verdict_like_keys(value, String::new(), &mut found);
    found
}

fn collect_verdict_like_keys(value: &serde_json::Value, path: String, found: &mut Vec<String>) {
    match value {
        serde_json::Value::Object(map) => {
            for (key, child) in map {
                let child_path = if path.is_empty() {
                    key.clone()
                } else {
                    format!("{path}.{key}")
                };
                if VERDICT_LIKE_KEYS.contains(&key.as_str()) {
                    found.push(child_path.clone());
                }
                collect_verdict_like_keys(child, child_path, found);
            }
        }
        serde_json::Value::Array(items) => {
            for (index, child) in items.iter().enumerate() {
                collect_verdict_like_keys(child, format!("{path}[{index}]"), found);
            }
        }
        _ => {}
    }
}

/// Where the command a planned check *would* run comes from.
///
/// A rehearsal can say this much: it is a fact about the manifest, decided at
/// resolution time, and it is the reason
/// [`BlockingPrediction`] can exist at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PlannedCommandSource {
    /// The runtime executes this concern in-process (`source-file-size`);
    /// no command binding is expected or needed.
    InProcess,
    /// The manifest binds a `command` for this check.
    Manifest,
    /// Nothing is bound. On a required check this is the coverage debt
    /// [`ResolvedGatePlan::unbound`] already lists.
    None,
}

/// One planned check, described. Deliberately not a
/// [`crate::gate::types::GateResult`]: this row has no `status`, no
/// `severity`, no findings and no diagnostic, because nothing ran.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PlannedCheckRow {
    pub id: String,
    pub required: bool,
    pub exempt: bool,
    /// The manifest layer that scheduled this check (`manifest`, a profile
    /// default, a changed-surface trigger). Renamed from `PlannedCheck`'s
    /// `source` so it cannot be confused with a command origin.
    pub selected_by: String,
    pub command_source: PlannedCommandSource,
    pub timeout_ms: u64,
}

/// Why the resolved plan alone already forces a blocking outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum BlockingReasonRule {
    /// A required check asserts coverage, binds no command and declares no
    /// exemption. `execute_plan` records it `UNBOUND`, and `aggregate`
    /// turns a required `UNBOUND` into a failure.
    UnboundRequiredCheck,
    /// The manifest declared more exemptions than its allowance, or exempted
    /// a never-exempable concern. Both become a required `FAIL` result.
    ExemptionBudgetExceeded,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BlockingReason {
    pub rule: BlockingReasonRule,
    pub checks: Vec<String>,
}

/// The two answers the plan can give. There is no third: the plan can prove a
/// block and can never prove a pass, so the honest unknown is its own variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WouldBlock {
    /// The resolved plan alone forces `blocked: true`, whatever every bound
    /// command does. Serialized as `true`.
    Forced,
    /// The plan alone cannot decide: the bound commands decide. Serialized as
    /// `null` — **never** `false`, because `false` is a claim that nothing
    /// ran and passed.
    Undetermined,
}

impl WouldBlock {
    fn as_value(self) -> serde_json::Value {
        match self {
            WouldBlock::Forced => serde_json::Value::Bool(true),
            WouldBlock::Undetermined => serde_json::Value::Null,
        }
    }
}

/// A plan-level **prediction**, never an observed verdict.
///
/// # What is knowable before execution
///
/// [`crate::gate::aggregate::aggregate`] derives `blocked` from results, so
/// the outcome of every check with a bound command is unknowable until it
/// runs. Two plan facts, however, are decided at resolution time and are
/// independent of any child's exit code:
///
/// 1. [`ResolvedGatePlan::unbound`] — required, non-exempt, non-in-process
///    checks with no bound command. `execute_plan` gives each of them
///    `UNBOUND`, and `aggregate` turns a required `UNBOUND` into a failure,
///    so one such check forces a block whatever everything else does.
/// 2. [`ResolvedGatePlan::exemption_budget`] — `OverCap` and `HardFail`
///    become a required `FAIL` plan-level result and also force a block.
///    `Exhausted` becomes an *optional* `REVIEW_REQUIRED` and forces nothing.
///
/// The condition is therefore **sufficient but not necessary**. `Forced` means
/// "this will block". `Undetermined` means "the bound commands decide" — and
/// it is the *only* other answer this type can produce, so a consumer can
/// never read this document as a passing rehearsal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockingPrediction {
    pub would_block: WouldBlock,
    pub reasons: Vec<BlockingReason>,
}

impl BlockingPrediction {
    /// Derive the prediction from the resolved plan alone. No execution, no
    /// child process, no result.
    pub fn from_resolved(resolved: &ResolvedGatePlan) -> Self {
        let mut reasons: Vec<BlockingReason> = Vec::new();

        if !resolved.unbound.is_empty() {
            reasons.push(BlockingReason {
                rule: BlockingReasonRule::UnboundRequiredCheck,
                checks: resolved.unbound.clone(),
            });
        }

        let verdict = resolved.exemption_budget.verdict();
        if matches!(
            verdict,
            ExemptionVerdict::OverCap | ExemptionVerdict::HardFail
        ) {
            reasons.push(BlockingReason {
                rule: BlockingReasonRule::ExemptionBudgetExceeded,
                checks: vec![crate::gate::manifest::EXEMPTION_BUDGET_CHECK.to_string()],
            });
        }

        BlockingPrediction {
            would_block: if reasons.is_empty() {
                WouldBlock::Undetermined
            } else {
                WouldBlock::Forced
            },
            reasons,
        }
    }

    fn as_value(&self) -> serde_json::Value {
        let kind = match self.would_block {
            WouldBlock::Forced => "plan-forced-block",
            WouldBlock::Undetermined => "undetermined-before-execution",
        };
        let note = match self.would_block {
            WouldBlock::Forced => concat!(
                "A prediction from the resolved plan alone, never an observed outcome. ",
                "This plan already forces a blocking result whatever the bound checks do. ",
                "It says nothing about whether a bound check will also fail or need review, ",
                "and this document never predicts that a gate passes."
            ),
            WouldBlock::Undetermined => concat!(
                "A prediction from the resolved plan alone, never an observed outcome. ",
                "The plan cannot decide: the bound commands decide. A null here means ",
                "unknown, never a pass. Nothing has been executed."
            ),
        };
        // `kind`, not `outcome`: `outcome` is on the deny-set above and the
        // rule must have no exceptions.
        serde_json::json!({
            "kind": kind,
            "derived_from": "resolved-plan",
            "would_block": self.would_block.as_value(),
            "sufficient_but_not_necessary": true,
            "reasons": self.reasons,
            "note": note,
        })
    }
}

/// What a rehearsal renders, decided without printing so it can be asserted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DryRunOutput {
    /// The human plan text, byte-identical to
    /// [`crate::gate::manifest::render_plan`].
    Plan(String),
    /// The machine-readable rehearsal document.
    Document(serde_json::Value),
}

/// Decide what a rehearsal renders.
///
/// `Human` reproduces [`crate::gate::manifest::render_plan`] exactly — the
/// same function, the same string. `Platform` renders that same plan text,
/// which is today's behaviour for `--dry-run --format platform`:
/// `platform.gate-result` is an *outcome* contract with no value for
/// "planned"; refusing with exit `2` would invent a semantic
/// `EXIT_UNREPRESENTABLE` does not mean, and emitting anything else would be
/// exactly the coercion that format was built to refuse. The combination is
/// therefore deliberately unchanged and pinned by a test, rather than left
/// accidental. `Json` is the rehearsal document.
pub fn dry_run_output(
    format: &GateOutputFormat,
    resolved: &ResolvedGatePlan,
    manifest: &GateManifest,
) -> DryRunOutput {
    match format {
        GateOutputFormat::Human | GateOutputFormat::Platform => {
            DryRunOutput::Plan(crate::gate::manifest::render_plan(resolved))
        }
        GateOutputFormat::Json => DryRunOutput::Document(build_document(resolved, manifest)),
    }
}

/// Render the rehearsal. The exit code is the caller's: a rehearsal returns
/// the same code for every document it can print, so the exit status can
/// never depend on what the plan describes.
pub fn print_dry_run(
    format: &GateOutputFormat,
    resolved: &ResolvedGatePlan,
    manifest: &GateManifest,
) {
    match dry_run_output(format, resolved, manifest) {
        DryRunOutput::Plan(plan) => print!("{plan}"),
        DryRunOutput::Document(doc) => {
            println!("{}", serde_json::to_string_pretty(&doc).unwrap_or_default());
        }
    }
}

/// Compose the rehearsal document. Pure: a function of an already-resolved
/// plan and the already-loaded manifest.
pub fn build_document(resolved: &ResolvedGatePlan, manifest: &GateManifest) -> serde_json::Value {
    let prediction = BlockingPrediction::from_resolved(resolved);
    let doc = serde_json::json!({
        "contract": DRY_RUN_PLAN_CONTRACT,
        "document_kind": DRY_RUN_DOCUMENT_KIND,
        "dry_run": true,
        "execution_mode": resolved.execution_mode,
        "executed": false,
        "persisted": false,
        "notice": concat!(
            "A rehearsal of a resolved gate plan. Nothing was executed, nothing was ",
            "persisted, and no check in this document has an outcome: it describes ",
            "what would run, not what ran."
        ),
        "plan_version": resolved.plan.version,
        "profile": resolved.profile,
        "rule_pack_version": resolved.rule_pack_version,
        "manifest_digest": resolved.manifest_digest,
        "blocking_policy": resolved.plan.policy,
        "default_timeout_ms": DEFAULT_CHECK_TIMEOUT_MS,
        "planned_checks": planned_check_rows(resolved, manifest),
        "exempted_checks": resolved.exemptions,
        "unbound_required_checks": resolved.unbound,
        "exemption_budget": {
            "required_checks": resolved.exemption_budget.required_checks,
            "cap": resolved.exemption_budget.cap,
            "declared": resolved.exemption_budget.declared,
            "headroom": resolved.exemption_budget.headroom(),
            "verdict": verdict_word(resolved.exemption_budget.verdict()),
            "never_exempable": resolved.exemption_budget.non_exempable,
        },
        "source_size_policy": resolved.source_size,
        "governance_quality_codes": resolved.governance_quality.codes,
        "context_providers": resolved.contexts,
        "project_command_names": resolved.project_commands.keys().collect::<Vec<_>>(),
        "not_scheduled": resolved.not_scheduled,
        "blocking_prediction": prediction.as_value(),
    });

    // The no-verdict rule has no exception. If a future edit adds a
    // verdict-shaped key, that is a failure the suite must see, not a
    // document that ships.
    debug_assert!(
        verdict_like_keys(&doc).is_empty(),
        "a rehearsal document must not carry a verdict-shaped key: {:?}",
        verdict_like_keys(&doc)
    );
    doc
}

/// One row per planned check: what it is, where its command would come from,
/// and its effective deadline. No outcome, because there is none.
fn planned_check_rows(
    resolved: &ResolvedGatePlan,
    manifest: &GateManifest,
) -> Vec<PlannedCheckRow> {
    resolved
        .plan
        .checks
        .iter()
        .map(|check: &PlannedCheck| PlannedCheckRow {
            id: check.gate_id.clone(),
            required: check.required,
            exempt: check.exempt,
            selected_by: check.source.clone(),
            command_source: command_source(manifest, check),
            timeout_ms: resolved
                .check_timeouts
                .get(check.gate_id.as_str())
                .copied()
                .unwrap_or(DEFAULT_CHECK_TIMEOUT_MS),
        })
        .collect()
}

/// Mirrors [`crate::gate::manifest::unbound_required_checks`]'s per-check
/// predicate: a check the runtime executes in-process needs no binding, a
/// manifest-bound `command` is a binding, and anything else is none. The
/// in-process predicate is shared, so the two cannot drift on the scanner
/// exception.
fn command_source(manifest: &GateManifest, check: &PlannedCheck) -> PlannedCommandSource {
    if is_source_file_size_concern(&check.gate_id) {
        PlannedCommandSource::InProcess
    } else if manifest
        .checks
        .iter()
        .any(|c| c.id == check.gate_id && c.enabled && c.command.is_some())
    {
        PlannedCommandSource::Manifest
    } else {
        // An optional check or an exempted one with no binding is not
        // coverage debt, but it equally has no command to name.
        PlannedCommandSource::None
    }
}

/// [`ExemptionVerdict`] is not serializable (it is a policy decision, not a
/// persisted state), and the rehearsal wants the word a reader can act on.
fn verdict_word(verdict: ExemptionVerdict) -> &'static str {
    match verdict {
        ExemptionVerdict::Within => "within",
        ExemptionVerdict::Exhausted => "exhausted",
        ExemptionVerdict::OverCap => "over-cap",
        ExemptionVerdict::HardFail => "hard-fail",
    }
}
