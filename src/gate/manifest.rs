//! Project Gate manifest: portable policy without tool implementations.
//!
//! A manifest declares **policy** (profile, selected checks, blocking
//! policy, project commands, rule-pack version, context providers, and
//! optional changed-surface triggers). It never installs tools, makes
//! network calls, or embeds language-specific logic.
//!
//! ## Precedence with `driftwatch.toml`
//!
//! * `driftwatch.toml` remains authoritative for checker **execution**
//!   (`driftwatch check` behavior, timeouts, working dirs). It is
//!   untouched by this module.
//! * The Gate manifest is an additive policy layer. [`manifest_path`]
//!   resolves `<root>/gate.toml` first, then `<root>/.driftwatch/gate.toml`
//!   (state-root confinement friendly). When neither exists, [`load`]
//!   returns `Ok(None)`: no gate is configured, and callers must not
//!   treat absent optional concerns as failures.
//! * A standalone `gate.toml` takes precedence over any future inline
//!   `[gate]` table; no inline table is read today.
//!
//! ## Profiles (runtime-provided defaults)
//!
//! * `backend` — `api-contract`, `migration`, `secret-scan`.
//!   No browser/responsive check is scheduled.
//! * `frontend` — `responsive`, `a11y`, `api-contract`.
//! * `full` — union of backend + frontend defaults.
//! * `minimal` — no default checks; only explicit checks apply.
//! * `product` / `rust-product` — the three product-quality concern
//!   ids (`product-code-boundary`, `placeholder-threshold`,
//!   `source-file-size`). The profile is data only and never invents a
//!   command; a project that picks either profile without binding
//!   `commands.<id>` has an *unbound* required check, which fails the
//!   gate as `UNBOUND` rather than being reported as a proven
//!   exemption, except for `source-file-size`, which runs the
//!   in-process scanner and needs no command at all.
//! * `release` — the two release-gate concern ids
//!   (`capability-conformance`, `release-evidence`). The profile is
//!   data only; a project that picks `release` without binding
//!   `commands.<id>` has an unbound required check and fails on
//!   `UNBOUND`. Driftwatchdog does not become a release publisher,
//!   signer, SBOM generator, or deployment executor.
//!
//! Projects may enable extra checks or relax a profile default to
//! `required = false`. Tightening an optional default to required is
//! allowed; arbitrary override fields are rejected.
//!
//! ## Command binding
//!
//! A check that asserts coverage but binds no command has *never been
//! attempted*. Two manifest-level declarations decide how that absence
//! is treated, and both are recorded rather than inferred:
//!
//! * `not_applicable.<id> = "<reason>"` — a written, per-check
//!   exemption for a check the project's stack genuinely does not
//!   implement. The reason is length-bounded and credential-redacted
//!   at parse time. An exemption naming an unselected check, or one
//!   that also binds a command, is a parse-time error.
//! * `binding_policy = "report" | "reject"` — what happens to a
//!   required, non-exempt check that binds no command. `report` (the
//!   default) records the result as `UNBOUND`, which blocks the gate.
//!   `reject` refuses to resolve at all, naming every offending check
//!   in one refusal, before any check executes and before any run is
//!   persisted.
//!
//! Exemptions are budgeted. [`exemption_cap`] gives one project
//! `min(floor(0.25 x required_checks), 5)` allowances, computed from that
//! manifest's own required-check count, and every declared
//! `not_applicable` entry consumes one. The four concerns in
//! [`NON_EXEMPTABLE_CONCERNS`] are never exempable whatever the budget
//! says. The budget is a verdict, not a refusal: an over-cap or
//! spine-violating manifest still parses and still runs, and the outcome
//! is recorded in the plan (and therefore in `--dry-run`) as
//! [`ResolvedGatePlan::exemption_budget`].
//!
//! ## Triggers
//!
//! A trigger maps a changed-surface substring (e.g. `frontend/`) to a
//! concern id. [`resolve`] takes the observed changed surfaces and
//! includes matching concerns. Matching is a plain substring on the
//! surface path: no glob engine, no filesystem access.
//!
//! ## Errors
//!
//! Unknown fields fail with the field name plus a `did you mean`
//! remediation hint. An enabled check with an empty command, an
//! unknown profile, a malformed project command, a malformed
//! exemption, or a `binding_policy` refusal fails plan resolution
//! **before any check executes**. [`render_plan`] is pure: it prints
//! the resolved plan and never spawns a child process or mutates
//! state.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::gate::aggregate::{BlockingPolicy, GatePlan, PlannedCheck};
use crate::gate::concerns::{
    is_source_file_size_concern, DEPLOYABLE_CONCERNS, PRODUCT_PROFILE_CONCERNS,
    RELEASE_GATE_CONCERNS,
};
use crate::gate::dto::GATE_CONTRACT_VERSION;
use crate::gate::redact::bound_text;

/// Profile defaults. Kept as functions so the contract stays
/// language- and specification-system-neutral.
pub fn profile_defaults(profile: &str) -> Option<&'static [&'static str]> {
    match profile {
        "backend" => Some(&["api-contract", "migration", "secret-scan"]),
        "frontend" => Some(&["a11y", "api-contract", "responsive"]),
        "full" => Some(&[
            "a11y",
            "api-contract",
            "migration",
            "responsive",
            "secret-scan",
        ]),
        "minimal" => Some(&[]),
        // Product-quality profiles select the stable product-quality
        // concern ids, the built-in scanner, and the code-scoped
        // `governance-quality` concern. Selection is data only and does
        // not invent a command; a project that picks `product` (or
        // `rust-product`) without binding `commands.<id>` has an
        // unbound required check and fails on UNBOUND. That includes
        // `governance-quality`: the governance checker lives outside
        // this repository, so there is no path to default to and no
        // binding here is ever inferred. The `source-file-size` concern
        // is the one exception: it is executed by the in-process
        // scanner and needs no command binding.
        "product" | "rust-product" => Some(PRODUCT_PROFILE_CONCERNS),
        // Release-gate profile selects the two stable release-gate
        // concern ids. The profile is data only and never invents a
        // command; a project that picks `release` without binding
        // `commands.<id>` has an unbound required check and fails on
        // UNBOUND. Driftwatchdog does not become a release publisher,
        // signer, SBOM generator, or deployment executor.
        "release" => Some(RELEASE_GATE_CONCERNS),
        // Deployable projects must bind both project-owned contract commands;
        // the profile supplies required checks but never invents commands.
        "deployable" => Some(DEPLOYABLE_CONCERNS),
        _ => None,
    }
}

/// Supported profile names, used in diagnostics.
pub const SUPPORTED_PROFILES: &[&str] = &[
    "backend",
    "frontend",
    "full",
    "minimal",
    "product",
    "rust-product",
    "release",
    "deployable",
];

/// Resolve the default concern set for `profile` in the context of a
/// concrete manifest: a built-in profile uses [`profile_defaults`];
/// otherwise a project-defined `[profiles.<name>]` entry is consulted.
/// Returns `None` only when the profile is neither built-in nor declared,
/// which is the same condition [`validate`] rejects.
pub fn profile_defaults_for(manifest: &GateManifest, profile: &str) -> Option<Vec<String>> {
    if let Some(builtin) = profile_defaults(profile) {
        return Some(builtin.iter().map(|s| (*s).to_string()).collect());
    }
    manifest.profiles.get(profile).cloned()
}

/// True when `profile` is a built-in name (not a project-defined one).
pub fn is_builtin_profile(profile: &str) -> bool {
    profile_defaults(profile).is_some()
}

/// On-disk manifest model. Unknown fields are rejected so typos
/// surface immediately instead of silently changing policy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GateManifest {
    /// Manifest schema version. Must equal [`GATE_CONTRACT_VERSION`].
    #[serde(default = "default_manifest_version")]
    pub version: u32,
    /// Profile name (see [`SUPPORTED_PROFILES`]).
    pub profile: String,
    /// Explicit check declarations (additive over profile defaults).
    #[serde(default)]
    pub checks: Vec<ManifestCheck>,
    /// Blocking policy override.
    #[serde(default)]
    pub blocking: ManifestBlocking,
    /// Named project commands (`name = "command ..."`) kept separate
    /// from gate-tool commands. Values must be non-empty.
    #[serde(default)]
    pub project_commands: BTreeMap<String, String>,
    /// Rule-pack version identity (default: `local`).
    #[serde(default = "default_rule_pack_version")]
    pub rule_pack_version: String,
    /// Context provider declarations (generic names only; the core
    /// never depends on OpenSpec or any provider SDK types).
    #[serde(default)]
    pub contexts: Vec<ContextDecl>,
    /// Changed-surface triggers.
    #[serde(default)]
    pub triggers: Vec<Trigger>,
    /// Project-defined domain profiles: `profile` may name a built-in
    /// (`backend`, `frontend`, `full`, `minimal`) or a key declared here.
    /// The value is the concern set the profile selects (its default
    /// checks; explicit `[[checks]]` still add to or relax it). Empty
    /// maps are not serialized so existing `gate.toml` digests stay
    /// stable.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub profiles: BTreeMap<String, Vec<String>>,
    /// Built-in `source-file-size` policy. Defaults are not serialized
    /// so manifests that never set `[source_size]` keep a stable digest.
    #[serde(default, skip_serializing_if = "SourceSizePolicy::is_default")]
    pub source_size: SourceSizePolicy,
    /// Written per-check exemptions: `not_applicable.<id> = "<reason>"`.
    /// A reason states *why* the project's stack has no implementation
    /// for a check the plan asserts. Values are length-bounded and
    /// credential-redacted at parse time, before they are rendered or
    /// persisted. Empty maps are not serialized so existing `gate.toml`
    /// digests stay stable.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub not_applicable: BTreeMap<String, String>,
    /// What to do with a required, non-exempt check that binds no
    /// command. Not serialized when left at the default so existing
    /// `gate.toml` digests stay stable.
    #[serde(
        default,
        deserialize_with = "de_binding_policy",
        skip_serializing_if = "BindingPolicy::is_default"
    )]
    pub binding_policy: BindingPolicy,
    /// The `governance-quality` concern's code scope (defaults to the
    /// concern's full declared set). Not serialized when left at the
    /// default so existing manifest digests stay stable.
    #[serde(default, skip_serializing_if = "GovernanceQualityPolicy::is_default")]
    pub governance_quality: GovernanceQualityPolicy,
}

/// Accepted `binding_policy` values, in diagnostic order.
pub const BINDING_POLICIES: &[&str] = &["report", "reject"];

/// Upper bound applied to an exemption reason before it is rendered or
/// persisted. Mirrors the diagnostic/remediation bounds so a reason can
/// never become an unbounded storage or rendering sink.
pub const MAX_EXEMPTION_REASON_BYTES: usize = 2048;

/// Gate result id carrying one project's exemption-budget verdict.
///
/// It is a plan-level result rather than a per-check one, because the
/// budget is a property of the manifest as a whole. The id is appended to
/// the effective plan exactly the way the optional `ai-review` check is,
/// so the aggregate, the persisted run, and every render carry the verdict
/// with no new status and no new consumer.
pub const EXEMPTION_BUDGET_CHECK: &str = "exemption-budget";

