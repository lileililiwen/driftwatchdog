//! Business-project Gate policy: `.ai-gate/gate.yaml` → `GateManifest`.
//!
//! Bootstrapped business repositories declare Gate policy in
//! `.ai-gate/gate.yaml` and name `driftwatchdog` as the shared runtime.
//! This module parses that YAML form into the same [`GateManifest`] the
//! native `gate.toml` produces, so every downstream step
//! (`resolve`/`render_plan`/execute/aggregate/persist) is reused
//! unchanged. No shared Gate logic is copied into the business project
//! and no new execution path is introduced.
//!
//! ## Declared schema
//!
//! ```yaml
//! version: 1
//! runtime: driftwatchdog          # optional; other values are not ours
//! profile: browser-extension      # built-in or project-defined
//! rule_pack: browser-extension@0.1.0   # optional identity
//! checks:                         # concern id -> true | false | "optional"
//!   build: true
//!   accessibility: optional
//! commands:                       # optional concern id -> command string
//! timeouts:                       # optional concern id -> deadline in ms
//!   build: 600000
//! governance_quality:              # optional; narrows the concern's code scope
//!   codes: [QUALITY_README_CLAIM_CONTRADICTED]
//!   build: "npm run build"
//! blocking: [FAIL, REVIEW_REQUIRED]  # optional; REVIEW_REQUIRED toggles fail-closed
//! not_applicable:                # optional written exemptions: id -> reason
//!   accessibility: "no UI surface in this project"
//! binding_policy: report         # optional; report | reject
//! project_commands:               # optional named project commands
//!   test: "npm test"
//! contexts: [git, openspec]       # optional context providers
//! notes:                          # optional operator annotations; never policy
//!   - "why this value is what it is"
//! ```
//!
//! `notes` is prose for the next reader and nothing else: it selects no
//! check, binds no command, and cannot change a result. It exists so an
//! operator's reasoning has a home in the file that records their decisions.
//! Recording "this check is off until the quality block lands" as an
//! exemption (`not_applicable`) would change the plan; deleting the line
//! loses the reason. A note is the third thing, and it is inert by
//! construction.
//!
//! Unknown fields fail with a `did you mean` hint. A `profile` that is
//! not a built-in name becomes a project-defined profile whose selection
//! is exactly the `true`/`"optional"` entries in `checks`.
//!
//! `not_applicable`, `binding_policy` and `timeouts` convert to the same
//! [`GateManifest`] fields the native `gate.toml` uses, so the command
//! binding and timeout rules run once, in `manifest::validate`, for both
//! forms.

use std::collections::BTreeMap;

use yaml_rust2::{Yaml, YamlLoader};

use crate::gate::dto::GATE_CONTRACT_VERSION;
use crate::gate::manifest::{
    self, BindingPolicy, ContextDecl, GateManifest, GovernanceQualityPolicy, ManifestBlocking,
    ManifestCheck, ManifestError, SourceSizePolicy, BINDING_POLICIES,
};

/// The runtime name this binary answers to in a `.ai-gate/gate.yaml`.
pub const RUNTIME_NAME: &str = "driftwatchdog";

/// Top-level keys accepted in a business Gate YAML manifest.
const YAML_FIELDS: &[&str] = &[
    "version",
    "runtime",
    "profile",
    "rule_pack",
    "checks",
    "commands",
    "timeouts",
    "blocking",
    "not_applicable",
    "binding_policy",
    "project_commands",
    "contexts",
    "governance_quality",
    "notes",
];

/// A parsed business Gate document: the declared runtime (for the
/// runtime guard) plus the converted native manifest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiGateDoc {
    pub runtime: Option<String>,
    pub manifest: GateManifest,
}

/// Parse and convert a business Gate YAML document, ignoring the
/// `runtime` field (assumes the caller already verified it targets this
/// runtime, or wants the manifest regardless).
pub fn parse(text: &str) -> Result<GateManifest, ManifestError> {
    Ok(parse_document(text)?.manifest)
}

