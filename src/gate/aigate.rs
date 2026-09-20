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
//!   build: "npm run build"
//! blocking: [FAIL, REVIEW_REQUIRED]  # optional; REVIEW_REQUIRED toggles fail-closed
//! project_commands:               # optional named project commands
//!   test: "npm test"
//! contexts: [git, openspec]       # optional context providers
//! ```
//!
//! Unknown fields fail with a `did you mean` hint. A `profile` that is
//! not a built-in name becomes a project-defined profile whose selection
//! is exactly the `true`/`"optional"` entries in `checks`.

use std::collections::BTreeMap;

use yaml_rust2::{Yaml, YamlLoader};

use crate::gate::dto::GATE_CONTRACT_VERSION;
use crate::gate::manifest::{
    self, ContextDecl, GateManifest, ManifestBlocking, ManifestCheck, ManifestError,
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
    "blocking",
    "project_commands",
    "contexts",
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
    let mut blocking_yaml: Option<&Yaml> = None;
    let mut project_commands_yaml: Option<&Yaml> = None;
    let mut contexts_yaml: Option<&Yaml> = None;

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
            "blocking" => blocking_yaml = Some(v),
            "project_commands" => project_commands_yaml = Some(v),
            "contexts" => contexts_yaml = Some(v),
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

    let mut checks: Vec<ManifestCheck> = Vec::new();
    for (id, required) in &selected {
        checks.push(ManifestCheck {
            id: id.clone(),
            required: *required,
            enabled: true,
            command: commands.get(id).cloned(),
            source: Some("manifest".to_string()),
        });
    }
    for id in &deselected {
        checks.push(ManifestCheck {
            id: id.clone(),
            required: true,
            enabled: false,
            command: None,
            source: Some("manifest".to_string()),
        });
    }

    // A non-built-in profile selects exactly its declared checks; a
    // built-in profile keeps its default set (adjustable via checks).
    let mut profiles: BTreeMap<String, Vec<String>> = BTreeMap::new();
    if !manifest::is_builtin_profile(&profile) {
        profiles.insert(profile.clone(), Vec::new());
    }

    let gate_manifest = GateManifest {
        version,
        profile,
        checks,
        blocking,
        project_commands,
        rule_pack_version: rule_pack.unwrap_or_else(|| "local".to_string()),
        contexts,
        triggers: Vec::new(),
        profiles,
    };
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
        assert_eq!(ids, vec!["accessibility", "build", "security", "tests"]);
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

    #[test]
    fn builtin_profile_keeps_defaults_and_deselect_removes() {
        let doc = parse_document("version: 1\nprofile: backend\nchecks:\n  secret-scan: false\n")
            .unwrap();
        let plan = resolve(&doc.manifest, &[]).unwrap();
        assert!(!plan.plan.checks.iter().any(|c| c.gate_id == "secret-scan"));
        assert!(plan.plan.checks.iter().any(|c| c.gate_id == "api-contract"));
    }

    #[test]
    fn domain_profile_selects_only_declared_checks() {
        let doc =
            parse_document("version: 1\nprofile: my-domain\nchecks:\n  alpha: true\n").unwrap();
        let plan = resolve(&doc.manifest, &[]).unwrap();
        let ids: Vec<&str> = plan
            .plan
            .checks
            .iter()
            .map(|c| c.gate_id.as_str())
            .collect();
        assert_eq!(ids, vec!["alpha"]);
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
}