/// The concerns that may never be exempted, whatever a project's budget
/// says.
///
/// The floor every other check stands on: without a bound command, a
/// build, test, security, or repository-integrity gate is the one hole a
/// project must not be allowed to declare away. Owner decision,
/// 2026-10-02; see
/// `openspec/changes/gate-unbound-check-fails/design.md` §3.5.
///
/// This is the canonical list, used for diagnostics and the plan render.
/// It is **not** what the matcher compares against: this workspace spells
/// the repository-integrity concern two ways, so matching walks
/// [`NON_EXEMPTABLE_ALIASES`] instead. Do not add a spelling here — add
/// it to the alias table, where it is visibly attached to a concern.
///
/// `governance-quality` is here for a different reason than the spine:
/// the spine is the floor every other check stands on, while this
/// concern owns an ERROR-severity finding code whose finding *is* the
/// defect the gate exists to report. An exemption removes a required
/// check from the gate; here that would remove the report of a
/// contradicted claim and leave nothing behind. See
/// `openspec/changes/bind-governance-quality-checks/design.md` §4.
pub const NON_EXEMPTABLE_CONCERNS: &[&str] = &[
    "build",
    "tests",
    "security",
    "repository",
    "governance-quality",
];

/// Every workspace spelling protected for each non-exempable concern:
/// the concern, then the ids that name it in a real `.ai-gate/gate.yaml`.
///
/// Aliasing is deliberate, not redundancy. The owner's ruling named
/// `repository`, and that is the spelling 63 of 76 workspace manifests
/// use — but `repository_integrity` is a distinct, equally real id used
/// by `labrys` and `openpanel`. A bare comparison against the canonical
/// list would let those two repos exempt the concern the spine exists to
/// protect, which is the exact escape the cap was written to close. A
/// future spelling must be added here, against the concern it names, so
/// the decision to widen the floor is a visible one.
pub const NON_EXEMPTABLE_ALIASES: &[(&str, &[&str])] = &[
    ("build", &["build"]),
    ("tests", &["tests"]),
    ("security", &["security"]),
    ("repository", &["repository", "repository_integrity"]),
    // One spelling, one concern, no alias: the alias table exists for
    // spellings a real `.ai-gate/gate.yaml` uses, and a concern with a
    // single id does not need a second name to be protected.
    ("governance-quality", &["governance-quality"]),
];

/// Absolute ceiling on exemptions, independent of manifest size.
///
/// The percentage alone would let a 40-check manifest exempt a quarter of
/// its gate; this bound is what keeps a large project from buying a wider
/// hole than a small one.
pub const MAX_EXEMPTION_CAP: usize = 5;

/// True when `id` names one of the non-exempable concerns under any
/// workspace spelling. Matches against [`NON_EXEMPTABLE_ALIASES`], so
/// an alias is protected exactly as the canonical id is.
pub fn is_non_exempable_concern(id: &str) -> bool {
    NON_EXEMPTABLE_ALIASES
        .iter()
        .any(|(_, spellings)| spellings.contains(&id))
}

/// Exemption allowance for a project that selects `required_checks`
/// required checks: `min(floor(0.25 x required_checks), 5)`.
///
/// Integer floor division by 4 is exactly `floor(0.25 x n)`; the `min`
/// binds from 20 required checks upward. A project selecting fewer than
/// four required checks gets an allowance of zero and may not exempt
/// anything.
pub fn exemption_cap(required_checks: usize) -> usize {
    (required_checks / 4).min(MAX_EXEMPTION_CAP)
}

/// The exemption verdict for one manifest. `HardFail` wins over `OverCap`
/// because a non-exempable concern is a categorically different claim: the
/// budget is about how much absence is tolerable, the spine is about
/// which absence is never tolerable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExemptionVerdict {
    /// Below the allowance: nothing is reported.
    Within,
    /// Exactly at the allowance: reported, never blocking.
    Exhausted,
    /// More exemptions than the allowance: blocking.
    OverCap,
    /// At least one non-exempable concern was exempted: blocking whatever
    /// the counts say.
    HardFail,
}

/// One project's exemption budget, computed from its own manifest.
///
/// Every declared `not_applicable` entry consumes budget, including one on
/// an optional check: `required = false` must not be a one-character way
/// around the ceiling.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExemptionBudget {
    /// Required checks the manifest selects (profile defaults plus
    /// explicit declarations; changed-surface triggers do not count, so
    /// the budget cannot move with the diff being gated).
    pub required_checks: usize,
    /// `min(floor(0.25 x required_checks), 5)`.
    pub cap: usize,
    /// Declared `not_applicable` entries.
    pub declared: usize,
    /// Declared exemptions naming `build`, `tests`, `security`, or
    /// `repository`, sorted.
    #[serde(default)]
    pub non_exempable: Vec<String>,
}

impl ExemptionBudget {
    /// Remaining exemptions before the run fails.
    pub fn headroom(&self) -> usize {
        self.cap.saturating_sub(self.declared)
    }

    /// True when at least one non-exempable concern was exempted.
    pub fn hard_fails(&self) -> bool {
        !self.non_exempable.is_empty()
    }

    /// True when more exemptions are declared than the allowance.
    pub fn over_cap(&self) -> bool {
        self.declared > self.cap
    }

    /// Exactly at the allowance with headroom left at zero. A project
    /// with no exemptions is not "exhausted" merely because its cap is
    /// also zero, so `declared` must be non-zero.
    pub fn at_cap(&self) -> bool {
        self.declared > 0 && self.declared == self.cap
    }

    pub fn verdict(&self) -> ExemptionVerdict {
        if self.hard_fails() {
            ExemptionVerdict::HardFail
        } else if self.over_cap() {
            ExemptionVerdict::OverCap
        } else if self.at_cap() {
            ExemptionVerdict::Exhausted
        } else {
            ExemptionVerdict::Within
        }
    }

    /// One-line statement of the arithmetic, so a diagnostic shows how the
    /// allowance was reached rather than only its result.
    pub fn formula(&self) -> String {
        format!(
            "min(floor(0.25 x {} required checks), {}) = {}",
            self.required_checks, MAX_EXEMPTION_CAP, self.cap
        )
    }

    /// Diagnostic for the recorded budget result.
    pub fn diagnostic(&self) -> String {
        match self.verdict() {
            ExemptionVerdict::Within => format!(
                "{} of {} exemptions declared; {} headroom left ({})",
                self.declared,
                self.cap,
                self.headroom(),
                self.formula()
            ),
            ExemptionVerdict::Exhausted => format!(
                "exemption headroom is exhausted: {} of {} allowances used, {} left ({})",
                self.declared,
                self.cap,
                self.headroom(),
                self.formula()
            ),
            ExemptionVerdict::OverCap => format!(
                "{} exemptions declared but only {} are allowed ({} over; {} headroom left; {})",
                self.declared,
                self.cap,
                self.declared - self.cap,
                self.headroom(),
                self.formula()
            ),
            ExemptionVerdict::HardFail => format!(
                "{} may never be exempted: not_applicable names {} ({})",
                NON_EXEMPTABLE_CONCERNS.join(", "),
                self.non_exempable.join(", "),
                self.formula()
            ),
        }
    }
}

/// Compute one project's exemption budget from its own manifest.
pub(crate) fn exemption_budget(
    manifest: &GateManifest,
    selected: &BTreeMap<String, (bool, String)>,
) -> ExemptionBudget {
    let required_checks = selected.values().filter(|(required, _)| *required).count();
    let non_exempable: Vec<String> = manifest
        .not_applicable
        .keys()
        .filter(|id| is_non_exempable_concern(id))
        .cloned()
        .collect();
    ExemptionBudget {
        required_checks,
        cap: exemption_cap(required_checks),
        declared: manifest.not_applicable.len(),
        non_exempable,
    }
}

/// How a required, non-exempt check that binds no command is treated.
///
/// `Report` records the result as `UNBOUND`, which blocks the gate:
/// the default is fail-closed on coverage, because an unbound required
/// check is a never-attempted absence, not a proven exemption.
/// `Reject` refuses to resolve instead, which is a resolution-time
/// refusal rather than a verdict.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum BindingPolicy {
    #[default]
    Report,
    Reject,
}

impl BindingPolicy {
    /// True at the default, so the field is omitted from the canonical
    /// manifest and existing digests stay stable.
    pub fn is_default(&self) -> bool {
        *self == BindingPolicy::default()
    }

    /// Parse an accepted value. Returns `None` for anything else, which
    /// both manifest forms turn into an error listing [`BINDING_POLICIES`].
    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "report" => Some(BindingPolicy::Report),
            "reject" => Some(BindingPolicy::Reject),
            _ => None,
        }
    }
}

fn de_binding_policy<'de, D: serde::Deserializer<'de>>(d: D) -> Result<BindingPolicy, D::Error> {
    use serde::de::Error as _;
    let raw = String::deserialize(d)?;
    BindingPolicy::parse(&raw).ok_or_else(|| {
        D::Error::custom(format!(
            "unknown binding policy \"{raw}\"; expected {}",
            BINDING_POLICIES.join(" or ")
        ))
    })
}

/// Default per-file physical-line maximum for `source-file-size`.
pub const DEFAULT_MAX_LINES: u32 = 1000;
/// Upper bound accepted for `[source_size] max_lines`.
pub const MAX_MAX_LINES: u32 = 1_000_000;

/// Repository source-boundary policy for the built-in `source-file-size`
/// concern. The scanner counts raw newline bytes (`wc -l` semantics) in
/// candidate files and fails a required Gate when one exceeds
/// [`SourceSizePolicy::max_lines`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceSizePolicy {
    /// Positive maximum physical lines per included file.
    #[serde(default = "default_max_lines")]
    pub max_lines: u32,
    /// Repository-relative glob patterns. When non-empty, a file is a
    /// candidate only if it matches at least one pattern.
    #[serde(default)]
    pub include: Vec<String>,
    /// Repository-relative glob patterns. Always win over `include`
    /// and default discovery.
    #[serde(default)]
    pub exclude: Vec<String>,
}

impl Default for SourceSizePolicy {
    fn default() -> Self {
        Self {
            max_lines: DEFAULT_MAX_LINES,
            include: Vec::new(),
            exclude: Vec::new(),
        }
    }
}

impl SourceSizePolicy {
    /// True when the policy equals the default, so the field is omitted
    /// from the canonical manifest and existing digests stay stable.
    pub fn is_default(&self) -> bool {
        self == &Self::default()
    }
}

/// The `governance-quality` concern's code scope: which governance
/// finding codes this project reports on.
///
/// **Scope is not exemption.** A scope declares what the concern looks
/// at; an exemption removes a required check from the gate. The
/// effective set may only be a subset of
/// [`crate::gate::governance::GOVERNANCE_QUALITY_DECLARED_CODES`], and
/// an empty set while the concern is enabled is a configuration error
/// the adapter reports as `REVIEW_REQUIRED` — never as a pass. The
/// enforcement lives in the adapter, which sees the effective set and
/// the run's findings; this type only carries the declaration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GovernanceQualityPolicy {
    /// Finding codes this project reports on. Empty means the project
    /// declared an empty scope, which the adapter refuses; the default
    /// is the concern's full declared set.
    #[serde(default = "default_governance_codes")]
    pub codes: Vec<String>,
}