/// Parse a business Gate YAML document into its runtime and converted
/// manifest. Structural validation runs before any return, so a
/// malformed policy never reaches execution.
pub fn parse_document(text: &str) -> Result<AiGateDoc, ManifestError> {
    let docs = YamlLoader::load_from_str(text)
        .map_err(|e| ManifestError::Parse(format!("yaml parse: {e}")))?;
    let doc = docs
        .first()
        .ok_or_else(|| ManifestError::Parse("empty gate.yaml document".to_string()))?;
    let hash = doc
        .as_hash()
        .ok_or_else(|| ManifestError::Parse("gate.yaml top level must be a mapping".to_string()))?;

    let mut version: u32 = GATE_CONTRACT_VERSION;
    let mut runtime: Option<String> = None;
    let mut profile: Option<String> = None;
    let mut rule_pack: Option<String> = None;
    let mut checks_yaml: Option<&Yaml> = None;
    let mut commands_yaml: Option<&Yaml> = None;
    let mut timeouts_yaml: Option<&Yaml> = None;
    let mut blocking_yaml: Option<&Yaml> = None;
    let mut not_applicable_yaml: Option<&Yaml> = None;
    let mut binding_policy_yaml: Option<&Yaml> = None;
    let mut project_commands_yaml: Option<&Yaml> = None;
    let mut contexts_yaml: Option<&Yaml> = None;
    let mut governance_quality_yaml: Option<&Yaml> = None;
    let mut notes_yaml: Option<&Yaml> = None;

    for (k, v) in hash {
        let key = k
            .as_str()
            .ok_or_else(|| ManifestError::Parse("non-string key in gate.yaml".to_string()))?;
        match key {
            "version" => {
                version = v
                    .as_i64()
                    .and_then(|i| u32::try_from(i).ok())
                    .ok_or_else(|| {
                        ManifestError::Parse("`version` must be an integer".to_string())
                    })?;
            }
            "runtime" => runtime = Some(expect_string(v, "runtime")?),
            "profile" => profile = Some(expect_string(v, "profile")?),
            "rule_pack" => rule_pack = Some(expect_string(v, "rule_pack")?),
            "checks" => checks_yaml = Some(v),
            "commands" => commands_yaml = Some(v),
            "timeouts" => timeouts_yaml = Some(v),
            "blocking" => blocking_yaml = Some(v),
            "not_applicable" => not_applicable_yaml = Some(v),
            "binding_policy" => binding_policy_yaml = Some(v),
            "project_commands" => project_commands_yaml = Some(v),
            "contexts" => contexts_yaml = Some(v),
            "governance_quality" => governance_quality_yaml = Some(v),
            "notes" => notes_yaml = Some(v),
            other => return Err(manifest::unknown_field_error(other, YAML_FIELDS)),
        }
    }

    if version != GATE_CONTRACT_VERSION {
        return Err(ManifestError::UnknownVersion {
            got: version,
            expected: GATE_CONTRACT_VERSION,
        });
    }
    let profile = profile
        .ok_or_else(|| ManifestError::Parse("gate.yaml requires a `profile`".to_string()))?;
    if profile.trim().is_empty() {
        return Err(ManifestError::Parse(
            "gate.yaml `profile` is empty".to_string(),
        ));
    }

    // checks: concern id -> selection.
    let mut selected: Vec<(String, bool)> = Vec::new();
    let mut deselected: Vec<String> = Vec::new();
    if let Some(c) = checks_yaml {
        let h = c
            .as_hash()
            .ok_or_else(|| ManifestError::Parse("`checks` must be a mapping".to_string()))?;
        for (k, v) in h {
            let id = k
                .as_str()
                .ok_or_else(|| ManifestError::Parse("check id must be a string".to_string()))?
                .to_string();
            if id.trim().is_empty() {
                return Err(ManifestError::Parse("check id is empty".to_string()));
            }
            match selection_of(v)? {
                Selection::Required => selected.push((id, true)),
                Selection::Optional => selected.push((id, false)),
                Selection::Disabled => deselected.push(id),
            }
        }
    }

    // commands: concern id -> command string; must reference a selection.
    let mut commands: BTreeMap<String, String> = BTreeMap::new();
    if let Some(c) = commands_yaml {
        let h = c
            .as_hash()
            .ok_or_else(|| ManifestError::Parse("`commands` must be a mapping".to_string()))?;
        for (k, v) in h {
            let id = k
                .as_str()
                .ok_or_else(|| ManifestError::Parse("command key must be a string".to_string()))?
                .to_string();
            commands.insert(id, expect_string(v, "command value")?);
        }
    }
    for id in commands.keys() {
        if !selected.iter().any(|(s, _)| s == id) {
            return Err(ManifestError::Parse(format!(
                "commands.{id} references a check that is not selected"
            )));
        }
    }

    // timeouts: concern id -> deadline in milliseconds. Converted
    // straight onto `ManifestCheck.timeout_ms`; the value rules (zero,
    // above cap) belong to `manifest::validate` so both manifest forms
    // run them once. A non-integer here is a shape error, so it is
    // caught in this parser, but the *value* is never judged here.
    let mut timeouts: BTreeMap<String, u64> = BTreeMap::new();
    if let Some(t) = timeouts_yaml {
        let h = t
            .as_hash()
            .ok_or_else(|| ManifestError::Parse("`timeouts` must be a mapping".to_string()))?;
        for (k, v) in h {
            let id = k
                .as_str()
                .ok_or_else(|| ManifestError::Parse("timeouts key must be a string".to_string()))?
                .to_string();
            let ms = v
                .as_i64()
                .and_then(|i| u64::try_from(i).ok())
                .ok_or_else(|| {
                    ManifestError::Parse(format!(
                        "timeouts.{id} must be a positive integer number of milliseconds"
                    ))
                })?;
            timeouts.insert(id, ms);
        }
    }
    for id in timeouts.keys() {
        if !selected.iter().any(|(s, _)| s == id) {
            return Err(ManifestError::Parse(format!(
                "timeouts.{id} references a check that is not selected"
            )));
        }
    }

    // blocking: status list -> review-required policy.
    let mut blocking = ManifestBlocking::default();
    if let Some(b) = blocking_yaml {
        let arr = b
            .as_vec()
            .ok_or_else(|| ManifestError::Parse("`blocking` must be a list".to_string()))?;
        let mut names = Vec::with_capacity(arr.len());
        for item in arr {
            names.push(expect_string(item, "blocking entry")?);
        }
        for n in &names {
            if n != "FAIL" && n != "REVIEW_REQUIRED" {
                return Err(ManifestError::Parse(format!(
                    "unknown blocking status \"{n}\"; expected FAIL or REVIEW_REQUIRED"
                )));
            }
        }
        blocking.review_required_blocks = names.iter().any(|n| n == "REVIEW_REQUIRED");
    }

    // not_applicable: written per-check exemptions. Structural
    // validation (selected check, non-empty reason, no bound command)
    // runs in `manifest::validate` so both manifest forms share it.
    let mut not_applicable: BTreeMap<String, String> = BTreeMap::new();
    if let Some(n) = not_applicable_yaml {
        let h = n.as_hash().ok_or_else(|| {
            ManifestError::Parse("`not_applicable` must be a mapping".to_string())
        })?;
        for (k, v) in h {
            let id = k
                .as_str()
                .ok_or_else(|| {
                    ManifestError::Parse("not_applicable key must be a string".to_string())
                })?
                .to_string();
            not_applicable.insert(id, expect_string(v, "not_applicable reason")?);
        }
    }

    // binding_policy: closed value set; an unknown value fails here
    // with the accepted set rather than silently defaulting.
    let mut binding_policy = BindingPolicy::default();
    if let Some(b) = binding_policy_yaml {
        let raw = expect_string(b, "binding_policy")?;
        binding_policy =
            BindingPolicy::parse(&raw).ok_or_else(|| ManifestError::UnknownBindingPolicy {
                got: raw,
                expected: BINDING_POLICIES.join(" or "),
            })?;
    }

    // project_commands: named, validated by the shared structural pass.
    let mut project_commands: BTreeMap<String, String> = BTreeMap::new();
    if let Some(p) = project_commands_yaml {
        let h = p.as_hash().ok_or_else(|| {
            ManifestError::Parse("`project_commands` must be a mapping".to_string())
        })?;
        for (k, v) in h {
            let name = k
                .as_str()
                .ok_or_else(|| {
                    ManifestError::Parse("project command name must be a string".to_string())
                })?
                .to_string();
            project_commands.insert(name, expect_string(v, "project command value")?);
        }
    }

    // contexts: list of provider names.
    let mut contexts: Vec<ContextDecl> = Vec::new();
    if let Some(cx) = contexts_yaml {
        let arr = cx
            .as_vec()
            .ok_or_else(|| ManifestError::Parse("`contexts` must be a list".to_string()))?;
        for item in arr {
            contexts.push(ContextDecl {
                provider: expect_string(item, "context provider")?,
            });
        }
    }

    // notes: operator annotations. Shape validation only — a note records
    // reasoning and is never read by the resolver, so there is nothing here
    // for it to be valid *against*. `manifest::validate` owns the non-empty
    // rule; `bound_exemption_reasons` owns redaction and the length bound.
    let mut notes: Vec<String> = Vec::new();
    if let Some(n) = notes_yaml {
        let arr = n
            .as_vec()
            .ok_or_else(|| ManifestError::Parse("`notes` must be a list".to_string()))?;
        for item in arr {
            notes.push(expect_string(item, "note")?);
        }
    }

    // governance_quality: the concern's code scope. The set itself is
    // enforced by the adapter (a subset only; an empty set is a
    // configuration error), so parsing here is shape validation only.
    let mut governance_quality = GovernanceQualityPolicy::default();
    if let Some(g) = governance_quality_yaml {
        let h = g.as_hash().ok_or_else(|| {
            ManifestError::Parse("`governance_quality` must be a mapping".to_string())
        })?;
        for (k, v) in h {
            match k.as_str() {
                Some("codes") => {
                    let arr = v.as_vec().ok_or_else(|| {
                        ManifestError::Parse(
                            "`governance_quality.codes` must be a list".to_string(),
                        )
                    })?;
                    let mut codes = Vec::with_capacity(arr.len());
                    for item in arr {
                        codes.push(expect_string(item, "governance_quality code")?);
                    }
                    governance_quality.codes = codes;
                }
                other => {
                    return Err(manifest::unknown_field_error(
                        other.unwrap_or_default(),
                        &["codes"],
                    ))
                }
            }
        }
    }

    let mut checks: Vec<ManifestCheck> = Vec::new();
    for (id, required) in &selected {
        checks.push(ManifestCheck {
            id: id.clone(),
            required: *required,
            enabled: true,
            command: commands.get(id).cloned(),
            timeout_ms: timeouts.get(id).copied(),
            source: Some("manifest".to_string()),
        });
    }
    for id in &deselected {
        checks.push(ManifestCheck {
            id: id.clone(),
            required: true,
            enabled: false,
            command: None,
            timeout_ms: None,
            source: Some("manifest".to_string()),
        });
    }

    // A non-built-in profile selects exactly its declared checks; a
    // built-in profile keeps its default set (adjustable via checks).
    let mut profiles: BTreeMap<String, Vec<String>> = BTreeMap::new();
    if !manifest::is_builtin_profile(&profile) {
        profiles.insert(profile.clone(), Vec::new());
    }

    let mut gate_manifest = GateManifest {
        version,
        profile,
        checks,
        blocking,
        project_commands,
        rule_pack_version: rule_pack.unwrap_or_else(|| "local".to_string()),
        contexts,
        triggers: Vec::new(),
        profiles,
        // Business YAML manifests use the built-in default source-size
        // policy; the native `gate.toml` `[source_size]` table is the
        // configuration surface for a custom boundary.
        source_size: SourceSizePolicy::default(),
        not_applicable,
        binding_policy,
        governance_quality,
        notes,
    };
    manifest::bound_exemption_reasons(&mut gate_manifest);
    manifest::validate(&gate_manifest)?;
    Ok(AiGateDoc {
        runtime,
        manifest: gate_manifest,
    })
}

