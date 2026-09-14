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
        _ => None,
    }
}

/// Supported profile names, used in diagnostics.
pub const SUPPORTED_PROFILES: &[&str] = &["backend", "frontend", "full", "minimal"];

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
}

/// Manifest-level errors. Every variant carries an actionable message;
/// the binary appends the config hint via [`crate::error::Error`].
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ManifestError {
    #[error("unknown field \"{field}\"{suggestion}")]
    UnknownField { field: String, suggestion: String },
    #[error(
        "unsupported profile \"{profile}\"; expected one of: backend, frontend, full, minimal"
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
];

fn edit_distance(a: &str, b: &str) -> usize {
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

fn validate(manifest: &GateManifest) -> Result<(), ManifestError> {
    if manifest.version != GATE_CONTRACT_VERSION {
        return Err(ManifestError::UnknownVersion {
            got: manifest.version,
            expected: GATE_CONTRACT_VERSION,
        });
    }
    if profile_defaults(&manifest.profile).is_none() {
        return Err(ManifestError::UnknownProfile {
            profile: manifest.profile.clone(),
        });
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
    let defaults = profile_defaults(&manifest.profile).unwrap_or(&[]);

    let mut selected: BTreeMap<String, (bool, String)> = BTreeMap::new();
    for id in defaults.iter() {
        selected.insert((*id).to_string(), (true, "manifest".to_string()));
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
/// first, then `<root>/.driftwatch/gate.toml`. Returns `None` when
/// neither exists (no gate configured — not an error).
pub fn manifest_path(project_root: &Path) -> Option<PathBuf> {
    let primary = project_root.join("gate.toml");
    if primary.is_file() {
        return Some(primary);
    }
    let state_scoped = project_root.join(".driftwatch").join("gate.toml");
    if state_scoped.is_file() {
        return Some(state_scoped);
    }
    None
}

/// Load the manifest for a project root. Returns `Ok(None)` when no
/// manifest file exists. Malformed manifests are errors with
/// actionable diagnostics.
pub fn load(project_root: &Path) -> Result<Option<GateManifest>, ManifestError> {
    let Some(path) = manifest_path(project_root) else {
        return Ok(None);
    };
    let text = fs::read_to_string(&path)
        .map_err(|e| ManifestError::Parse(format!("cannot read {}: {e}", path.display())))?;
    parse(&text).map(Some)
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
}