impl Default for GovernanceQualityPolicy {
    fn default() -> Self {
        Self {
            codes: default_governance_codes(),
        }
    }
}

impl GovernanceQualityPolicy {
    /// True when the policy equals the default, so the field is omitted
    /// from the canonical manifest and existing digests stay stable.
    pub fn is_default(&self) -> bool {
        self == &Self::default()
    }
}

fn default_governance_codes() -> Vec<String> {
    crate::gate::governance::GOVERNANCE_QUALITY_DECLARED_CODES
        .iter()
        .map(|c| (*c).to_string())
        .collect()
}

fn default_max_lines() -> u32 {
    DEFAULT_MAX_LINES
}

fn default_manifest_version() -> u32 {
    GATE_CONTRACT_VERSION
}

fn default_rule_pack_version() -> String {
    "local".to_string()
}

/// One explicit check declaration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManifestCheck {
    /// Gate concern id (e.g. `responsive`, `api-contract`).
    pub id: String,
    /// Whether the check blocks the gate. Defaults to true.
    #[serde(default = "default_true")]
    pub required: bool,
    /// Whether this declaration is active. `enabled = false` removes
    /// the concern from the resolved plan (used to opt out of a
    /// profile default).
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Optional tool command for diagnostics. When present and empty,
    /// resolution fails before execution.
    #[serde(default)]
    pub command: Option<String>,
    /// Optional source label. Defaults to `manifest`.
    #[serde(default)]
    pub source: Option<String>,
}

fn default_true() -> bool {
    true
}

/// Blocking policy override.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManifestBlocking {
    #[serde(default = "default_true")]
    pub review_required_blocks: bool,
}

impl Default for ManifestBlocking {
    fn default() -> Self {
        Self {
            review_required_blocks: true,
        }
    }
}

/// Generic context provider declaration. The `provider` is an opaque
/// name (`git`, `project-files`, `openspec`, ...); no provider-specific
/// fields live in the core schema.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContextDecl {
    pub provider: String,
}

/// Changed-surface trigger: when any observed surface contains
/// `when_changed` as a substring, `include` is added to the plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Trigger {
    pub when_changed: String,
    pub include: String,
}

/// Fully resolved, deterministic plan ready for dry-run rendering or
/// future execution. No child process has run at this point.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolvedGatePlan {
    pub plan: GatePlan,
    pub profile: String,
    pub execution_mode: String,
    pub project_commands: BTreeMap<String, String>,
    pub rule_pack_version: String,
    pub manifest_digest: String,
    pub contexts: Vec<String>,
    /// Concerns explicitly not scheduled (e.g. responsive on a
    /// backend profile). Kept explicit so absence is never mistaken
    /// for a silent skip.
    pub not_scheduled: Vec<String>,
    /// Resolved built-in source-size policy (defaults when the manifest
    /// declares no `[source_size]`). Rendered by `--dry-run` and passed
    /// to the in-process scanner.
    pub source_size: SourceSizePolicy,
    /// Resolved `governance-quality` code scope (defaults when the
    /// manifest declares no `[governance_quality]`). Rendered by
    /// `--dry-run` and passed to the code-scoped adapter, which is the
    /// only place a scope is enforced.
    pub governance_quality: GovernanceQualityPolicy,
    /// Scheduled required checks that bind no command and declare no
    /// exemption. Recorded at resolution time so `--dry-run` can name
    /// the coverage debt before anything executes.
    #[serde(default)]
    pub unbound: Vec<String>,
    /// Exemptions in force for this plan: check id to the bounded,
    /// redacted reason the manifest gave.
    #[serde(default)]
    pub exemptions: BTreeMap<String, String>,
    /// This project's exemption allowance, computed from this
    /// manifest's own required-check count. Recorded at resolution time
    /// so `--dry-run` can show the budget before anything executes and
    /// the runtime can turn the verdict into one result.
    #[serde(default)]
    pub exemption_budget: ExemptionBudget,
}

/// Manifest-level errors. Every variant carries an actionable message;
/// the binary appends the config hint via [`crate::error::Error`].
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ManifestError {
    #[error("unknown field \"{field}\"{suggestion}")]
    UnknownField { field: String, suggestion: String },
    #[error(
        "unsupported profile \"{profile}\"; expected a built-in (backend, frontend, full, minimal, product, rust-product, release, deployable) or a `[profiles.{profile}]` declaration"
    )]
    UnknownProfile { profile: String },
    #[error("unsupported manifest version {got}, expected {expected}")]
    UnknownVersion { got: u32, expected: u32 },
    #[error(
        "check \"{id}\" declares an empty command (remove `command` or set a non-empty value)"
    )]
    EmptyCheckCommand { id: String },
    #[error("project command \"{name}\" is empty or malformed (set a non-empty command string)")]
    InvalidProjectCommand { name: String },
    #[error("trigger for \"{include}\" has an empty `when_changed` pattern")]
    EmptyTrigger { include: String },
    #[error("invalid [source_size] policy: {detail}")]
    InvalidSourceSize { detail: String },
    #[error(
        "not_applicable[\"{id}\"] has an empty reason (state why the check does not apply to this project)"
    )]
    EmptyExemptionReason { id: String },
    #[error(
        "not_applicable[\"{id}\"] names a check the plan does not select (select the check, or drop the exemption)"
    )]
    ExemptionNotSelected { id: String },
    #[error(
        "not_applicable[\"{id}\"] exempts a check that also binds a command; bind the command and let it report, or drop the command"
    )]
    ExemptionConflictsWithCommand { id: String },
    #[error("unknown binding policy \"{got}\"; expected {expected}")]
    UnknownBindingPolicy { got: String, expected: String },
    #[error(
        "binding_policy = \"reject\" and no command is bound for required check(s): {checks} (bind commands.<id>, add a not_applicable exemption with a reason, or set binding_policy = \"report\")"
    )]
    UnboundRequiredCheck { checks: String },
    #[error("manifest parse error: {0}")]
    Parse(String),
}

const KNOWN_FIELDS: &[&str] = &[
    "version",
    "profile",
    "checks",
    "blocking",
    "project_commands",
    "rule_pack_version",
    "contexts",
    "triggers",
    "id",
    "required",
    "enabled",
    "command",
    "source",
    "review_required_blocks",
    "provider",
    "when_changed",
    "include",
    "profiles",
    "source_size",
    "max_lines",
    "exclude",
    "not_applicable",
    "binding_policy",
    "governance_quality",
    "codes",
];

pub(crate) fn edit_distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for (i, &ca) in a.iter().enumerate() {
        let mut cur = vec![i + 1];
        for (j, &cb) in b.iter().enumerate() {
            let cost = usize::from(ca != cb);
            cur.push((prev[j] + cost).min(cur[j] + 1).min(prev[j + 1] + 1));
        }
        prev = cur;
    }
    prev[b.len()]
}

fn suggest_field(unknown: &str) -> Option<&'static str> {
    let mut best: Option<(&'static str, usize)> = None;
    for &known in KNOWN_FIELDS {
        let d = edit_distance(unknown, known);
        if d <= 3 && best.map_or(true, |(_, bd)| d < bd) {
            best = Some((known, d));
        }
    }
    best.map(|(s, _)| s)
}

/// Nearest known field name for an unknown key, over an arbitrary
/// candidate set (used by the YAML converter's own schema). Returns
/// `None` when nothing is within edit distance 3.
pub(crate) fn suggest_from(unknown: &str, known: &[&str]) -> Option<String> {
    let mut best: Option<(&str, usize)> = None;
    for &k in known {
        let d = edit_distance(unknown, k);
        if d <= 3 && best.map_or(true, |(_, bd)| d < bd) {
            best = Some((k, d));
        }
    }
    best.map(|(s, _)| s.to_string())
}

/// Build an [`ManifestError::UnknownField`] with a `did you mean` hint
/// resolved against `known`.
pub(crate) fn unknown_field_error(field: &str, known: &[&str]) -> ManifestError {
    let suggestion = suggest_from(field, known)
        .map(|s| format!("; did you mean \"{s}\"?"))
        .unwrap_or_default();
    ManifestError::UnknownField {
        field: field.to_string(),
        suggestion,
    }
}

fn extract_unknown_field(msg: &str) -> Option<String> {
    for pat in ["unknown field `", "unknown field \"", "unknown field '"] {
        if let Some(start) = msg.find(pat) {
            let rest = &msg[start + pat.len()..];
            let end = rest.find(['`', '"', '\'']).unwrap_or(rest.len());
            let field: String = rest[..end].chars().take(64).collect();
            if !field.is_empty() {
                return Some(field);
            }
        }
    }
    None
}

fn map_toml_error(e: toml::de::Error) -> ManifestError {
    let msg = e.to_string();
    if msg.contains("unknown field") {
        if let Some(field) = extract_unknown_field(&msg) {
            let suggestion = suggest_field(&field)
                .map(|s| format!("; did you mean \"{s}\"?"))
                .unwrap_or_default();
            return ManifestError::UnknownField { field, suggestion };
        }
    }
    ManifestError::Parse(msg)
}

/// Short SHA-256 digest (`sha256:` + 16 hex chars) of the canonical
/// manifest text. Recorded in the resolved plan as rule-pack/manifest
/// identity.
pub fn manifest_digest(text: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
    let hex = format!("{:x}", hasher.finalize());
    format!("sha256:{}", &hex[..16.min(hex.len())])
}

/// Parse and structurally validate manifest text. Fails on unknown
/// fields, unknown profiles, version mismatch, empty enabled-check
/// commands, malformed project commands, malformed exemptions, and a
/// `binding_policy` refusal — always before any execution.
pub fn parse(text: &str) -> Result<GateManifest, ManifestError> {
    let mut manifest: GateManifest = toml::from_str(text).map_err(map_toml_error)?;
    bound_exemption_reasons(&mut manifest);
    validate(&manifest)?;
    Ok(manifest)
}