enum Selection {
    Required,
    Optional,
    Disabled,
}

fn selection_of(v: &Yaml) -> Result<Selection, ManifestError> {
    match v {
        Yaml::Boolean(true) => Ok(Selection::Required),
        Yaml::Boolean(false) => Ok(Selection::Disabled),
        Yaml::String(s) if s == "optional" => Ok(Selection::Optional),
        _ => Err(ManifestError::Parse(
            "check value must be true, false, or \"optional\"".to_string(),
        )),
    }
}

fn expect_string(v: &Yaml, ctx: &str) -> Result<String, ManifestError> {
    v.as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| ManifestError::Parse(format!("`{ctx}` must be a string")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gate::manifest::resolve;

    const BYOK: &str = r#"
version: 1
runtime: driftwatchdog
profile: browser-extension
rule_pack: browser-extension@0.1.0
checks:
  build: true
  tests: true
  security: true
  accessibility: optional
commands:
  build: "npm run build"
  tests: "npm test"
blocking:
  - FAIL
  - REVIEW_REQUIRED
project_commands:
  lint: "npm run lint"
contexts:
  - git
  - openspec
"#;

    #[test]
    fn byok_style_manifest_resolves_with_identity() {
        let doc = parse_document(BYOK).unwrap();
        assert_eq!(doc.runtime.as_deref(), Some("driftwatchdog"));
        assert_eq!(doc.manifest.profile, "browser-extension");
        assert_eq!(doc.manifest.rule_pack_version, "browser-extension@0.1.0");
        let plan = resolve(&doc.manifest, &[]).unwrap();
        let ids: Vec<&str> = plan
            .plan
            .checks
            .iter()
            .map(|c| c.gate_id.as_str())
            .collect();
        assert_eq!(
            ids,
            vec![
                "accessibility",
                "build",
                "governance-quality",
                "security",
                "tests"
            ]
        );
        let acc = plan
            .plan
            .checks
            .iter()
            .find(|c| c.gate_id == "accessibility")
            .unwrap();
        assert!(!acc.required, "optional concern is not required");
        assert!(plan.project_commands.contains_key("lint"));
        assert_eq!(
            plan.contexts,
            vec!["git".to_string(), "openspec".to_string()]
        );
        assert!(plan.plan.policy.review_required_blocks);
    }

    #[test]
    fn command_binding_matches_selected_concern() {
        let doc = parse_document(BYOK).unwrap();
        let m = resolve(&doc.manifest, &[]).unwrap();
        let build = m.plan.checks.iter().find(|c| c.gate_id == "build").unwrap();
        assert!(build.required);
        // command presence is carried on the manifest, not the plan.
        let cmd = doc
            .manifest
            .checks
            .iter()
            .find(|c| c.id == "build")
            .unwrap()
            .command
            .clone();
        assert_eq!(cmd.as_deref(), Some("npm run build"));
    }

    #[test]
    fn unknown_field_fails_with_hint() {
        let err = parse_document("version: 1\nprofile: backend\nprofiel: x\n").unwrap_err();
        match err {
            ManifestError::UnknownField { field, suggestion } => {
                assert_eq!(field, "profiel");
                assert!(suggestion.contains("profile"), "got {suggestion}");
            }
            other => panic!("unexpected {other}"),
        }
    }

    #[test]
    fn missing_profile_fails() {
        assert!(parse_document("version: 1\nchecks:\n  build: true\n").is_err());
    }

    #[test]
    fn version_mismatch_fails() {
        let err = parse_document("version: 99\nprofile: backend\n").unwrap_err();
        assert!(matches!(err, ManifestError::UnknownVersion { .. }));
    }

    #[test]
    fn unknown_blocking_status_fails() {
        let err = parse_document("version: 1\nprofile: backend\nblocking: [MAYBE]\n").unwrap_err();
        assert!(err.to_string().contains("unknown blocking status"));
    }

    #[test]
    fn blocking_without_review_required_relaxes_policy() {
        let doc = parse_document("version: 1\nprofile: backend\nblocking: [FAIL]\n").unwrap();
        assert!(!doc.manifest.blocking.review_required_blocks);
    }

    #[test]
    fn command_for_unselected_check_fails() {
        let err = parse_document("version: 1\nprofile: backend\ncommands:\n  ghost: \"true\"\n")
            .unwrap_err();
        assert!(err.to_string().contains("not selected"));
    }

    #[test]
    fn empty_command_fails_before_execution() {
        let err = parse_document(
            "version: 1\nprofile: backend\nchecks:\n  build: true\ncommands:\n  build: \"\"\n",
        )
        .unwrap_err();
        assert!(matches!(err, ManifestError::EmptyCheckCommand { .. }));
    }

    /// The field seven fleet manifests already carry: an operator's
    /// reasoning must parse, and must survive intact.
    #[test]
    fn notes_parse_and_are_preserved_verbatim() {
        let doc = parse_document(
            "version: 1\nprofile: backend\nnotes:\n  - \"Set product_quality only after a quality block lands.\"\n  - \"The OpenSpec name preflight remains mandatory.\"\n",
        )
        .unwrap();
        assert_eq!(
            doc.manifest.notes,
            vec![
                "Set product_quality only after a quality block lands.".to_string(),
                "The OpenSpec name preflight remains mandatory.".to_string(),
            ]
        );
    }

    /// An annotation is not policy. Two manifests differing only in `notes`
    /// must resolve to the same plan: if a note could change a selection, a
    /// command, or a blocking decision, it would be policy wearing a
    /// comment's clothes. (`manifest_digest` is deliberately not compared
    /// here: it is a digest of the manifest *text*, so it tracks the note
    /// that is present — as an identity, not as a decision.)
    #[test]
    fn notes_do_not_change_the_resolved_plan() {
        let base = "version: 1\nprofile: backend\nchecks:\n  build: true\n  tests: true\n";
        let with_notes = format!("{base}notes:\n  - \"an annotation\"\n");
        let plain = resolve(&parse_document(base).unwrap().manifest, &[]).unwrap();
        let noted = resolve(&parse_document(&with_notes).unwrap().manifest, &[]).unwrap();
        assert_eq!(plain.plan.checks, noted.plan.checks);
        assert_eq!(plain.not_scheduled, noted.not_scheduled);
        assert_eq!(
            plain.plan.policy.review_required_blocks,
            noted.plan.policy.review_required_blocks
        );
    }

    /// `notes` is a *named* field, not a hole in the contract: a near miss
    /// still fails and still gets the hint.
    #[test]
    fn a_near_miss_of_notes_still_fails() {
        let err =
            parse_document("version: 1\nprofile: backend\nnote:\n  - \"typo\"\n").unwrap_err();
        assert!(
            matches!(err, ManifestError::UnknownField { .. }),
            "got {err}"
        );
    }

    #[test]
    fn an_empty_note_fails_rather_than_vanishing() {
        let err =
            parse_document("version: 1\nprofile: backend\nnotes:\n  - \"   \"\n").unwrap_err();
        assert!(
            matches!(err, ManifestError::EmptyNote { index: 0 }),
            "got {err}"
        );
    }

    /// A note is credential-redacted and length-bounded on the same terms as
    /// an exemption reason, so pasted prose cannot become a stored secret or
    /// an unbounded render.
    #[test]
    fn notes_are_redacted_and_bounded() {
        let long = "y".repeat(crate::gate::manifest::MAX_NOTE_BYTES * 2);
        let secret = "Set api_key=supersecret123 when wiring.".to_string();
        let doc = parse_document(&format!(
            "version: 1\nprofile: backend\nnotes:\n  - \"{secret}\"\n  - \"{long}\"\n"
        ))
        .unwrap();
        assert!(
            !doc.manifest.notes[0].contains("supersecret123"),
            "a note must not carry a credential into a render"
        );
        assert!(doc.manifest.notes[0].contains("[REDACTED]"));
        assert!(doc.manifest.notes[1].len() <= crate::gate::manifest::MAX_NOTE_BYTES + 3);
    }

    /// A manifest with no notes must not serialize a `notes` key, so a
    /// `gate.toml` digest that never mentioned notes still does not.
    #[test]
    fn absent_notes_are_not_serialized() {
        let doc = parse_document("version: 1\nprofile: backend\n").unwrap();
        assert!(doc.manifest.notes.is_empty());
        let out = toml::to_string(&doc.manifest).unwrap();
        assert!(
            !out.contains("notes"),
            "an empty notes list must not reach the serialized manifest: {out}"
        );
    }

    #[test]
    fn builtin_profile_keeps_defaults_and_deselect_removes() {
        let doc = parse_document("version: 1\nprofile: backend\nchecks:\n  secret-scan: false\n")
            .unwrap();
        let plan = resolve(&doc.manifest, &[]).unwrap();
        assert!(!plan.plan.checks.iter().any(|c| c.gate_id == "secret-scan"));
        assert!(plan.plan.checks.iter().any(|c| c.gate_id == "api-contract"));
    }

    #[test]
    fn domain_profile_preserves_declared_checks_and_adds_mandatory_governance() {
        let doc =
            parse_document("version: 1\nprofile: my-domain\nchecks:\n  alpha: true\n").unwrap();
        let plan = resolve(&doc.manifest, &[]).unwrap();
        let ids: Vec<&str> = plan
            .plan
            .checks
            .iter()
            .map(|c| c.gate_id.as_str())
            .collect();
        assert_eq!(ids, vec!["alpha", "governance-quality"]);
    }

    #[test]
    fn optional_selection_is_not_required() {
        let doc =
            parse_document("version: 1\nprofile: my-domain\nchecks:\n  a11y: optional\n").unwrap();
        let plan = resolve(&doc.manifest, &[]).unwrap();
        assert!(!plan.plan.checks[0].required);
    }

    #[test]
    fn top_level_must_be_mapping() {
        assert!(parse_document("- a\n- b\n").is_err());
    }

    /// The YAML form exposes the same command-binding vocabulary as
    /// `gate.toml`: a required check with no `commands.<id>` entry is
    /// unbound, and a written `not_applicable` reason exempts it.
    #[test]
    fn required_check_without_command_is_unbound_in_yaml_form() {
        let m = parse(
            r#"
version: 1
runtime: driftwatchdog
profile: minimal
checks:
  build: true
  tests: true
commands:
  tests: "npm test"
"#,
        )
        .unwrap();
        assert_eq!(m.binding_policy, BindingPolicy::Report);
        let r = resolve(&m, &[]).unwrap();
        assert_eq!(
            r.unbound,
            vec!["build".to_string(), "governance-quality".to_string()]
        );
        assert!(r.exemptions.is_empty());
    }

    #[test]
    fn not_applicable_exemption_clears_unbound_in_yaml_form() {
        let m = parse(
            r#"
version: 1
runtime: driftwatchdog
profile: minimal
checks:
  build: true
  accessibility: true
not_applicable:
  accessibility: "no UI surface in this project"
"#,
        )
        .unwrap();
        let r = resolve(&m, &[]).unwrap();
        assert_eq!(
            r.unbound,
            vec!["build".to_string(), "governance-quality".to_string()]
        );
        assert_eq!(
            r.exemptions.get("accessibility").map(String::as_str),
            Some("no UI surface in this project")
        );
        let check = r
            .plan
            .checks
            .iter()
            .find(|c| c.gate_id == "accessibility")
            .unwrap();
        assert!(check.exempt);
    }

    #[test]
    fn exemption_with_empty_reason_fails_in_yaml_form() {
        let err = parse(
            r#"
version: 1
runtime: driftwatchdog
profile: minimal
checks:
  a11y: true
not_applicable:
  a11y: "  "
"#,
        )
        .unwrap_err();
        assert!(matches!(err, ManifestError::EmptyExemptionReason { .. }));
        assert!(err.to_string().contains("a11y"));
    }

    #[test]
    fn exemption_for_unselected_check_fails_in_yaml_form() {
        let err = parse(
            r#"
version: 1
runtime: driftwatchdog
profile: minimal
checks:
  build: true
not_applicable:
  a11y: "no UI here"
"#,
        )
        .unwrap_err();
        assert!(matches!(err, ManifestError::ExemptionNotSelected { .. }));
    }

    #[test]
    fn exemption_that_binds_a_command_fails_in_yaml_form() {
        let err = parse(
            r#"
version: 1
runtime: driftwatchdog
profile: minimal
checks:
  build: true
commands:
  build: "npm run build"
not_applicable:
  build: "we do have a command"
"#,
        )
        .unwrap_err();
        assert!(matches!(
            err,
            ManifestError::ExemptionConflictsWithCommand { .. }
        ));
    }

    #[test]
    fn yaml_exemption_reason_is_bounded_and_redacted() {
        let long = "y".repeat(crate::gate::manifest::MAX_EXEMPTION_REASON_BYTES * 2);
        let m = parse(&format!(
            r#"
version: 1
runtime: driftwatchdog
profile: minimal
checks:
  a11y: true
not_applicable:
  a11y: "api_key=supersecret123 {long}"
"#
        ))
        .unwrap();
        let reason = m.not_applicable.get("a11y").unwrap();
        assert!(!reason.contains("supersecret123"), "{reason}");
        assert!(reason.len() <= crate::gate::manifest::MAX_EXEMPTION_REASON_BYTES + 3);
    }

    #[test]
    fn unknown_binding_policy_fails_with_accepted_set_in_yaml_form() {
        let err = parse(
            r#"
version: 1
runtime: driftwatchdog
profile: minimal
binding_policy: "MAYBE"
"#,
        )
        .unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("MAYBE"), "{msg}");
        assert!(msg.contains("report") && msg.contains("reject"), "{msg}");
    }

    #[test]
    fn reject_policy_refuses_to_resolve_in_yaml_form() {
        let err = parse(
            r#"
version: 1
runtime: driftwatchdog
profile: minimal
binding_policy: reject
checks:
  build: true
  tests: true
commands:
  tests: "npm test"
"#,
        )
        .unwrap_err();
        assert!(matches!(err, ManifestError::UnboundRequiredCheck { .. }));
        assert!(err.to_string().contains("build"), "{err}");
        assert!(!err.to_string().contains("tests"), "{err}");
    }

    // --- per-check timeout -------------------------------------------------

    const YAML_TIMEOUTS: &str = r#"
version: 1
runtime: driftwatchdog
profile: minimal
checks:
  quick: true
  slow: true
commands:
  quick: "true"
  slow: "sleep 5"
timeouts:
  slow: 900000
"#;

    /// The single-owner invariant: a YAML manifest and the equivalent
    /// native `gate.toml` must produce the *same* `ManifestCheck`. If
    /// they diverged, the value rules would have to exist twice and one
    /// of the copies would eventually be wrong.
    #[test]
    fn yaml_and_native_forms_resolve_to_the_same_check_timeout() {
        const NATIVE_EQUIVALENT: &str = r#"
version = 1
profile = "minimal"

[[checks]]
id = "quick"
command = "true"

[[checks]]
id = "slow"
command = "sleep 5"
timeout_ms = 900000
"#;
        let from_yaml = parse(YAML_TIMEOUTS).unwrap();
        let from_toml = crate::gate::manifest::parse(NATIVE_EQUIVALENT).unwrap();

        // Same field on the check, per check.
        for id in ["quick", "slow"] {
            let y = from_yaml.checks.iter().find(|c| c.id == id).unwrap();
            let t = from_toml.checks.iter().find(|c| c.id == id).unwrap();
            assert_eq!(y.timeout_ms, t.timeout_ms, "{id} disagrees across forms");
            assert_eq!(y.command, t.command, "{id} command disagrees");
        }
        assert_eq!(
            from_yaml
                .checks
                .iter()
                .find(|c| c.id == "slow")
                .unwrap()
                .timeout_ms,
            Some(900_000)
        );
        // ...and the same resolved timeout, which is what execution
        // reads. Compared on the timeout block rather than the whole
        // rendering: the two forms label a check's `source` differently
        // (the YAML converter stamps `manifest`, the native form leaves
        // it absent), so full-plan text was never equal and asserting it
        // would be asserting a difference that has nothing to do with
        // timeouts.
        let ry = resolve(&from_yaml, &[]).unwrap();
        let rt = resolve(&from_toml, &[]).unwrap();
        assert_eq!(ry.check_timeouts, rt.check_timeouts);
        assert_eq!(
            timeout_lines(&crate::gate::manifest::render_plan(&ry)),
            timeout_lines(&crate::gate::manifest::render_plan(&rt))
        );
    }

    /// The rendered `check timeouts:` block, lines only, so two
    /// renderings can be compared on deadlines without dragging in
    /// unrelated fields.
    fn timeout_lines(rendered: &str) -> Vec<&str> {
        rendered
            .lines()
            .skip_while(|l| !l.starts_with("check timeouts"))
            .take_while(|l| l.starts_with("  - ") || l.starts_with("check timeouts"))
            .collect()
    }

    /// The value rules are the shared validator's, not a second copy in
    /// this parser. Both refusals must surface from the same variant a
    /// native manifest produces.
    #[test]
    fn yaml_form_reports_the_same_timeout_refusals() {
        for (ms, must_name) in [("0", "omit the key"), ("3600001", "3600000")] {
            let doc = format!(
                "version: 1\nruntime: driftwatchdog\nprofile: minimal\nchecks:\n  slow: true\ncommands:\n  slow: \"sleep 5\"\ntimeouts:\n  slow: {ms}\n"
            );
            let err = parse(&doc).unwrap_err();
            assert!(
                matches!(&err, ManifestError::InvalidCheckTimeout { id, .. } if id == "slow"),
                "ms={ms} produced {err}"
            );
            let msg = err.to_string();
            assert!(msg.contains("slow"), "{msg}");
            assert!(msg.contains(must_name), "{msg}");
        }
    }

    /// A deadline bound to no scheduled check is a value that can never
    /// take effect, so it is refused rather than stored.
    #[test]
    fn a_timeout_for_an_unselected_check_is_refused() {
        let err = parse(
            r#"
version: 1
runtime: driftwatchdog
profile: minimal
checks:
  quick: true
timeouts:
  ghost: 900000
"#,
        )
        .unwrap_err();
        assert!(err.to_string().contains("ghost"), "{err}");
        assert!(err.to_string().contains("not selected"), "{err}");
    }

    /// A YAML manifest that says nothing about timeouts resolves to the
    /// default, so all 76 existing manifests keep their behaviour.
    #[test]
    fn yaml_manifest_without_timeouts_resolves_to_the_default() {
        const PLAIN: &str = r#"
version: 1
runtime: driftwatchdog
profile: minimal
checks:
  quick: true
commands:
  quick: "true"
"#;
        let m = parse(PLAIN).unwrap();
        assert!(m.checks.iter().all(|c| c.timeout_ms.is_none()));
        let plan = resolve(&m, &[]).unwrap();
        assert_eq!(plan.check_timeouts.get("quick"), Some(&60_000));
        assert!(crate::gate::manifest::render_plan(&plan).contains("quick: 60000ms"));
    }

    #[test]
    fn a_non_integer_yaml_timeout_is_a_parse_error() {
        let err = parse(
            r#"
version: 1
runtime: driftwatchdog
profile: minimal
checks:
  quick: true
timeouts:
  quick: "soon"
"#,
        )
        .unwrap_err();
        assert!(err.to_string().contains("milliseconds"), "{err}");
    }
}
