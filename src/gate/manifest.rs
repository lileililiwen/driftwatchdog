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
//!   `commands.<id>` lands on `REVIEW_REQUIRED` via the existing
//!   required + missing-command path, except for `source-file-size`,
//!   which runs the in-process scanner and needs no command at all.
//! * `release` — the two release-gate concern ids
//!   (`capability-conformance`, `release-evidence`). The profile is
//!   data only; a project that picks `release` without binding
//!   `commands.<id>` lands on `REVIEW_REQUIRED` via the existing
//!   required + missing-command path. Driftwatchdog does not become
//!   a release publisher, signer, SBOM generator, or deployment
//!   executor.
//!
//! Projects may enable extra checks or relax a profile default to
//! `required = false`. Tightening an optional default to required is
//! allowed; arbitrary override fields are rejected.
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
//! unknown profile, or a malformed project command fails plan
//! resolution **before any check executes**. [`render_plan`] is pure:
//! it prints the resolved plan and never spawns a child process or
//! mutates state.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::gate::aggregate::{BlockingPolicy, GatePlan, PlannedCheck};
use crate::gate::concerns::{DEPLOYABLE_CONCERNS, PRODUCT_QUALITY_CONCERNS, RELEASE_GATE_CONCERNS};
use crate::gate::dto::GATE_CONTRACT_VERSION;

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
        // concern ids. Selection is data only and does not invent a
        // command; a project that picks `product` (or `rust-product`)
        // without binding `commands.<id>` lands on REVIEW_REQUIRED
        // via the existing required + missing-command path. The
        // `source-file-size` concern is the exception: it is executed
        // by the in-process scanner and needs no command binding.
        "product" | "rust-product" => Some(PRODUCT_QUALITY_CONCERNS),
        // Release-gate profile selects the two stable release-gate
        // concern ids. The profile is data only and never invents a
        // command; a project that picks `release` without binding
        // `commands.<id>` lands on REVIEW_REQUIRED via the existing
        // required + missing-command path. Driftwatchdog does not
        // become a release publisher, signer, SBOM generator, or
        // deployment executor.
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
/// commands, and malformed project commands — always before any
/// execution.
pub fn parse(text: &str) -> Result<GateManifest, ManifestError> {
    let manifest: GateManifest = toml::from_str(text).map_err(map_toml_error)?;
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
        let source = check
            .source
            .clone()
            .unwrap_or_else(|| "manifest".to_string());
        // Explicit declarations override profile defaults (including
        // relaxing a default to `required = false`).
        selected.insert(check.id.clone(), (check.required, source));
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
        .into_iter()
        .map(|(gate_id, (required, source))| PlannedCheck {
            gate_id,
            source,
            required,
        })
        .collect();
    // BTreeMap iteration is already sorted; keep an explicit sort so
    // the determinism guarantee does not depend on map internals.
    checks.sort_by(|a, b| a.gate_id.cmp(&b.gate_id));

    // Explicitly record well-known concerns that are *not* scheduled
    // so backend-only plans are self-explanatory.
    let scheduled: BTreeSet<&str> = checks.iter().map(|c| c.gate_id.as_str()).collect();
    let mut not_scheduled: Vec<String> = ["responsive", "browser", "a11y"]
        .into_iter()
        .chain(PRODUCT_QUALITY_CONCERNS.iter().copied())
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

    const VALID: &str = r#"
version = 1
profile = "backend"

[blocking]
review_required_blocks = true

[project_commands]
test = "cargo test"
"#;

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
                "placeholder-threshold",
                "product-code-boundary",
                "source-file-size"
            ],
            "product profile selects all product-quality concerns in sorted order"
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