pub(crate) fn validate(manifest: &GateManifest) -> Result<(), ManifestError> {
    if manifest.version != GATE_CONTRACT_VERSION {
        return Err(ManifestError::UnknownVersion {
            got: manifest.version,
            expected: GATE_CONTRACT_VERSION,
        });
    }
    if profile_defaults_for(manifest, &manifest.profile).is_none() {
        return Err(ManifestError::UnknownProfile {
            profile: manifest.profile.clone(),
        });
    }
    for (name, concerns) in &manifest.profiles {
        if name.trim().is_empty() {
            return Err(ManifestError::Parse(
                "profile table with an empty name (use `[profiles.<name>]`)".to_string(),
            ));
        }
        if is_builtin_profile(name) {
            return Err(ManifestError::Parse(format!(
                "profile \"{name}\" shadows a built-in profile; choose a different name"
            )));
        }
        for concern in concerns {
            if concern.trim().is_empty() {
                return Err(ManifestError::Parse(format!(
                    "profile \"{name}\" declares an empty concern"
                )));
            }
        }
    }
    for check in &manifest.checks {
        if check.enabled {
            if let Some(cmd) = check.command.as_ref() {
                if cmd.trim().is_empty() {
                    return Err(ManifestError::EmptyCheckCommand {
                        id: check.id.clone(),
                    });
                }
            }
        }
        if check.id.trim().is_empty() {
            return Err(ManifestError::Parse(
                "check with an empty id (set `id = \"...\"`)".to_string(),
            ));
        }
    }
    for (name, cmd) in &manifest.project_commands {
        if name.trim().is_empty() || cmd.trim().is_empty() {
            return Err(ManifestError::InvalidProjectCommand { name: name.clone() });
        }
    }
    for trigger in &manifest.triggers {
        if trigger.when_changed.trim().is_empty() {
            return Err(ManifestError::EmptyTrigger {
                include: trigger.include.clone(),
            });
        }
        if trigger.include.trim().is_empty() {
            return Err(ManifestError::Parse(
                "trigger with an empty `include` concern".to_string(),
            ));
        }
    }
    for ctx in &manifest.contexts {
        if ctx.provider.trim().is_empty() {
            return Err(ManifestError::Parse(
                "context with an empty `provider` name".to_string(),
            ));
        }
    }
    validate_source_size(&manifest.source_size)?;
    validate_command_binding(manifest)?;
    Ok(())
}

/// Checks the manifest selects for execution, ignoring
/// changed-surface triggers: the profile defaults (built-in or
/// project-defined) with explicit `[[checks]]` applied as overrides.
/// [`resolve`] builds on this so the plan and the manifest-level
/// command-binding rule always agree on what is selected.
fn selected_without_triggers(manifest: &GateManifest) -> BTreeMap<String, (bool, String)> {
    let defaults = profile_defaults_for(manifest, &manifest.profile).unwrap_or_default();
    let mut selected: BTreeMap<String, (bool, String)> = BTreeMap::new();
    for id in defaults.iter() {
        selected.insert(id.clone(), (true, "manifest".to_string()));
    }
    for check in &manifest.checks {
        if !check.enabled {
            selected.remove(&check.id);
            continue;
        }
        let source = check
            .source
            .clone()
            .unwrap_or_else(|| "manifest".to_string());
        // Explicit declarations override profile defaults (including
        // relaxing a default to `required = false`).
        selected.insert(check.id.clone(), (check.required, source));
    }
    selected
}

/// True when `id` binds a project command in the manifest.
fn binds_command(manifest: &GateManifest, id: &str) -> bool {
    manifest
        .checks
        .iter()
        .any(|c| c.id == id && c.enabled && c.command.is_some())
}

/// Required, non-exempt, non-in-process checks with no bound command,
/// sorted. This is the set `binding_policy = "reject"` refuses on and
/// the set the runtime records as `UNBOUND`.
///
/// The in-process test reuses [`is_source_file_size_concern`], the same
/// predicate `execute_plan` uses to skip the project-command branch: a
/// check the runtime executes itself needs no command binding.
pub(crate) fn unbound_required_checks(
    manifest: &GateManifest,
    selected: &BTreeMap<String, (bool, String)>,
) -> Vec<String> {
    let mut unbound: Vec<String> = selected
        .iter()
        .filter(|(id, (required, _))| {
            *required
                && !is_source_file_size_concern(id)
                && !manifest.not_applicable.contains_key(*id)
                && !binds_command(manifest, id)
        })
        .map(|(id, _)| (*id).to_string())
        .collect();
    unbound.sort();
    unbound.dedup();
    unbound
}

/// Normalize every exemption reason in place: redact credential-shaped
/// tokens, then bound the length. Applied by both manifest forms so a
/// reason can never reach a digest, a render, or the database unbounded.
pub(crate) fn bound_exemption_reasons(manifest: &mut GateManifest) {
    for reason in manifest.not_applicable.values_mut() {
        *reason = bound_text(reason, MAX_EXEMPTION_REASON_BYTES);
    }
}

/// Command-binding rules: exemptions must name a selected check with a
/// reason and no command, and `binding_policy = "reject"` refuses a
/// manifest with a required unbound check. The empty-command case is
/// checked before this (it is a different defect with its own error),
/// and `reject` is a resolution refusal, not a verdict.
fn validate_command_binding(manifest: &GateManifest) -> Result<(), ManifestError> {
    let selected = selected_without_triggers(manifest);
    for (id, reason) in &manifest.not_applicable {
        if reason.trim().is_empty() {
            return Err(ManifestError::EmptyExemptionReason { id: id.clone() });
        }
        if !selected.contains_key(id) {
            return Err(ManifestError::ExemptionNotSelected { id: id.clone() });
        }
        if binds_command(manifest, id) {
            return Err(ManifestError::ExemptionConflictsWithCommand { id: id.clone() });
        }
    }
    if manifest.binding_policy == BindingPolicy::Reject {
        let unbound = unbound_required_checks(manifest, &selected);
        if !unbound.is_empty() {
            return Err(ManifestError::UnboundRequiredCheck {
                checks: unbound.join(", "),
            });
        }
    }
    Ok(())
}

/// Validate the `[source_size]` policy before any execution: a positive
/// maximum within range, and repository-relative glob patterns that are
/// non-empty and free of absolute or `..` traversal components.
fn validate_source_size(policy: &SourceSizePolicy) -> Result<(), ManifestError> {
    if policy.max_lines == 0 {
        return Err(ManifestError::InvalidSourceSize {
            detail: "max_lines must be a positive integer".to_string(),
        });
    }
    if policy.max_lines > MAX_MAX_LINES {
        return Err(ManifestError::InvalidSourceSize {
            detail: format!("max_lines must be <= {MAX_MAX_LINES}"),
        });
    }
    for (label, patterns) in [("include", &policy.include), ("exclude", &policy.exclude)] {
        for pattern in patterns {
            validate_source_pattern(label, pattern)?;
        }
    }
    Ok(())
}

fn validate_source_pattern(label: &str, pattern: &str) -> Result<(), ManifestError> {
    let trimmed = pattern.trim();
    if trimmed.is_empty() {
        return Err(ManifestError::InvalidSourceSize {
            detail: format!("{label} contains an empty pattern"),
        });
    }
    if trimmed.starts_with('/') || trimmed.starts_with('\\') {
        return Err(ManifestError::InvalidSourceSize {
            detail: format!("{label} pattern \"{trimmed}\" must be repository-relative"),
        });
    }
    // A Windows drive prefix (`C:`) is an absolute reference in disguise.
    let bytes: Vec<char> = trimmed.chars().collect();
    if bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == ':' {
        return Err(ManifestError::InvalidSourceSize {
            detail: format!("{label} pattern \"{trimmed}\" must be repository-relative"),
        });
    }
    if trimmed
        .split(['/', '\\'])
        .any(|component| component == "..")
    {
        return Err(ManifestError::InvalidSourceSize {
            detail: format!("{label} pattern \"{trimmed}\" must not traverse with `..`"),
        });
    }
    Ok(())
}

/// Resolve a manifest into a deterministic plan. `changed_surfaces`
/// are observed paths (e.g. `frontend/App.tsx`); matching triggers
/// include their concerns. No child process runs and no state is
/// mutated.
pub fn resolve(
    manifest: &GateManifest,
    changed_surfaces: &[String],
) -> Result<ResolvedGatePlan, ManifestError> {
    validate(manifest)?;
    let mut selected = selected_without_triggers(manifest);
    for check in &manifest.checks {
        // Enabled check with an empty command already failed in
        // `validate`; re-check here so direct struct construction
        // cannot bypass the fail-before-execution rule.
        if let Some(cmd) = check.command.as_ref() {
            if cmd.trim().is_empty() {
                return Err(ManifestError::EmptyCheckCommand {
                    id: check.id.clone(),
                });
            }
        }
    }
    for trigger in &manifest.triggers {
        let hit = changed_surfaces
            .iter()
            .any(|s| s.contains(trigger.when_changed.as_str()));
        if hit && !selected.contains_key(&trigger.include) {
            selected.insert(trigger.include.clone(), (true, "trigger".to_string()));
        }
    }

    let mut checks: Vec<PlannedCheck> = selected
        .iter()
        .map(|(gate_id, (required, source))| PlannedCheck {
            gate_id: gate_id.clone(),
            source: source.clone(),
            required: *required,
            exempt: manifest.not_applicable.contains_key(gate_id),
        })
        .collect();
    // BTreeMap iteration is already sorted; keep an explicit sort so
    // the determinism guarantee does not depend on map internals.
    checks.sort_by(|a, b| a.gate_id.cmp(&b.gate_id));

    // Command-free required checks are coverage debt, resolved here so
    // the plan can be rendered before anything executes. Triggers
    // contribute to the plan but not to this set: a manifest-level
    // command-binding rule must not depend on observed change surfaces.
    let unbound = unbound_required_checks(manifest, &selected);
    let exemptions: BTreeMap<String, String> = manifest
        .not_applicable
        .iter()
        .map(|(id, reason)| (id.clone(), bound_text(reason, MAX_EXEMPTION_REASON_BYTES)))
        .collect();
    // The exemption allowance is a manifest-level rule for the same
    // reason: it is derived from the manifest's own selection, never
    // from the diff being gated.
    let exemption_budget = exemption_budget(manifest, &selected_without_triggers(manifest));

    // Explicitly record well-known concerns that are *not* scheduled
    // so backend-only plans are self-explanatory.
    let scheduled: BTreeSet<&str> = checks.iter().map(|c| c.gate_id.as_str()).collect();
    let mut not_scheduled: Vec<String> = ["responsive", "browser", "a11y"]
        .into_iter()
        .chain(PRODUCT_PROFILE_CONCERNS.iter().copied())
        .chain(RELEASE_GATE_CONCERNS.iter().copied())
        .filter(|id| !scheduled.contains(id))
        .map(|s| s.to_string())
        .collect();
    not_scheduled.sort();
    not_scheduled.dedup();

    let canonical = toml::to_string_pretty(manifest)
        .unwrap_or_else(|_| format!("profile={}", manifest.profile));
    let plan = GatePlan {
        version: GATE_CONTRACT_VERSION,
        checks,
        policy: BlockingPolicy {
            review_required_blocks: manifest.blocking.review_required_blocks,
        },
    };
    Ok(ResolvedGatePlan {
        plan,
        profile: manifest.profile.clone(),
        execution_mode: "dry-run".to_string(),
        project_commands: manifest.project_commands.clone(),
        rule_pack_version: manifest.rule_pack_version.clone(),
        manifest_digest: manifest_digest(&canonical),
        contexts: manifest
            .contexts
            .iter()
            .map(|c| c.provider.clone())
            .collect(),
        not_scheduled,
        source_size: manifest.source_size.clone(),
        governance_quality: manifest.governance_quality.clone(),
        unbound,
        exemptions,
        exemption_budget,
    })
}

/// Render the resolved plan as human-readable text. Pure: no child
/// process is spawned and no state is mutated.
pub fn render_plan(resolved: &ResolvedGatePlan) -> String {
    let mut out = String::new();
    out.push_str("gate plan (dry-run; nothing was executed)\n");
    out.push_str(&format!(
        "contract version: {} | profile: {} | rule-pack: {} | manifest: {}\n",
        resolved.plan.version,
        resolved.profile,
        resolved.rule_pack_version,
        resolved.manifest_digest
    ));
    out.push_str(&format!(
        "blocking: review_required_blocks={}\n",
        resolved.plan.policy.review_required_blocks
    ));
    out.push_str(&format!("execution mode: {}\n", resolved.execution_mode));
    out.push_str(&format!(
        "source_size: max_lines={} include=[{}] exclude=[{}]\n",
        resolved.source_size.max_lines,
        resolved.source_size.include.join(", "),
        resolved.source_size.exclude.join(", ")
    ));
    out.push_str(&format!(
        "governance_quality: codes=[{}]\n",
        resolved.governance_quality.codes.join(", ")
    ));
    out.push_str("checks:\n");
    if resolved.plan.checks.is_empty() {
        out.push_str("  (none)\n");
    } else {
        for c in &resolved.plan.checks {
            out.push_str(&format!(
                "  - {} [{}] via {}\n",
                c.gate_id,
                if c.required { "required" } else { "optional" },
                c.source
            ));
        }
    }
    if !resolved.exemptions.is_empty() {
        out.push_str("exempted checks (declared not_applicable; do not block):\n");
        for (id, reason) in &resolved.exemptions {
            out.push_str(&format!("  - {id}: {reason}\n"));
        }
    }
    // Always name the allowance, including for a project that has used
    // none of it: the budget is what an operator edits the manifest
    // against, and a zero-allowance small manifest is exactly the case
    // that needs to be visible before anything executes.
    out.push_str(&format!(
        "exemption budget: {} of {} allowances used ({} headroom left; {})\n",
        resolved.exemption_budget.declared,
        resolved.exemption_budget.cap,
        resolved.exemption_budget.headroom(),
        resolved.exemption_budget.formula()
    ));
    if resolved.exemption_budget.verdict() != ExemptionVerdict::Within {
        out.push_str(&format!(
            "  verdict: {}\n",
            resolved.exemption_budget.diagnostic()
        ));
    }
    if !resolved.exemption_budget.non_exempable.is_empty() {
        out.push_str(&format!(
            "  never exempable: {}\n",
            NON_EXEMPTABLE_CONCERNS.join(", ")
        ));
    }
    if !resolved.unbound.is_empty() {
        out.push_str("unbound required checks (no command bound; will fail as UNBOUND):\n");
        for id in &resolved.unbound {
            out.push_str(&format!(
                "  - {id}: bind `command` for this check, or add a not_applicable exemption with a reason\n"
            ));
        }
    }
    out.push_str("project commands:\n");
    if resolved.project_commands.is_empty() {
        out.push_str("  (none)\n");
    } else {
        for (name, cmd) in &resolved.project_commands {
            out.push_str(&format!("  - {name}: {cmd}\n"));
        }
    }
    out.push_str("context providers:\n");
    if resolved.contexts.is_empty() {
        out.push_str("  (none)\n");
    } else {
        for ctx in &resolved.contexts {
            out.push_str(&format!("  - {ctx}\n"));
        }
    }
    if !resolved.not_scheduled.is_empty() {
        out.push_str("explicitly not scheduled:\n");
        for id in &resolved.not_scheduled {
            out.push_str(&format!("  - {id}\n"));
        }
    }
    out
}

/// Resolve the manifest path for a project root: `<root>/gate.toml`
/// first, then `<root>/.driftwatch/gate.toml`, then
/// `<root>/.ai-gate/gate.yaml` (the business-project convention).
/// Returns `None` when none exists (no gate configured — not an error).
pub fn manifest_path(project_root: &Path) -> Option<PathBuf> {
    [
        project_root.join("gate.toml"),
        project_root.join(".driftwatch").join("gate.toml"),
        project_root.join(".ai-gate").join("gate.yaml"),
    ]
    .into_iter()
    .find(|p| p.is_file())
}

/// Outcome of a runtime-filtered manifest load. Distinguishes "no
/// manifest" from "a manifest that targets a different Gate runtime" so
/// the command can report the latter without executing or persisting it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoadOutcome {
    NoManifest,
    /// The manifest names a runtime other than driftwatchdog; nothing
    /// was executed and no gate run was persisted.
    ForeignRuntime {
        runtime: String,
    },
    /// Boxed because the full manifest is much larger than the other
    /// variants and is moved exactly once per gate run.
    Manifest(Box<GateManifest>),
}

/// Read and parse the manifest at `path`, dispatching on extension:
/// `.yaml`/`.yml` go through the business Gate YAML converter, anything
/// else is parsed as native `gate.toml`.
fn read_manifest(path: &Path) -> Result<GateManifest, ManifestError> {
    let text = fs::read_to_string(path)
        .map_err(|e| ManifestError::Parse(format!("cannot read {}: {e}", path.display())))?;
    match path.extension().and_then(|e| e.to_str()) {
        Some("yaml") | Some("yml") => crate::gate::aigate::parse(&text),
        _ => parse(&text),
    }
}

/// Load the manifest for a project root. Returns `Ok(None)` when no
/// manifest exists **or** when the only manifest targets a foreign
/// runtime (callers that must distinguish the two use
/// [`load_for_runtime`]). Malformed manifests are errors with actionable
/// diagnostics.
pub fn load(project_root: &Path) -> Result<Option<GateManifest>, ManifestError> {
    match load_for_runtime(project_root)? {
        LoadOutcome::Manifest(m) => Ok(Some(*m)),
        LoadOutcome::NoManifest | LoadOutcome::ForeignRuntime { .. } => Ok(None),
    }
}

/// Load the manifest honoring the optional `runtime` field of a
/// `.ai-gate/gate.yaml`. Native `gate.toml` manifests have no runtime
/// field and are always driftwatchdog's.
pub fn load_for_runtime(project_root: &Path) -> Result<LoadOutcome, ManifestError> {
    let Some(path) = manifest_path(project_root) else {
        return Ok(LoadOutcome::NoManifest);
    };
    if path.extension().and_then(|e| e.to_str()) == Some("yaml")
        || path.extension().and_then(|e| e.to_str()) == Some("yml")
    {
        let text = fs::read_to_string(&path)
            .map_err(|e| ManifestError::Parse(format!("cannot read {}: {e}", path.display())))?;
        let doc = crate::gate::aigate::parse_document(&text)?;
        if let Some(rt) = &doc.runtime {
            if rt != crate::gate::aigate::RUNTIME_NAME {
                return Ok(LoadOutcome::ForeignRuntime {
                    runtime: rt.clone(),
                });
            }
        }
        return Ok(LoadOutcome::Manifest(Box::new(doc.manifest)));
    }
    read_manifest(&path).map(|m| LoadOutcome::Manifest(Box::new(m)))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A `minimal` manifest with `required` required checks, each bound to
    /// a command so nothing is unbound, and the named ones exempted.
    fn checks_manifest(required: usize, exempt: &[(&str, bool)]) -> String {
        let mut out = String::from("version = 1\nprofile = \"minimal\"\n");
        for i in 0..required {
            out.push_str(&format!(
                "\n[[checks]]\nid = \"c{i}\"\ncommand = \"true\"\n"
            ));
        }
        for (id, optional) in exempt {
            out.push_str(&format!("\n[[checks]]\nid = \"{id}\"\n"));
            if *optional {
                out.push_str("required = false\n");
            }
        }
        if !exempt.is_empty() {
            out.push_str("\n[not_applicable]\n");
            for (id, _) in exempt {
                out.push_str(&format!(
                    "{id} = \"no implementation of {id} on this stack\"\n"
                ));
            }
        }
        out
    }

    const VALID: &str = r#"
version = 1
profile = "backend"

[blocking]
review_required_blocks = true

[project_commands]
test = "cargo test"
"#;

    /// Task 2.4/2.5: a required check with no `commands.<id>` entry
    /// resolves under the default `report` policy, is listed as
    /// unbound, and binds no exemption.
    #[test]
    fn required_check_without_command_resolves_as_unbound() {
        let m = parse(
            r#"
version = 1
profile = "minimal"

[[checks]]
id = "ux-review"
"#,
        )
        .unwrap();
        assert_eq!(m.binding_policy, BindingPolicy::Report);
        assert!(m.not_applicable.is_empty());
        let r = resolve(&m, &[]).unwrap();
        assert_eq!(r.unbound, vec!["ux-review".to_string()]);
        assert!(r.exemptions.is_empty());
        let check = r
            .plan
            .checks
            .iter()
            .find(|c| c.gate_id == "ux-review")
            .unwrap();
        assert!(check.required);
        assert!(!check.exempt);
    }

    /// A profile default counts: it is a required, selected check with
    /// no command, and it must appear as unbound (this is the shape of
    /// the 41 product-profile repos in the workspace).
    ///
    /// `governance-quality` is in that set on purpose. The governance
    /// checker lives outside this repository, so the profile cannot
    /// default a command for it: a project that picks a product profile
    /// without binding one records `UNBOUND` and fails, which is the
    /// honest result rather than a silent skip.
    #[test]
    fn product_profile_defaults_are_unbound_when_no_command_is_bound() {
        let m = parse("version = 1\nprofile = \"product\"\n").unwrap();
        let r = resolve(&m, &[]).unwrap();
        // `source-file-size` runs in-process, so it needs no command.
        assert_eq!(
            r.unbound,
            vec![
                "governance-quality".to_string(),
                "placeholder-threshold".to_string(),
                "product-code-boundary".to_string()
            ]
        );
        assert!(!r.unbound.iter().any(|id| id == "source-file-size"));
    }

    /// A bound command removes the check from the unbound set.
    #[test]
    fn bound_command_is_not_unbound() {
        let m = parse(
            r#"
version = 1
profile = "minimal"

[[checks]]
id = "ux-review"
command = "npm run review"
"#,
        )
        .unwrap();
        let r = resolve(&m, &[]).unwrap();
        assert!(r.unbound.is_empty());
        assert!(!r.exemptions.contains_key("ux-review"));
    }

    /// A written exemption clears the check from the unbound set and
    /// marks the plan entry, while the check stays scheduled.
    #[test]
    fn exemption_clears_unbound_and_marks_the_plan() {
        let m = parse(
            r#"
version = 1
profile = "minimal"

[[checks]]
id = "ux-review"

[not_applicable]
ux-review = "no UI surface in this project"
"#,
        )
        .unwrap();
        let r = resolve(&m, &[]).unwrap();
        assert!(r.unbound.is_empty());
        assert_eq!(
            r.exemptions.get("ux-review").map(String::as_str),
            Some("no UI surface in this project")
        );
        let check = r
            .plan
            .checks
            .iter()
            .find(|c| c.gate_id == "ux-review")
            .unwrap();
        assert!(check.required, "an exemption does not deselect the check");
        assert!(check.exempt);
    }

    /// A reason is length-bounded and credential-redacted at parse
    /// time, so it can never reach a digest, a render, or the database
    /// raw.
    #[test]
    fn exemption_reason_is_bounded_and_redacted() {
        let long = "x".repeat(MAX_EXEMPTION_REASON_BYTES * 2);
        let m = parse(&format!(
            "version = 1\nprofile = \"minimal\"\n\n[[checks]]\nid = \"ux-review\"\n\
             \n[not_applicable]\nux-review = \"api_key=supersecret123 {long}\"\n"
        ))
        .unwrap();
        let reason = m.not_applicable.get("ux-review").unwrap();
        assert!(
            !reason.contains("supersecret123"),
            "credential-shaped token must be redacted: {reason}"
        );
        // `bound_text` truncates to the limit and appends the `…`
        // marker, the same convention as diagnostics and remediation.
        assert!(
            reason.len() <= MAX_EXEMPTION_REASON_BYTES + 3,
            "reason must be bounded, got {} bytes",
            reason.len()
        );
    }

    #[test]
    fn empty_exemption_reason_is_rejected_naming_the_key() {
        let err = parse(
            r#"
version = 1
profile = "minimal"

[[checks]]
id = "ux-review"

[not_applicable]
ux-review = "   "
"#,
        )
        .unwrap_err();
        assert!(matches!(err, ManifestError::EmptyExemptionReason { .. }));
        assert!(err.to_string().contains("ux-review"));
    }

    #[test]
    fn exemption_for_an_unselected_check_is_rejected() {
        let err = parse(
            r#"
version = 1
profile = "minimal"

[not_applicable]
a11y = "no UI here"
"#,
        )
        .unwrap_err();
        assert!(matches!(err, ManifestError::ExemptionNotSelected { .. }));
        assert!(err.to_string().contains("a11y"));
    }

    #[test]
    fn exemption_that_also_binds_a_command_is_rejected() {
        let err = parse(
            r#"
version = 1
profile = "minimal"

[[checks]]
id = "ux-review"
command = "npm run review"

[not_applicable]
ux-review = "we have a command after all"
"#,
        )
        .unwrap_err();
        assert!(matches!(
            err,
            ManifestError::ExemptionConflictsWithCommand { .. }
        ));
        assert!(err.to_string().contains("ux-review"));
    }

    /// Task 2.5: an unknown `binding_policy` value fails with the
    /// offending value and the accepted set.
    #[test]
    fn unknown_binding_policy_is_rejected_with_accepted_set() {
        let err =
            parse("version = 1\nprofile = \"minimal\"\nbinding_policy = \"MAYBE\"\n").unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("MAYBE"), "{msg}");
        assert!(msg.contains("report"), "{msg}");
        assert!(msg.contains("reject"), "{msg}");
    }

    // ----- Exemption budget (owner decision, 2026-10-02) -----

    /// The cap formula at the numbers the owner named, plus the two
    /// fleet-relevant ends. `required / 4` is exactly
    /// `floor(0.25 x required)`; the `min(..., 5)` binds from 20 up.
    #[test]
    fn exemption_cap_matches_the_owner_formula_at_the_boundaries() {
        // 10 required -> floor(2.5) = 2
        assert_eq!(exemption_cap(10), 2);
        // 20 required -> exactly the absolute cap
        assert_eq!(exemption_cap(20), 5);
        // 24 required -> 6 by percentage, 5 by the absolute cap: the
        // absolute cap is what binds.
        assert_eq!(exemption_cap(24), 5);
        // 40 required -> 10 by percentage, 5 by the absolute cap: a
        // larger manifest buys no wider hole.
        assert_eq!(exemption_cap(40), 5);
        // Small-manifest end: a project under four required checks gets
        // no allowance at all.
        assert_eq!(exemption_cap(3), 0);
        assert_eq!(exemption_cap(0), 0);
        assert_eq!(exemption_cap(4), 1);
        assert_eq!(exemption_cap(19), 4);
    }

    #[test]
    fn budget_counts_the_manifests_own_required_checks() {
        let m = parse(&checks_manifest(8, &[])).unwrap();
        let r = resolve(&m, &[]).unwrap();
        assert_eq!(r.exemption_budget.required_checks, 8);
        assert_eq!(r.exemption_budget.cap, 2);
        assert_eq!(r.exemption_budget.declared, 0);
        assert_eq!(r.exemption_budget.verdict(), ExemptionVerdict::Within);
        assert!(r.exemption_budget.diagnostic().contains("2 headroom left"));
    }

    /// An optional check still spends budget, so `required = false` is
    /// not a one-character way around the ceiling.
    #[test]
    fn budget_counts_optional_exemptions_too() {
        let m = parse(&checks_manifest(8, &[("opt-one", true), ("opt-two", true)])).unwrap();
        let r = resolve(&m, &[]).unwrap();
        // The denominator is the *required* count (8), so the two
        // optional checks do not widen the allowance...
        assert_eq!(r.exemption_budget.required_checks, 8);
        // ...but they do spend it, which puts this manifest at its cap.
        assert_eq!(r.exemption_budget.cap, 2);
        assert_eq!(r.exemption_budget.declared, 2);
        assert_eq!(r.exemption_budget.verdict(), ExemptionVerdict::Exhausted);
    }

    /// Every canonical concern is protected, and so is every workspace
    /// alias of one. The alias table is the matcher's only input, so an
    /// alias missing from it would be silently exempable — which is the
    /// defect this table exists to prevent.
    #[test]
    fn non_exempable_concerns_and_their_aliases_are_never_exemptable() {
        for id in NON_EXEMPTABLE_CONCERNS {
            assert!(is_non_exempable_concern(id), "{id} must be in the spine");
        }
        // The repository-integrity concern is spelled two ways in this
        // workspace; both are protected.
        assert!(is_non_exempable_concern("repository"));
        assert!(is_non_exempable_concern("repository_integrity"));
        // A genuine outsider is not swept in by the aliasing.
        assert!(!is_non_exempable_concern("accessibility"));
        assert!(!is_non_exempable_concern("repository_hygiene"));
        assert!(!is_non_exempable_concern("build-cache"));
    }

    /// The alias table and the canonical list must not drift apart: every
    /// canonical concern needs an entry, and every entry names a real
    /// concern. Adding a spelling is therefore a deliberate edit here
    /// rather than an accidental divergence.
    #[test]
    fn the_alias_table_covers_exactly_the_non_exempable_concerns() {
        for concern in NON_EXEMPTABLE_CONCERNS {
            let entry = NON_EXEMPTABLE_ALIASES
                .iter()
                .find(|(name, _)| name == concern)
                .unwrap_or_else(|| panic!("{concern} has no alias entry"));
            assert!(
                entry.1.contains(concern),
                "{concern} must be protected under its own spelling"
            );
        }
        for (concern, _) in NON_EXEMPTABLE_ALIASES {
            assert!(
                NON_EXEMPTABLE_CONCERNS.contains(concern),
                "{concern} is aliased but is not a non-exempable concern"
            );
        }
        // The measured workspace spellings, recorded so a future rename
        // is a visible decision: `repository` in 63 manifests,
        // `repository_integrity` in 2.
        assert_eq!(NON_EXEMPTABLE_ALIASES.len(), 5);
        assert_eq!(
            NON_EXEMPTABLE_ALIASES
                .iter()
                .flat_map(|(_, spellings)| spellings.iter().copied())
                .count(),
            6
        );
    }

    /// The spine fails the budget whatever the counts say: one
    /// exemption of a spine check inside the allowance is still a hard
    /// failure.
    #[test]
    fn exempting_a_spine_check_hard_fails_within_the_cap() {
        let m = parse(&checks_manifest(8, &[("tests", false)])).unwrap();
        let r = resolve(&m, &[]).unwrap();
        assert_eq!(r.exemption_budget.cap, 2);
        assert_eq!(r.exemption_budget.declared, 1);
        assert!(!r.exemption_budget.over_cap());
        assert_eq!(r.exemption_budget.non_exempable, vec!["tests".to_string()]);
        assert_eq!(r.exemption_budget.verdict(), ExemptionVerdict::HardFail);
        assert!(r
            .exemption_budget
            .diagnostic()
            .contains("may never be exempted"));
        // The plan an operator reads before executing names the spine.
        assert!(render_plan(&r).contains("never exempable: build, tests, security, repository"));
    }

    /// The same hard failure for the *other* spelling of the
    /// repository-integrity concern. A manifest using
    /// `repository_integrity` must not be able to exempt the concern the
    /// spine protects just because it spells the id differently.
    #[test]
    fn exempting_the_repository_integrity_alias_hard_fails() {
        let m = parse(&checks_manifest(8, &[("repository_integrity", false)])).unwrap();
        let r = resolve(&m, &[]).unwrap();
        assert_eq!(r.exemption_budget.cap, 2);
        assert_eq!(r.exemption_budget.declared, 1);
        assert!(
            !r.exemption_budget.over_cap(),
            "the alias is not a cap case"
        );
        assert_eq!(
            r.exemption_budget.non_exempable,
            vec!["repository_integrity".to_string()],
            "the alias must be recorded as the offending id, not dropped"
        );
        assert_eq!(r.exemption_budget.verdict(), ExemptionVerdict::HardFail);
    }

    /// The over-cap verdict wins on a large manifest, and the diagnostic
    /// shows the arithmetic rather than only the outcome.
    #[test]
    fn over_cap_reports_the_arithmetic() {
        let m = parse(&checks_manifest(40, &[])).unwrap();
        let mut m = m;
        for id in ["a", "b", "c", "d", "e", "f"] {
            m.not_applicable
                .insert(id.to_string(), format!("{id} has no home here"));
            m.checks.push(ManifestCheck {
                id: id.to_string(),
                required: true,
                enabled: true,
                command: None,
                source: None,
            });
        }
        let r = resolve(&m, &[]).unwrap();
        assert_eq!(r.exemption_budget.cap, 5);
        assert_eq!(r.exemption_budget.declared, 6);
        assert_eq!(r.exemption_budget.verdict(), ExemptionVerdict::OverCap);
        let diag = r.exemption_budget.diagnostic();
        assert!(diag.contains("6 exemptions declared but only 5"), "{diag}");
        assert!(diag.contains("1 over"), "{diag}");
        assert!(diag.contains("0 headroom left"), "{diag}");
        assert!(
            diag.contains("min(floor(0.25 x 46 required checks), 5)"),
            "{diag}"
        );
    }

    /// A changed-surface trigger must not move the ceiling: the budget
    /// is a property of the manifest, not of the diff.
    #[test]
    fn triggers_do_not_move_the_exemption_cap() {
        let text = format!(
            "{}\n[[triggers]]\nwhen_changed = \"frontend/\"\ninclude = \"responsive\"\n",
            checks_manifest(8, &[])
        );
        let m = parse(&text).unwrap();
        let with_trigger = resolve(
            &m,
            &[
                "frontend/app.tsx".to_string(),
                "frontend/main.tsx".to_string(),
            ],
        )
        .unwrap();
        let without = resolve(&m, &[]).unwrap();
        assert_eq!(
            with_trigger.exemption_budget.required_checks,
            without.exemption_budget.required_checks
        );
        assert!(with_trigger
            .plan
            .checks
            .iter()
            .any(|c| c.gate_id == "responsive"));
    }

    /// A small manifest is a zero allowance, and one exemption is over
    /// it: the stated consequence of the formula, not an accident.
    #[test]
    fn a_small_manifest_gets_a_zero_allowance() {
        let m = parse(
            "version = 1\nprofile = \"minimal\"\n\n[[checks]]\nid = \"ux-review\"\n\n\
             [not_applicable]\nux-review = \"no UI surface in this project\"\n",
        )
        .unwrap();
        let r = resolve(&m, &[]).unwrap();
        assert_eq!(r.exemption_budget.cap, 0);
        assert_eq!(r.exemption_budget.verdict(), ExemptionVerdict::OverCap);
    }

    /// The plan render names the budget even for a project that has used
    /// none of it, because the allowance is what a manifest is edited
    /// against.
    #[test]
    fn plan_render_names_the_exemption_budget() {
        let m = parse(&checks_manifest(24, &[])).unwrap();
        let r = resolve(&m, &[]).unwrap();
        let out = render_plan(&r);
        assert!(
            out.contains("exemption budget: 0 of 5 allowances used"),
            "{out}"
        );
        assert!(
            out.contains("min(floor(0.25 x 24 required checks), 5)"),
            "{out}"
        );
        // Nothing to report: an unused budget is not a finding.
        assert!(!out.contains("verdict:"), "{out}");
    }

    #[test]
    fn binding_policy_parses_both_accepted_values() {
        for (raw, want) in [
            ("report", BindingPolicy::Report),
            ("reject", BindingPolicy::Reject),
        ] {
            let m = parse(&format!(
                "version = 1\nprofile = \"minimal\"\nbinding_policy = \"{raw}\"\n"
            ))
            .unwrap();
            assert_eq!(m.binding_policy, want);
        }
    }

    /// Task 2.6: `reject` refuses to resolve, naming every offending
    /// check in one refusal, and `resolve` is what the gate command
    /// calls before opening the database — so nothing runs and no run
    /// is persisted.
    #[test]
    fn reject_policy_refuses_to_resolve_naming_every_unbound_check() {
        const UNBOUND: &str = r#"
version = 1
profile = "minimal"
binding_policy = "reject"

[[checks]]
id = "ux-review"

[[checks]]
id = "api-contract"

[[checks]]
id = "smoke"
command = "true"
"#;
        // Both entry points refuse: `parse` (used by `load`) validates
        // before returning a manifest, and `resolve` re-validates so a
        // directly constructed struct cannot slip past.
        let err = parse(UNBOUND).unwrap_err();
        let msg = err.to_string();
        assert!(matches!(err, ManifestError::UnboundRequiredCheck { .. }));
        assert!(msg.contains("api-contract"), "{msg}");
        assert!(msg.contains("ux-review"), "{msg}");
        assert!(
            !msg.contains("smoke"),
            "bound checks are not offenders: {msg}"
        );

        let report = UNBOUND.replace("binding_policy = \"reject\"", "");
        let mut m = parse(&report).unwrap();
        m.binding_policy = BindingPolicy::Reject;
        let err = resolve(&m, &[]).unwrap_err();
        assert!(matches!(err, ManifestError::UnboundRequiredCheck { .. }));
    }

    #[test]
    fn reject_policy_resolves_when_every_required_check_is_bound_or_exempt() {
        let m = parse(
            r#"
version = 1
profile = "minimal"
binding_policy = "reject"

[[checks]]
id = "smoke"
command = "true"

[[checks]]
id = "ux-review"

[not_applicable]
ux-review = "no UI surface in this project"
"#,
        )
        .unwrap();
        let r = resolve(&m, &[]).unwrap();
        assert!(r.unbound.is_empty());
    }

    /// The default (`report`) policy must not refuse: an unbound
    /// required check is recorded, not fatal at resolution time.
    #[test]
    fn report_policy_never_refuses_resolution() {
        let m = parse("version = 1\nprofile = \"product\"\n").unwrap();
        assert!(resolve(&m, &[]).is_ok());
    }

    /// The absent-command case must not merge with the existing
    /// empty-command case: `command = ""` keeps its own error.
    #[test]
    fn empty_command_and_absent_command_stay_distinct() {
        let err = parse(
            r#"
version = 1
profile = "minimal"

[[checks]]
id = "ux-review"
command = ""
"#,
        )
        .unwrap_err();
        assert!(matches!(err, ManifestError::EmptyCheckCommand { .. }));
    }

    /// Digests must not move for manifests that do not use the new
    /// fields: both new fields are skipped when empty/default, so every
    /// pre-existing `gate.toml` keeps the digest it had.
    #[test]
    fn new_fields_do_not_change_manifest_digests() {
        let plain = resolve(&parse(VALID).unwrap(), &[]).unwrap();
        let explicit = parse(&VALID.replace(
            "profile = \"backend\"",
            "profile = \"backend\"\nbinding_policy = \"report\"",
        ))
        .unwrap();
        assert_eq!(
            resolve(&explicit, &[]).unwrap().manifest_digest,
            plain.manifest_digest,
            "the default policy must not change the digest"
        );
    }

    /// The dry-run plan render names unbound checks and their
    /// remediation, separately from the exempted list.
    #[test]
    fn plan_render_names_unbound_and_exempted_separately() {
        let m = parse(
            r#"
version = 1
profile = "minimal"

[[checks]]
id = "ux-review"

[[checks]]
id = "a11y"

[not_applicable]
a11y = "no UI surface in this project"
"#,
        )
        .unwrap();
        let text = render_plan(&resolve(&m, &[]).unwrap());
        assert!(text.contains("unbound required checks"), "{text}");
        assert!(text.contains("ux-review"), "{text}");
        assert!(text.contains("exempted checks"), "{text}");
        assert!(text.contains("no UI surface in this project"), "{text}");
    }

    #[test]
    fn valid_manifest_resolves_without_execution() {
        let m = parse(VALID).unwrap();
        let plan = resolve(&m, &[]).unwrap();
        assert_eq!(plan.profile, "backend");
        assert_eq!(plan.plan.version, GATE_CONTRACT_VERSION);
        // Backend profile schedules no browser check; absence is explicit.
        assert!(!plan.plan.checks.iter().any(|c| c.gate_id == "responsive"));
        assert!(plan.not_scheduled.contains(&"responsive".to_string()));
        assert_eq!(
            plan.project_commands.get("test").map(String::as_str),
            Some("cargo test")
        );
        assert_eq!(plan.rule_pack_version, "local");
        assert!(plan.manifest_digest.starts_with("sha256:"));
    }

    #[test]
    fn unknown_field_fails_with_name_and_hint() {
        let err = parse("version = 1\nprofile = \"backend\"\nprofiel = \"x\"\n").unwrap_err();
        match err {
            ManifestError::UnknownField { field, suggestion } => {
                assert_eq!(field, "profiel");
                assert!(suggestion.contains("profile"), "got: {suggestion}");
            }
            other => panic!("unexpected: {other}"),
        }
    }

    #[test]
    fn unknown_profile_fails() {
        let err = parse("version = 1\nprofile = \"quantum\"\n").unwrap_err();
        assert!(matches!(err, ManifestError::UnknownProfile { .. }));
    }

    #[test]
    fn version_mismatch_fails() {
        let err = parse("version = 999\nprofile = \"backend\"\n").unwrap_err();
        assert!(matches!(err, ManifestError::UnknownVersion { .. }));
    }

    #[test]
    fn empty_enabled_check_command_fails_before_execution() {
        let text = "version = 1\nprofile = \"minimal\"\n[[checks]]\nid = \"api-contract\"\ncommand = \"\"\n";
        let err = parse(text).unwrap_err();
        assert!(matches!(err, ManifestError::EmptyCheckCommand { .. }));
    }

    #[test]
    fn empty_project_command_fails() {
        let text = "version = 1\nprofile = \"minimal\"\n[project_commands]\ntest = \"\"\n";
        let err = parse(text).unwrap_err();
        assert!(matches!(err, ManifestError::InvalidProjectCommand { .. }));
    }

    #[test]
    fn backend_profile_schedules_no_browser_check() {
        let m = parse(VALID).unwrap();
        let plan = resolve(&m, &[]).unwrap();
        assert!(!plan.plan.checks.iter().any(|c| c.gate_id == "browser"));
    }

    #[test]
    fn ui_trigger_includes_responsive_concern() {
        let text = format!(
            "{VALID}\n[[triggers]]\nwhen_changed = \"frontend/\"\ninclude = \"responsive\"\n"
        );
        let m = parse(&text).unwrap();
        let without: Vec<String> = vec![];
        assert!(!resolve(&m, &without)
            .unwrap()
            .plan
            .checks
            .iter()
            .any(|c| c.gate_id == "responsive"));
        let with = vec!["frontend/App.tsx".to_string()];
        let plan = resolve(&m, &with).unwrap();
        assert!(plan.plan.checks.iter().any(|c| c.gate_id == "responsive"));
    }

    #[test]
    fn explicit_check_can_disable_profile_default() {
        let text = format!("{VALID}\n[[checks]]\nid = \"secret-scan\"\nenabled = false\n");
        let m = parse(&text).unwrap();
        let plan = resolve(&m, &[]).unwrap();
        assert!(!plan.plan.checks.iter().any(|c| c.gate_id == "secret-scan"));
    }

    #[test]
    fn resolved_plans_are_deterministic() {
        let m = parse(VALID).unwrap();
        let a = resolve(&m, &[]).unwrap();
        let b = resolve(&m, &[]).unwrap();
        assert_eq!(a, b);
        let ids: Vec<&str> = a.plan.checks.iter().map(|c| c.gate_id.as_str()).collect();
        let mut sorted = ids.clone();
        sorted.sort();
        assert_eq!(ids, sorted);
    }

    #[test]
    fn dry_run_render_contains_plan_identity() {
        let m = parse(VALID).unwrap();
        let plan = resolve(&m, &[]).unwrap();
        let text = render_plan(&plan);
        assert!(text.contains("dry-run"));
        assert!(text.contains("backend"));
        assert!(text.contains("api-contract"));
        assert!(text.contains("cargo test"));
        assert!(text.contains("local"));
        assert!(text.contains("nothing was executed"));
    }

    #[test]
    fn project_and_tool_commands_stay_separate() {
        let text =
            format!("{VALID}\n[[checks]]\nid = \"api-contract\"\ncommand = \"gate-tool check\"\n");
        let m = parse(&text).unwrap();
        let plan = resolve(&m, &[]).unwrap();
        // Project commands map is independent of per-check tool commands.
        assert!(plan.project_commands.contains_key("test"));
        assert!(plan.plan.checks.iter().any(|c| c.gate_id == "api-contract"));
    }

    #[test]
    fn context_providers_are_generic_names() {
        let text = format!("{VALID}\n[[contexts]]\nprovider = \"openspec\"\n");
        let m = parse(&text).unwrap();
        let plan = resolve(&m, &[]).unwrap();
        assert_eq!(plan.contexts, vec!["openspec".to_string()]);
        assert!(render_plan(&plan).contains("openspec"));
    }

    #[test]
    fn missing_manifest_is_not_an_error() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(load(tmp.path()).unwrap().is_none());
        assert!(manifest_path(tmp.path()).is_none());
    }

    #[test]
    fn manifest_path_prefers_project_root() {
        let tmp = tempfile::tempdir().unwrap();
        let primary = tmp.path().join("gate.toml");
        let scoped_dir = tmp.path().join(".driftwatch");
        std::fs::create_dir_all(&scoped_dir).unwrap();
        let scoped = scoped_dir.join("gate.toml");
        std::fs::write(&primary, VALID).unwrap();
        std::fs::write(&scoped, VALID).unwrap();
        assert_eq!(manifest_path(tmp.path()), Some(primary));
    }

    #[test]
    fn product_profile_schedules_all_product_quality_concerns() {
        let text = r#"
version = 1
profile = "product"
"#;
        let m = parse(text).unwrap();
        let plan = resolve(&m, &[]).unwrap();
        let ids: Vec<&str> = plan
            .plan
            .checks
            .iter()
            .map(|c| c.gate_id.as_str())
            .collect();
        assert_eq!(
            ids,
            vec![
                "governance-quality",
                "placeholder-threshold",
                "product-code-boundary",
                "source-file-size"
            ],
            "product profile selects all product-profile concerns in sorted order"
        );
        // All are required by default: the profile does not silently
        // relax the threshold.
        for c in &plan.plan.checks {
            assert!(
                c.required,
                "product profile default is required: {}",
                c.gate_id
            );
        }
        // Frontend concerns stay explicit "not scheduled" for the
        // product profile.
        assert!(plan.not_scheduled.contains(&"responsive".to_string()));
        assert!(plan.not_scheduled.contains(&"a11y".to_string()));
    }

    #[test]
    fn rust_product_profile_schedules_same_concerns_as_product() {
        let product = parse(
            r#"version = 1
profile = "product"
"#,
        )
        .unwrap();
        let rust_product = parse(
            r#"version = 1
profile = "rust-product"
"#,
        )
        .unwrap();
        let product_plan = resolve(&product, &[]).unwrap();
        let rust_product_plan = resolve(&rust_product, &[]).unwrap();
        let product_ids: Vec<&str> = product_plan
            .plan
            .checks
            .iter()
            .map(|c| c.gate_id.as_str())
            .collect();
        let rust_ids: Vec<&str> = rust_product_plan
            .plan
            .checks
            .iter()
            .map(|c| c.gate_id.as_str())
            .collect();
        assert_eq!(product_ids, rust_ids);
    }

    #[test]
    fn product_profile_check_can_be_relaxed_to_optional() {
        let text = r#"
version = 1
profile = "product"
[[checks]]
id = "product-code-boundary"
required = false
"#;
        let m = parse(text).unwrap();
        let plan = resolve(&m, &[]).unwrap();
        let boundary = plan
            .plan
            .checks
            .iter()
            .find(|c| c.gate_id == "product-code-boundary")
            .unwrap();
        assert!(!boundary.required, "explicit `required = false` wins");
        let threshold = plan
            .plan
            .checks
            .iter()
            .find(|c| c.gate_id == "placeholder-threshold")
            .unwrap();
        assert!(
            threshold.required,
            "untouched concern keeps the profile default"
        );
    }

    #[test]
    fn product_profile_can_be_disabled_explicitly() {
        let text = r#"
version = 1
profile = "product"
[[checks]]
id = "placeholder-threshold"
enabled = false
"#;
        let m = parse(text).unwrap();
        let plan = resolve(&m, &[]).unwrap();
        assert!(!plan
            .plan
            .checks
            .iter()
            .any(|c| c.gate_id == "placeholder-threshold"));
        assert!(plan
            .plan
            .checks
            .iter()
            .any(|c| c.gate_id == "product-code-boundary"));
    }

    #[test]
    fn unknown_profile_still_rejects_typos() {
        // The new profiles do not weaken the unknown-profile error.
        let err = parse("version = 1\nprofile = \"prodcut\"\n").unwrap_err();
        assert!(matches!(err, ManifestError::UnknownProfile { .. }));
    }

    #[test]
    fn supported_profiles_lists_product_quality_entries() {
        for name in ["product", "rust-product"] {
            assert!(
                SUPPORTED_PROFILES.contains(&name),
                "SUPPORTED_PROFILES must list `{name}`"
            );
            assert!(
                profile_defaults(name).is_some(),
                "profile_defaults must resolve `{name}`"
            );
            assert!(
                is_builtin_profile(name),
                "is_builtin_profile must accept `{name}`"
            );
        }
    }

    #[test]
    fn release_profile_schedules_both_release_gate_concerns() {
        let text = r#"
version = 1
profile = "release"
"#;
        let m = parse(text).unwrap();
        let plan = resolve(&m, &[]).unwrap();
        let ids: Vec<&str> = plan
            .plan
            .checks
            .iter()
            .map(|c| c.gate_id.as_str())
            .collect();
        assert_eq!(
            ids,
            vec!["capability-conformance", "release-evidence"],
            "release profile selects both release-gate concerns in sorted order"
        );
        // Both are required by default: the profile does not silently
        // relax the gate.
        for c in &plan.plan.checks {
            assert!(
                c.required,
                "release profile default is required: {}",
                c.gate_id
            );
        }
        // Frontend + product-quality concerns stay explicit "not
        // scheduled" for the release profile.
        assert!(plan.not_scheduled.contains(&"responsive".to_string()));
        assert!(plan
            .not_scheduled
            .contains(&"product-code-boundary".to_string()));
        assert!(plan
            .not_scheduled
            .contains(&"placeholder-threshold".to_string()));
    }

    #[test]
    fn release_profile_check_can_be_relaxed_to_optional() {
        let text = r#"
version = 1
profile = "release"
[[checks]]
id = "release-evidence"
required = false
"#;
        let m = parse(text).unwrap();
        let plan = resolve(&m, &[]).unwrap();
        let evidence = plan
            .plan
            .checks
            .iter()
            .find(|c| c.gate_id == "release-evidence")
            .unwrap();
        assert!(!evidence.required, "explicit `required = false` wins");
        let capability = plan
            .plan
            .checks
            .iter()
            .find(|c| c.gate_id == "capability-conformance")
            .unwrap();
        assert!(
            capability.required,
            "untouched concern keeps the profile default"
        );
    }

    #[test]
    fn release_profile_can_be_disabled_explicitly() {
        let text = r#"
version = 1
profile = "release"
[[checks]]
id = "capability-conformance"
enabled = false
"#;
        let m = parse(text).unwrap();
        let plan = resolve(&m, &[]).unwrap();
        assert!(!plan
            .plan
            .checks
            .iter()
            .any(|c| c.gate_id == "capability-conformance"));
        assert!(plan
            .plan
            .checks
            .iter()
            .any(|c| c.gate_id == "release-evidence"));
    }

    #[test]
    fn unsupported_plan_records_release_gate_concerns_as_not_scheduled() {
        // A `backend` plan must still mention the release-gate
        // concerns as "not scheduled" so a reader knows they exist
        // and were consciously excluded.
        let m = parse(VALID).unwrap();
        let plan = resolve(&m, &[]).unwrap();
        assert!(plan
            .not_scheduled
            .contains(&"capability-conformance".to_string()));
        assert!(plan.not_scheduled.contains(&"release-evidence".to_string()));
    }

    #[test]
    fn supported_profiles_lists_release_entry() {
        let name = "release";
        assert!(
            SUPPORTED_PROFILES.contains(&name),
            "SUPPORTED_PROFILES must list `{name}`"
        );
        assert!(
            profile_defaults(name).is_some(),
            "profile_defaults must resolve `{name}`"
        );
        assert!(
            is_builtin_profile(name),
            "is_builtin_profile must accept `{name}`"
        );
    }

    #[test]
    fn source_size_defaults_to_1000_lines_and_is_rendered() {
        let m = parse("version = 1\nprofile = \"product\"\n").unwrap();
        assert_eq!(m.source_size, SourceSizePolicy::default());
        assert_eq!(m.source_size.max_lines, DEFAULT_MAX_LINES);
        let plan = resolve(&m, &[]).unwrap();
        assert_eq!(plan.source_size, SourceSizePolicy::default());
        let text = render_plan(&plan);
        assert!(text.contains("source_size: max_lines=1000"), "got: {text}");
        assert!(text.contains("source-file-size"));
    }

    #[test]
    fn source_size_policy_parses_and_changes_the_digest() {
        let base = parse("version = 1\nprofile = \"product\"\n").unwrap();
        let base_digest = resolve(&base, &[]).unwrap().manifest_digest;
        let text = r#"
version = 1
profile = "product"

[source_size]
max_lines = 250
include = ["src/**"]
exclude = ["src/generated/**"]
"#;
        let m = parse(text).unwrap();
        assert_eq!(m.source_size.max_lines, 250);
        assert_eq!(m.source_size.include, vec!["src/**".to_string()]);
        let plan = resolve(&m, &[]).unwrap();
        assert_eq!(plan.source_size.max_lines, 250);
        assert_ne!(
            base_digest, plan.manifest_digest,
            "a policy change must change the manifest digest"
        );
    }

    #[test]
    fn source_size_rejects_zero_and_out_of_range_max_lines() {
        for body in [
            "version = 1\nprofile = \"product\"\n[source_size]\nmax_lines = 0\n",
            "version = 1\nprofile = \"product\"\n[source_size]\nmax_lines = 1000001\n",
        ] {
            let err = parse(body).unwrap_err();
            assert!(
                matches!(err, ManifestError::InvalidSourceSize { .. }),
                "expected InvalidSourceSize, got {err}"
            );
        }
    }

    #[test]
    fn source_size_rejects_absolute_traversal_and_empty_patterns() {
        for pattern in ["/etc/passwd", "../secrets", "", "C:/src"] {
            let body = format!(
                "version = 1\nprofile = \"product\"\n[source_size]\ninclude = [\"{pattern}\"]\n"
            );
            let err = parse(&body).unwrap_err();
            assert!(
                matches!(err, ManifestError::InvalidSourceSize { .. }),
                "pattern {pattern:?} should be rejected, got {err}"
            );
        }
    }

    #[test]
    fn source_size_unknown_field_is_rejected_with_hint() {
        let err = parse("version = 1\nprofile = \"product\"\n[source_size]\nmax_line = 10\n")
            .unwrap_err();
        match err {
            ManifestError::UnknownField { field, suggestion } => {
                assert_eq!(field, "max_line");
                assert!(suggestion.contains("max_lines"), "got: {suggestion}");
            }
            other => panic!("unexpected: {other}"),
        }
    }

    #[test]
    fn source_file_size_can_be_relaxed_or_disabled() {
        let relaxed = parse(
            "version = 1\nprofile = \"product\"\n[[checks]]\nid = \"source-file-size\"\nrequired = false\n",
        )
        .unwrap();
        let plan = resolve(&relaxed, &[]).unwrap();
        let check = plan
            .plan
            .checks
            .iter()
            .find(|c| c.gate_id == "source-file-size")
            .unwrap();
        assert!(!check.required);

        let disabled = parse(
            "version = 1\nprofile = \"product\"\n[[checks]]\nid = \"source-file-size\"\nenabled = false\n",
        )
        .unwrap();
        let plan = resolve(&disabled, &[]).unwrap();
        assert!(!plan
            .plan
            .checks
            .iter()
            .any(|c| c.gate_id == "source-file-size"));
        assert!(plan.not_scheduled.contains(&"source-file-size".to_string()));
    }
}
