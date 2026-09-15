//! Provider-neutral AI evaluation.
//!
//! Some concerns (UX, architecture, privacy interpretation, responsive
//! layout) cannot be proven mechanically. This module defines a typed
//! evaluator contract over rules and evidence without embedding an LLM
//! in the core and without making any provider mandatory.
//!
//! ## Contract
//!
//! * The caller builds a bounded, secret-redacted request
//!   ([`AiEvalRequest`]) from selected [`AiRule`]s, evidence references
//!   with previews, and generic context previews. Raw artifact bytes are
//!   never sent; only capped previews and references travel.
//! * A provider (an external command emitting JSON, never an in-process
//!   SDK) returns [`AiProviderOutput`]: typed status, confidence, rules
//!   checked, violations, evidence, missing evidence, reason, and
//!   recommended actions.
//! * [`validate_output`] enforces fail-closed semantics: `PASS` requires
//!   valid schema **and** sufficient available evidence; malformed output
//!   or an unknown status is `REVIEW_REQUIRED`, never `PASS`.
//! * [`run_ai_evaluation`] is the opt-in invocation boundary: disabled
//!   AI yields `NOT_APPLICABLE` without any provider call; an enabled
//!   concern with no configured provider follows explicit
//!   [`MissingProviderPolicy`]; provider spawn/timeout/signal failures
//!   are distinguishable `REVIEW_REQUIRED` results and never mutate code.
//! * Validated outputs are stored by callers as evidence-backed
//!   evaluations carrying provider/model id plus the prompt and rule
//!   digests ([`AiEvaluationRecord`]).
//!
//! No network call is made here beyond spawning the configured local
//! provider command. No API keys are stored. No OpenSpec types appear.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::checker::runner::{run_checker, CheckerSpec};
use crate::doctor::check::{Check, Status};
use crate::gate::context::ContextDocument;
use crate::gate::dto::{
    MAX_DIAGNOSTIC_BYTES, MAX_EVIDENCE_KEY_BYTES, MAX_EVIDENCE_REFS, MAX_LOCATION_BYTES,
    MAX_MISSING_EVIDENCE, MAX_REMEDIATION_BYTES, MAX_TITLE_BYTES,
};
use crate::gate::evidence::ArtifactRecord;
use crate::gate::redact::{bound_text, bound_text_with_extra};
use crate::gate::types::{EvidenceRef, Finding, GateResult, GateSeverity, GateStatus};
use crate::util::truncate_char_boundary;

/// Largest provider prompt submitted (32 KiB of redacted JSON).
pub const MAX_AI_PROMPT_BYTES: usize = 32 * 1024;
/// Largest provider response accepted (64 KiB).
pub const MAX_AI_RESPONSE_BYTES: usize = 64 * 1024;
/// Largest number of rules per evaluation.
pub const MAX_AI_RULES: usize = 16;
/// Largest number of violations accepted per evaluation.
pub const MAX_AI_VIOLATIONS: usize = 32;
/// Largest reason text kept.
pub const MAX_AI_REASON_BYTES: usize = 2048;
/// Largest single recommended action kept.
pub const MAX_AI_ACTION_BYTES: usize = 512;
/// Largest number of recommended actions kept.
pub const MAX_AI_ACTIONS: usize = 16;
/// Largest rule text submitted per rule.
pub const MAX_AI_RULE_TEXT_BYTES: usize = 2048;

/// Policy when an AI concern is enabled but no provider is configured.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum MissingProviderPolicy {
    /// Fail closed: needs human review before the change completes.
    #[default]
    ReviewRequired,
    /// Explicitly skip: the concern does not apply without a provider.
    NotApplicable,
}

impl MissingProviderPolicy {
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "review_required" | "review-required" | "review" => {
                Some(MissingProviderPolicy::ReviewRequired)
            }
            "not_applicable" | "not-applicable" | "skip" => {
                Some(MissingProviderPolicy::NotApplicable)
            }
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            MissingProviderPolicy::ReviewRequired => "review_required",
            MissingProviderPolicy::NotApplicable => "not_applicable",
        }
    }
}

/// Provider configuration. Opt-in: `enabled = false` (the default) never
/// invokes a provider. `command`/`args` name a local executable emitting
/// the provider JSON schema on stdout; there is no built-in model and no
/// API key storage.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiProviderConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_provider_name")]
    pub provider: String,
    #[serde(default = "default_model_name")]
    pub model: String,
    #[serde(default)]
    pub command: Option<String>,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default = "default_ai_timeout")]
    pub timeout_ms: u64,
    #[serde(default)]
    pub missing_policy: MissingProviderPolicy,
}

fn default_provider_name() -> String {
    "unconfigured".to_string()
}

fn default_model_name() -> String {
    "unconfigured".to_string()
}

fn default_ai_timeout() -> u64 {
    30_000
}

impl Default for AiProviderConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            provider: default_provider_name(),
            model: default_model_name(),
            command: None,
            args: Vec::new(),
            timeout_ms: default_ai_timeout(),
            missing_policy: MissingProviderPolicy::ReviewRequired,
        }
    }
}

/// One semantic rule submitted for AI review.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiRule {
    pub id: String,
    pub text: String,
}

/// One violation reported by the provider.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiViolation {
    #[serde(default)]
    pub rule: String,
    #[serde(default)]
    pub message: String,
    #[serde(default)]
    pub evidence: Vec<String>,
}

/// Provider output. Tolerant by contract: unknown JSON fields are
/// ignored, but the required typed fields must be present and valid or
/// the output is `REVIEW_REQUIRED` (never `PASS`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiProviderOutput {
    /// One of `PASS`, `FAIL`, `REVIEW_REQUIRED`, `NOT_APPLICABLE`.
    #[serde(default)]
    pub status: String,
    /// 0-100 confidence. Optional; clamped when present.
    #[serde(default)]
    pub confidence: Option<u8>,
    #[serde(default)]
    pub rules_checked: Vec<String>,
    #[serde(default)]
    pub violations: Vec<AiViolation>,
    #[serde(default)]
    pub evidence: Vec<String>,
    #[serde(default)]
    pub missing_evidence: Vec<String>,
    #[serde(default)]
    pub reason: String,
    #[serde(default)]
    pub actions: Vec<String>,
}

/// Bounded, redacted provider request: canonical JSON plus digests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiEvalRequest {
    /// Redacted, bounded JSON submitted to the provider.
    pub prompt_json: String,
    /// `sha256:` digest over the prompt JSON.
    pub prompt_digest: String,
    /// `sha256:` digest over the sorted rule ids + texts.
    pub rule_digest: String,
    /// True when any input was truncated to fit the bounds.
    pub truncated: bool,
}

/// Stored evaluation: validated result plus provider/model identity and
/// prompt/rule digests for reproducibility.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiEvaluationRecord {
    pub gate_id: String,
    pub provider: String,
    pub model: String,
    pub prompt_digest: String,
    pub rule_digest: String,
    pub result: GateResult,
}

/// Provider invocation failures. Every variant maps to a
/// `REVIEW_REQUIRED` [`GateResult`], never to a panic.
#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum AiError {
    #[error("AI evaluation is disabled (no provider call made)")]
    Disabled,
    #[error("no AI provider configured for `{gate_id}`")]
    MissingProvider { gate_id: String },
    #[error("AI provider `{provider}` could not start: {detail}")]
    SpawnFailed { provider: String, detail: String },
    #[error("AI provider `{provider}` exceeded its timeout of {timeout_ms}ms")]
    Timeout { provider: String, timeout_ms: u64 },
    #[error("AI provider `{provider}` was killed by a signal")]
    Signalled { provider: String },
    #[error("AI provider `{provider}` emitted malformed output: {detail}")]
    MalformedOutput { provider: String, detail: String },
    #[error("AI provider `{provider}` exited {code}")]
    NonZeroExit { provider: String, code: String },
}

fn digest_hex(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    let hex = format!("{:x}", h.finalize());
    format!("sha256:{}", &hex[..16.min(hex.len())])
}

fn bound_key(s: &str) -> String {
    truncate_char_boundary(
        &bound_text(s, MAX_EVIDENCE_KEY_BYTES),
        MAX_EVIDENCE_KEY_BYTES,
    )
}

/// Build a bounded, secret-redacted provider request.
///
/// Only evidence keys + capped previews and context previews travel;
/// raw artifact bytes are never embedded. Secrets (recognized patterns
/// plus `sensitive` values) are redacted before serialization, then the
/// document is truncated at a char boundary to [`MAX_AI_PROMPT_BYTES`].
pub fn build_request(
    rules: &[AiRule],
    evidence: &[EvidenceRef],
    contexts: &[ContextDocument],
    sensitive: &[String],
) -> AiEvalRequest {
    let extra: Vec<&str> = sensitive.iter().map(|s| s.as_str()).collect();
    let mut truncated = false;
    let rules_capped: Vec<BTreeMap<String, String>> = rules
        .iter()
        .take(MAX_AI_RULES)
        .map(|r| {
            let mut m = BTreeMap::new();
            m.insert(
                "id".to_string(),
                truncate_char_boundary(&bound_text(&r.id, 256), 256),
            );
            m.insert(
                "text".to_string(),
                bound_text_with_extra(&r.text, &extra, MAX_AI_RULE_TEXT_BYTES),
            );
            m
        })
        .collect();
    if rules.len() > MAX_AI_RULES {
        truncated = true;
    }
    let evidence_capped: Vec<BTreeMap<String, String>> = evidence
        .iter()
        .take(MAX_EVIDENCE_REFS)
        .map(|e| {
            let mut m = BTreeMap::new();
            m.insert("key".to_string(), bound_key(&e.key));
            if let Some(p) = e.preview.as_deref() {
                m.insert(
                    "preview".to_string(),
                    bound_text_with_extra(p, &extra, MAX_LOCATION_BYTES),
                );
            }
            if let Some(d) = e.digest.as_deref() {
                m.insert("digest".to_string(), truncate_char_boundary(d, 128));
            }
            m
        })
        .collect();
    if evidence.len() > MAX_EVIDENCE_REFS {
        truncated = true;
    }
    let contexts_capped: Vec<BTreeMap<String, String>> = contexts
        .iter()
        .take(MAX_AI_RULES)
        .map(|d| {
            let mut m = BTreeMap::new();
            m.insert(
                "provider".to_string(),
                truncate_char_boundary(&d.provider, 128),
            );
            m.insert("kind".to_string(), truncate_char_boundary(&d.kind, 128));
            m.insert("path".to_string(), truncate_char_boundary(&d.path, 512));
            m.insert(
                "preview".to_string(),
                bound_text_with_extra(d.safe_preview(), &extra, MAX_LOCATION_BYTES),
            );
            m
        })
        .collect();
    let mut doc = BTreeMap::new();
    doc.insert(
        "schema".to_string(),
        serde_json::json!("gate-ai-evaluation/v1"),
    );
    doc.insert(
        "rules".to_string(),
        serde_json::to_value(&rules_capped).unwrap_or_default(),
    );
    doc.insert(
        "evidence".to_string(),
        serde_json::to_value(&evidence_capped).unwrap_or_default(),
    );
    doc.insert(
        "context".to_string(),
        serde_json::to_value(&contexts_capped).unwrap_or_default(),
    );
    let mut prompt_json = serde_json::to_string(&doc).unwrap_or_else(|_| "{}".to_string());
    // Redact once more over the serialized form so key-adjacent secrets
    // cannot leak through structure, then bound.
    prompt_json = bound_text_with_extra(&prompt_json, &extra, MAX_AI_PROMPT_BYTES);
    if prompt_json.len() >= MAX_AI_PROMPT_BYTES {
        truncated = true;
    }
    let prompt_digest = digest_hex(prompt_json.as_bytes());
    let mut rule_canonical = String::new();
    for r in rules.iter().take(MAX_AI_RULES) {
        rule_canonical.push_str(&r.id);
        rule_canonical.push('\n');
        rule_canonical.push_str(&r.text);
        rule_canonical.push('\n');
    }
    let rule_digest = digest_hex(rule_canonical.as_bytes());
    AiEvalRequest {
        prompt_json,
        prompt_digest,
        rule_digest,
        truncated,
    }
}

fn infra_result(
    gate_id: &str,
    config: &AiProviderConfig,
    error: &AiError,
    missing: Vec<String>,
    prompt_digest: Option<&str>,
    rule_digest: Option<&str>,
) -> GateResult {
    let mut diagnostic = bound_text(&error.to_string(), MAX_DIAGNOSTIC_BYTES);
    if let Some(pd) = prompt_digest {
        diagnostic.push_str(&format!(" | prompt {pd}"));
    }
    if let Some(rd) = rule_digest {
        diagnostic.push_str(&format!(" rule {rd}"));
    }
    let diagnostic = bound_text(&diagnostic, MAX_DIAGNOSTIC_BYTES);
    let mut missing_evidence: Vec<String> = missing.into_iter().map(|k| bound_key(&k)).collect();
    if missing_evidence.is_empty() {
        missing_evidence.push("ai:output".to_string());
    }
    missing_evidence.truncate(MAX_MISSING_EVIDENCE);
    GateResult {
        gate_id: truncate_char_boundary(gate_id, 128),
        source: truncate_char_boundary(&format!("ai:{}", config.provider), 256),
        status: GateStatus::ReviewRequired,
        severity: GateSeverity::Warning,
        findings: vec![],
        evidence: vec![],
        missing_evidence,
        diagnostic: Some(diagnostic),
        remediation: Some(bound_text(
            "Configure an AI provider or resolve the provider error before trusting this concern.",
            MAX_REMEDIATION_BYTES,
        )),
    }
}

/// Validate provider output bytes into a [`GateResult`].
///
/// Fail-closed rules (in order):
/// * oversized or malformed JSON → `REVIEW_REQUIRED`, never `PASS`;
/// * unknown status string → `REVIEW_REQUIRED`, never `PASS`;
/// * `PASS` requires valid schema, a non-empty `rules_checked` list,
///   zero violations, and non-empty evidence where every key resolves
///   to an available artifact — otherwise `REVIEW_REQUIRED` naming the
///   missing evidence;
/// * any violation → `FAIL` with one finding per violation (capped).
pub fn validate_output(
    gate_id: &str,
    config: &AiProviderConfig,
    request: &AiEvalRequest,
    output_bytes: &[u8],
    resolve: &dyn Fn(&str) -> Option<ArtifactRecord>,
) -> GateResult {
    let provider = config.provider.clone();
    let model = config.model.clone();
    if output_bytes.len() > MAX_AI_RESPONSE_BYTES * 4 {
        return infra_result(
            gate_id,
            config,
            &AiError::MalformedOutput {
                provider,
                detail: format!(
                    "response is {} bytes, exceeding the intake limit",
                    output_bytes.len()
                ),
            },
            vec!["ai:output".to_string()],
            Some(&request.prompt_digest),
            Some(&request.rule_digest),
        );
    }
    let text =
        String::from_utf8_lossy(&output_bytes[..output_bytes.len().min(MAX_AI_RESPONSE_BYTES * 2)]);
    let output: AiProviderOutput = match serde_json::from_str(&text) {
        Ok(o) => o,
        Err(e) => {
            return infra_result(
                gate_id,
                config,
                &AiError::MalformedOutput {
                    provider,
                    detail: truncate_char_boundary(&e.to_string(), 512),
                },
                vec!["ai:output".to_string()],
                Some(&request.prompt_digest),
                Some(&request.rule_digest),
            );
        }
    };
    let status = match GateStatus::parse(&output.status) {
        Some(s) => s,
        None => {
            return infra_result(
                gate_id,
                config,
                &AiError::MalformedOutput {
                    provider,
                    detail: format!(
                        "invalid status `{}`",
                        output.status.chars().take(64).collect::<String>()
                    ),
                },
                vec!["ai:output".to_string()],
                Some(&request.prompt_digest),
                Some(&request.rule_digest),
            );
        }
    };
    // Bound + redact every free-text field before persistence.
    let reason = bound_text(&output.reason, MAX_AI_REASON_BYTES);
    let actions: Vec<String> = output
        .actions
        .into_iter()
        .take(MAX_AI_ACTIONS)
        .map(|a| bound_text(&a, MAX_AI_ACTION_BYTES))
        .collect();
    let mut missing: Vec<String> = output
        .missing_evidence
        .into_iter()
        .take(MAX_MISSING_EVIDENCE)
        .map(|k| bound_key(&k))
        .collect();
    let mut evidence_keys: Vec<String> = output
        .evidence
        .into_iter()
        .take(MAX_EVIDENCE_REFS)
        .map(|k| bound_key(&k))
        .collect();
    let rules_checked: Vec<String> = output
        .rules_checked
        .into_iter()
        .take(MAX_AI_RULES + 8)
        .map(|r| truncate_char_boundary(&bound_text(&r, 256), 256))
        .collect();
    let violations: Vec<AiViolation> = output
        .violations
        .into_iter()
        .take(MAX_AI_VIOLATIONS + 1)
        .map(|v| AiViolation {
            rule: truncate_char_boundary(&bound_text(&v.rule, 256), 256),
            message: bound_text(&v.message, MAX_TITLE_BYTES),
            evidence: v
                .evidence
                .into_iter()
                .take(8)
                .map(|k| bound_key(&k))
                .collect(),
        })
        .collect();
    let violations_truncated = violations.len() > MAX_AI_VIOLATIONS;
    let violations: Vec<AiViolation> = violations.into_iter().take(MAX_AI_VIOLATIONS).collect();

    let mut evidence_refs: Vec<EvidenceRef> = evidence_keys
        .iter()
        .map(|k| {
            let rec = resolve(k);
            EvidenceRef {
                key: k.clone(),
                digest: rec.as_ref().map(|r| r.digest.clone()),
                media_type: None,
                byte_size: rec.as_ref().map(|r| r.byte_size),
                preview: rec.as_ref().and_then(|r| r.preview.clone()),
            }
        })
        .collect();
    evidence_refs.truncate(MAX_EVIDENCE_REFS);

    let meta = format!(
        "provider {provider}/{model} prompt {} rule {}",
        request.prompt_digest, request.rule_digest
    );
    let mut diagnostic = if reason.trim().is_empty() {
        meta.clone()
    } else {
        format!("{reason} | {meta}")
    };
    if !rules_checked.is_empty() {
        diagnostic.push_str(&format!(" | rules: {}", rules_checked.join(",")));
    }
    if !actions.is_empty() {
        diagnostic.push_str(&format!(" | actions: {}", actions.join("; ")));
    }
    if violations_truncated {
        diagnostic.push_str(&format!(" | violations truncated to {MAX_AI_VIOLATIONS}"));
    }
    let diagnostic = bound_text(&diagnostic, MAX_DIAGNOSTIC_BYTES);
    let remediation = if actions.is_empty() {
        None
    } else {
        Some(bound_text(&actions.join("; "), MAX_REMEDIATION_BYTES))
    };

    // Violations decide before the claimed status: a provider that
    // reports violations cannot claim PASS.
    if !violations.is_empty() {
        let findings: Vec<Finding> = violations
            .iter()
            .map(|v| Finding {
                title: v.message.clone(),
                severity: GateSeverity::Warning,
                location: None,
                rule: if v.rule.is_empty() {
                    None
                } else {
                    Some(v.rule.clone())
                },
            })
            .collect();
        return GateResult {
            gate_id: truncate_char_boundary(gate_id, 128),
            source: truncate_char_boundary(&format!("ai:{provider}"), 256),
            status: GateStatus::Fail,
            severity: GateSeverity::Warning,
            findings,
            evidence: evidence_refs,
            missing_evidence: missing,
            diagnostic: Some(diagnostic),
            remediation: remediation.or_else(|| {
                Some(bound_text(
                    "Investigate the reported AI findings and add regression coverage.",
                    MAX_REMEDIATION_BYTES,
                ))
            }),
        };
    }

    match status {
        GateStatus::Pass => {
            // Evidence-backed PASS guard: non-empty rules, non-empty
            // evidence, every key available.
            let mut unbacked: Vec<String> = Vec::new();
            for k in &evidence_keys {
                match resolve(k) {
                    Some(rec) if rec.available => {}
                    _ => unbacked.push(k.clone()),
                }
            }
            if rules_checked.is_empty() || evidence_keys.is_empty() || !unbacked.is_empty() {
                for k in unbacked {
                    if !missing.contains(&k) {
                        missing.push(k);
                    }
                }
                if missing.is_empty() {
                    missing.push("ai:evidence".to_string());
                }
                missing.truncate(MAX_MISSING_EVIDENCE);
                evidence_keys.clear();
                return GateResult {
                    gate_id: truncate_char_boundary(gate_id, 128),
                    source: truncate_char_boundary(&format!("ai:{provider}"), 256),
                    status: GateStatus::ReviewRequired,
                    severity: GateSeverity::Warning,
                    findings: vec![],
                    evidence: evidence_refs,
                    missing_evidence: missing,
                    diagnostic: Some(bound_text(
                        &format!("{diagnostic} | PASS requires explicit available evidence; cannot claim evidence-backed PASS"),
                        MAX_DIAGNOSTIC_BYTES,
                    )),
                    remediation: Some(bound_text(
                        "Supply the missing evidence or keep this concern in review.",
                        MAX_REMEDIATION_BYTES,
                    )),
                };
            }
            GateResult {
                gate_id: truncate_char_boundary(gate_id, 128),
                source: truncate_char_boundary(&format!("ai:{provider}"), 256),
                status: GateStatus::Pass,
                severity: GateSeverity::Info,
                findings: vec![],
                evidence: evidence_refs,
                missing_evidence: vec![],
                diagnostic: Some(diagnostic),
                remediation: None,
            }
        }
        GateStatus::Fail => GateResult {
            gate_id: truncate_char_boundary(gate_id, 128),
            source: truncate_char_boundary(&format!("ai:{provider}"), 256),
            status: GateStatus::Fail,
            severity: GateSeverity::Warning,
            findings: vec![],
            evidence: evidence_refs,
            missing_evidence: missing,
            diagnostic: Some(diagnostic),
            remediation,
        },
        GateStatus::ReviewRequired => GateResult {
            gate_id: truncate_char_boundary(gate_id, 128),
            source: truncate_char_boundary(&format!("ai:{provider}"), 256),
            status: GateStatus::ReviewRequired,
            severity: GateSeverity::Warning,
            findings: vec![],
            evidence: evidence_refs,
            missing_evidence: missing,
            diagnostic: Some(diagnostic),
            remediation,
        },
        GateStatus::NotApplicable => GateResult {
            gate_id: truncate_char_boundary(gate_id, 128),
            source: truncate_char_boundary(&format!("ai:{provider}"), 256),
            status: GateStatus::NotApplicable,
            severity: GateSeverity::Info,
            findings: vec![],
            evidence: evidence_refs,
            missing_evidence: missing,
            diagnostic: Some(diagnostic),
            remediation: None,
        },
    }
}

/// Caller-supplied inputs for one AI evaluation. Bundled so the
/// invocation boundary keeps a stable, clippy-clean signature.
#[derive(Debug, Clone)]
pub struct AiEvalInput<'a> {
    pub gate_id: &'a str,
    pub rules: &'a [AiRule],
    pub evidence: &'a [EvidenceRef],
    pub contexts: &'a [ContextDocument],
    pub config: &'a AiProviderConfig,
    pub sensitive: &'a [String],
    pub project_root: &'a Path,
}

/// Run one AI evaluation through the bounded provider boundary.
///
/// * disabled config → `NOT_APPLICABLE`, no provider call;
/// * enabled without `command` → explicit missing-provider policy;
/// * provider spawn/timeout/signal/nonzero-unparsable → distinct
///   `REVIEW_REQUIRED` diagnostics; no code is changed by any path.
pub fn run_ai_evaluation(
    input: &AiEvalInput<'_>,
    resolve: &dyn Fn(&str) -> Option<ArtifactRecord>,
) -> GateResult {
    let gate_id = input.gate_id;
    let rules = input.rules;
    let evidence = input.evidence;
    let contexts = input.contexts;
    let config = input.config;
    let sensitive = input.sensitive;
    let project_root = input.project_root;
    let request = build_request(rules, evidence, contexts, sensitive);
    if !config.enabled {
        return GateResult {
            gate_id: truncate_char_boundary(gate_id, 128),
            source: truncate_char_boundary(&format!("ai:{}", config.provider), 256),
            status: GateStatus::NotApplicable,
            severity: GateSeverity::Info,
            findings: vec![],
            evidence: vec![],
            missing_evidence: vec![],
            diagnostic: Some(bound_text(
                &format!(
                    "AI evaluation disabled; no provider call made | prompt {} rule {}",
                    request.prompt_digest, request.rule_digest
                ),
                MAX_DIAGNOSTIC_BYTES,
            )),
            remediation: None,
        };
    }
    let (program, argv) = match config.command.as_deref() {
        Some(c) if !c.trim().is_empty() => {
            let mut parts = vec![c.trim().to_string()];
            parts.extend(config.args.clone());
            (parts.remove(0), parts)
        }
        _ => {
            // Explicit policy: missing provider is either review or skip.
            let missing = vec!["ai:provider".to_string()];
            let mut result = infra_result(
                gate_id,
                config,
                &AiError::MissingProvider {
                    gate_id: gate_id.to_string(),
                },
                missing,
                Some(&request.prompt_digest),
                Some(&request.rule_digest),
            );
            if config.missing_policy == MissingProviderPolicy::NotApplicable {
                result.status = GateStatus::NotApplicable;
                result.severity = GateSeverity::Info;
                result.missing_evidence.clear();
            }
            return result;
        }
    };
    let spec = CheckerSpec {
        name: "ai-provider".to_string(),
        program,
        args: argv,
        working_dir: project_root.to_path_buf(),
        env: BTreeMap::new(),
        timeout: Duration::from_millis(config.timeout_ms.max(1)),
        max_output_bytes: (MAX_AI_RESPONSE_BYTES * 2) as u64,
    };
    // The prompt travels on stdin? The checker runner has no stdin
    // affordance, so the prompt is passed as the final argv element.
    // Providers read `argv[last]` as the request JSON. This keeps the
    // existing bounded runner (capture, timeout, process-group kill)
    // without a new spawn path.
    let mut spec_with_prompt = spec.clone();
    spec_with_prompt.args.push(request.prompt_json.clone());
    let run = run_checker(&spec_with_prompt);
    if let Some(detail) = run.spawn_error.clone() {
        return infra_result(
            gate_id,
            config,
            &AiError::SpawnFailed {
                provider: config.provider.clone(),
                detail,
            },
            vec!["ai:provider".to_string(), "ai:output".to_string()],
            Some(&request.prompt_digest),
            Some(&request.rule_digest),
        );
    }
    if run.timed_out {
        return infra_result(
            gate_id,
            config,
            &AiError::Timeout {
                provider: config.provider.clone(),
                timeout_ms: config.timeout_ms,
            },
            vec!["ai:provider".to_string(), "ai:timeout".to_string()],
            Some(&request.prompt_digest),
            Some(&request.rule_digest),
        );
    }
    if run.signalled {
        return infra_result(
            gate_id,
            config,
            &AiError::Signalled {
                provider: config.provider.clone(),
            },
            vec!["ai:provider".to_string(), "ai:output".to_string()],
            Some(&request.prompt_digest),
            Some(&request.rule_digest),
        );
    }
    // Findings-equivalent: a parseable document decides the status even
    // when the exit code is nonzero; unparsable output falls back to
    // infrastructure mapping so provider crashes stay distinguishable.
    let mut result = validate_output(gate_id, config, &request, run.stdout.as_bytes(), resolve);
    let exit_note = match run.exit_code {
        Some(0) => "provider exit 0".to_string(),
        Some(c) => format!("provider exit {c}"),
        None => "provider no exit code".to_string(),
    };
    let diag = match result.diagnostic.take() {
        Some(d) => format!("{d} | {exit_note}"),
        None => exit_note,
    };
    result.diagnostic = Some(bound_text(&diag, MAX_DIAGNOSTIC_BYTES));
    // A nonzero exit with otherwise-valid PASS output is still review:
    // the provider signalled its own failure.
    if run.exit_code != Some(0) && result.status == GateStatus::Pass {
        result.status = GateStatus::ReviewRequired;
        result.severity = GateSeverity::Warning;
        if !result.missing_evidence.iter().any(|k| k == "ai:output") {
            result.missing_evidence.push("ai:output".to_string());
        }
    }
    result
}

/// Load the optional `[ai]` table from `gate.toml` (project root first,
/// then `.driftwatch/`). Missing file or missing table is `Ok(None)`:
/// AI stays disabled and silent.
pub fn load_ai_config(project_root: &Path) -> Result<Option<AiProviderConfig>, String> {
    let candidates = [
        project_root.join("gate.toml"),
        project_root.join(".driftwatch/gate.toml"),
    ];
    for path in &candidates {
        let Ok(text) = std::fs::read_to_string(path) else {
            continue;
        };
        let value: toml::Value =
            toml::from_str(&text).map_err(|e| format!("gate.toml parse: {e}"))?;
        let Some(ai) = value.get("ai") else {
            continue;
        };
        if ai.as_table().map(|t| t.is_empty()).unwrap_or(false) {
            continue;
        }
        let config: AiProviderConfig = ai
            .clone()
            .try_into()
            .map_err(|e: toml::de::Error| format!("gate.toml [ai]: {e}"))?;
        if !config.enabled && config.command.is_none() {
            return Ok(None);
        }
        return Ok(Some(config));
    }
    Ok(None)
}

/// Doctor readiness for the optional AI layer. Absence stays silent;
/// disabled is honest `Info`; enabled-without-provider is `Warn` with
/// the explicit remediation; a configured command that cannot start is
/// `Warn` (optional provider, never `Fail`).
pub fn ai_checks(project_root: &Path) -> Vec<Check> {
    let config = match load_ai_config(project_root) {
        Ok(Some(c)) => c,
        Ok(None) => return Vec::new(),
        Err(e) => {
            return vec![
                Check::warn("gate.ai.config", "AI provider config is invalid", e).with_remediation(
                    "Fix the `[ai]` table in gate.toml or remove it to keep AI disabled.",
                ),
            ];
        }
    };
    if !config.enabled {
        return vec![Check::info(
            "gate.ai.config",
            "AI evaluation disabled",
            format!(
                "provider {}/{} is configured but not enabled; no provider call is made.",
                config.provider, config.model
            ),
        )
        .with_remediation("Set `[ai] enabled = true` and a `command` to opt in.")];
    }
    let Some(cmd) = config.command.clone() else {
        let (status, detail) = match config.missing_policy {
            MissingProviderPolicy::ReviewRequired => (
                Status::Warn,
                format!(
                    "AI concern enabled for provider {}/{} with no `command`; evaluations report REVIEW_REQUIRED.",
                    config.provider, config.model
                ),
            ),
            MissingProviderPolicy::NotApplicable => (
                Status::Info,
                format!(
                    "AI concern enabled for provider {}/{} with no `command`; evaluations report NOT_APPLICABLE per policy.",
                    config.provider, config.model
                ),
            ),
        };
        let mut check = Check {
            id: "gate.ai.provider",
            name: "AI provider is not configured".to_string(),
            status,
            detail: Some(detail),
            remediation: None,
        };
        check = check.with_remediation(
            "Set `[ai] command` to a local provider emitting the gate-ai JSON schema, or disable `[ai]`.",
        );
        return vec![check];
    };
    // Probe: the command must exist (PATH search or explicit path).
    let available = command_available(&cmd);
    if available {
        vec![Check::pass(
            "gate.ai.provider",
            format!("AI provider `{}` is available", config.provider),
        )
        .with_remediation(format!(
            "model: {}, timeout: {}ms, missing-policy: {}",
            config.model,
            config.timeout_ms,
            config.missing_policy.as_str()
        ))]
    } else {
        vec![
            Check::warn(
                "gate.ai.provider",
                format!("AI provider `{}` is not on PATH", config.provider),
                format!("command: {cmd}"),
            )
            .with_remediation(format!(
                "Install `{cmd}` or fix `[ai] command` in gate.toml; enabled AI without a provider reports REVIEW_REQUIRED."
            )),
        ]
    }
}

fn command_available(program: &str) -> bool {
    let path = PathBuf::from(program);
    if path.components().count() > 1 {
        return path.is_file();
    }
    std::env::var_os("PATH")
        .map(|paths| {
            std::env::split_paths(&paths).any(|dir| {
                dir.join(program).is_file() || dir.join(format!("{program}.exe")).is_file()
            })
        })
        .unwrap_or(false)
}

/// Wrap a validated [`GateResult`] with provider/model identity and
/// prompt/rule digests for evidence-backed persistence.
pub fn record_evaluation(
    gate_id: &str,
    config: &AiProviderConfig,
    request: &AiEvalRequest,
    result: GateResult,
) -> AiEvaluationRecord {
    AiEvaluationRecord {
        gate_id: gate_id.to_string(),
        provider: config.provider.clone(),
        model: config.model.clone(),
        prompt_digest: request.prompt_digest.clone(),
        rule_digest: request.rule_digest.clone(),
        result,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config(enabled: bool, command: Option<&str>) -> AiProviderConfig {
        AiProviderConfig {
            enabled,
            provider: "test-provider".to_string(),
            model: "test-model".to_string(),
            command: command.map(|s| s.to_string()),
            args: Vec::new(),
            timeout_ms: 10_000,
            missing_policy: MissingProviderPolicy::ReviewRequired,
        }
    }

    fn available(key: &str) -> Option<ArtifactRecord> {
        Some(ArtifactRecord {
            key: key.to_string(),
            kind: crate::gate::evidence::ArtifactKind::ToolJson,
            producer: "test".to_string(),
            producer_version: None,
            created_at: "2026-09-15T00:00:00Z".to_string(),
            byte_size: 12,
            digest: "sha256:abc".to_string(),
            media_type: None,
            rel_path: Some("artifacts/x.json".to_string()),
            redacted: true,
            available: true,
            preview: Some("preview".to_string()),
        })
    }

    fn missing(_: &str) -> Option<ArtifactRecord> {
        None
    }

    fn valid_pass_bytes() -> Vec<u8> {
        serde_json::json!({
            "status": "PASS",
            "confidence": 90,
            "rules_checked": ["responsive"],
            "violations": [],
            "evidence": ["ui:screenshot"],
            "missing_evidence": [],
            "reason": "layout matches spec",
            "actions": []
        })
        .to_string()
        .into_bytes()
    }

    #[test]
    fn valid_output_with_evidence_may_pass() {
        let config = test_config(true, Some("true"));
        let request = build_request(&[], &[], &[], &[]);
        let result = validate_output("ux", &config, &request, &valid_pass_bytes(), &available);
        assert_eq!(result.status, GateStatus::Pass);
        assert!(result
            .diagnostic
            .as_deref()
            .unwrap()
            .contains("test-provider"));
    }

    #[test]
    fn pass_without_evidence_is_review() {
        let config = test_config(true, Some("true"));
        let request = build_request(&[], &[], &[], &[]);
        let bytes = serde_json::json!({
            "status": "PASS",
            "confidence": 95,
            "rules_checked": ["responsive"],
            "violations": [],
            "evidence": [],
            "missing_evidence": [],
            "reason": "trust me",
            "actions": []
        })
        .to_string()
        .into_bytes();
        let result = validate_output("ux", &config, &request, &bytes, &available);
        assert_eq!(result.status, GateStatus::ReviewRequired);
        assert!(!result.missing_evidence.is_empty());
    }

    #[test]
    fn pass_with_unavailable_evidence_is_review() {
        let config = test_config(true, Some("true"));
        let request = build_request(&[], &[], &[], &[]);
        let result = validate_output("ux", &config, &request, &valid_pass_bytes(), &missing);
        assert_eq!(result.status, GateStatus::ReviewRequired);
        assert!(result
            .missing_evidence
            .contains(&"ui:screenshot".to_string()));
    }

    #[test]
    fn malformed_output_is_review_never_pass() {
        let config = test_config(true, Some("true"));
        let request = build_request(&[], &[], &[], &[]);
        let result = validate_output("ux", &config, &request, b"not json{{{", &available);
        assert_eq!(result.status, GateStatus::ReviewRequired);
        assert_ne!(result.status, GateStatus::Pass);
        assert!(result
            .missing_evidence
            .iter()
            .any(|k| k.contains("ai:output")));
    }

    #[test]
    fn invalid_status_is_review_never_pass() {
        let config = test_config(true, Some("true"));
        let request = build_request(&[], &[], &[], &[]);
        let bytes = serde_json::json!({
            "status": "APPROVED",
            "confidence": 100,
            "rules_checked": ["r"],
            "violations": [],
            "evidence": ["e"],
            "missing_evidence": [],
            "reason": "x",
            "actions": []
        })
        .to_string()
        .into_bytes();
        let result = validate_output("ux", &config, &request, &bytes, &available);
        assert_eq!(result.status, GateStatus::ReviewRequired);
    }

    #[test]
    fn violations_force_fail() {
        let config = test_config(true, Some("true"));
        let request = build_request(&[], &[], &[], &[]);
        let bytes = serde_json::json!({
            "status": "PASS",
            "confidence": 80,
            "rules_checked": ["a11y"],
            "violations": [{"rule": "a11y", "message": "missing label", "evidence": []}],
            "evidence": ["ui:screenshot"],
            "missing_evidence": [],
            "reason": "found one",
            "actions": ["add label"]
        })
        .to_string()
        .into_bytes();
        let result = validate_output("a11y", &config, &request, &bytes, &available);
        assert_eq!(result.status, GateStatus::Fail);
        assert_eq!(result.findings.len(), 1);
    }

    #[test]
    fn disabled_config_makes_no_provider_call() {
        let config = test_config(false, Some("definitely-missing-binary-xyz"));
        let empty_rules: Vec<AiRule> = Vec::new();
        let empty_evidence: Vec<EvidenceRef> = Vec::new();
        let empty_contexts: Vec<ContextDocument> = Vec::new();
        let empty_sensitive: Vec<String> = Vec::new();
        let result = run_ai_evaluation(
            &AiEvalInput {
                gate_id: "ux",
                rules: &empty_rules,
                evidence: &empty_evidence,
                contexts: &empty_contexts,
                config: &config,
                sensitive: &empty_sensitive,
                project_root: Path::new("/tmp"),
            },
            &missing,
        );
        assert_eq!(result.status, GateStatus::NotApplicable);
        assert!(result
            .diagnostic
            .as_deref()
            .unwrap()
            .contains("no provider call"));
    }

    #[test]
    fn missing_provider_follows_review_policy() {
        let config = test_config(true, None);
        let empty_rules: Vec<AiRule> = Vec::new();
        let empty_evidence: Vec<EvidenceRef> = Vec::new();
        let empty_contexts: Vec<ContextDocument> = Vec::new();
        let empty_sensitive: Vec<String> = Vec::new();
        let result = run_ai_evaluation(
            &AiEvalInput {
                gate_id: "ux",
                rules: &empty_rules,
                evidence: &empty_evidence,
                contexts: &empty_contexts,
                config: &config,
                sensitive: &empty_sensitive,
                project_root: Path::new("/tmp"),
            },
            &missing,
        );
        assert_eq!(result.status, GateStatus::ReviewRequired);
        assert!(result.missing_evidence.contains(&"ai:provider".to_string()));
    }

    #[test]
    fn missing_provider_not_applicable_policy_skips() {
        let mut config = test_config(true, None);
        config.missing_policy = MissingProviderPolicy::NotApplicable;
        let empty_rules: Vec<AiRule> = Vec::new();
        let empty_evidence: Vec<EvidenceRef> = Vec::new();
        let empty_contexts: Vec<ContextDocument> = Vec::new();
        let empty_sensitive: Vec<String> = Vec::new();
        let result = run_ai_evaluation(
            &AiEvalInput {
                gate_id: "ux",
                rules: &empty_rules,
                evidence: &empty_evidence,
                contexts: &empty_contexts,
                config: &config,
                sensitive: &empty_sensitive,
                project_root: Path::new("/tmp"),
            },
            &missing,
        );
        assert_eq!(result.status, GateStatus::NotApplicable);
    }

    #[test]
    fn timeout_records_unavailability() {
        let mut config = test_config(true, Some("sh"));
        config.args = vec!["-c".to_string(), "sleep 30".to_string()];
        config.timeout_ms = 200;
        let empty_rules: Vec<AiRule> = Vec::new();
        let empty_evidence: Vec<EvidenceRef> = Vec::new();
        let empty_contexts: Vec<ContextDocument> = Vec::new();
        let empty_sensitive: Vec<String> = Vec::new();
        let result = run_ai_evaluation(
            &AiEvalInput {
                gate_id: "ux",
                rules: &empty_rules,
                evidence: &empty_evidence,
                contexts: &empty_contexts,
                config: &config,
                sensitive: &empty_sensitive,
                project_root: Path::new("/tmp"),
            },
            &missing,
        );
        assert_eq!(result.status, GateStatus::ReviewRequired);
        assert!(result.diagnostic.as_deref().unwrap().contains("timeout"));
    }

    #[test]
    fn secrets_redacted_before_submission() {
        let rules = vec![AiRule {
            id: "privacy".to_string(),
            text: "check password = hunter2 handling".to_string(),
        }];
        let sensitive = vec!["hunter2".to_string()];
        let request = build_request(&rules, &[], &[], &sensitive);
        assert!(
            !request.prompt_json.contains("hunter2"),
            "secret leaked: {}",
            request.prompt_json
        );
        assert!(!request.prompt_digest.is_empty());
        assert!(!request.rule_digest.is_empty());
    }

    #[test]
    fn persisted_output_contains_no_secrets() {
        let config = test_config(true, Some("true"));
        let request = build_request(&[], &[], &[], &[]);
        let bytes = serde_json::json!({
            "status": "FAIL",
            "confidence": 70,
            "rules_checked": ["privacy"],
            "violations": [],
            "evidence": [],
            "missing_evidence": ["ui:screenshot"],
            "reason": "password = hunter2 visible",
            "actions": []
        })
        .to_string()
        .into_bytes();
        let result = validate_output("privacy", &config, &request, &bytes, &available);
        let debug = format!("{result:?}");
        assert!(!debug.contains("hunter2"), "secret leaked: {debug}");
    }

    #[test]
    fn absent_ai_table_is_silent() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("gate.toml"), "profile = \"minimal\"\n").unwrap();
        let checks = ai_checks(tmp.path());
        assert!(checks.is_empty());
    }

    #[test]
    fn enabled_without_command_warns() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("gate.toml"),
            "profile = \"minimal\"\n[ai]\nenabled = true\n",
        )
        .unwrap();
        let checks = ai_checks(tmp.path());
        assert_eq!(checks.len(), 1);
        assert_eq!(checks[0].id, "gate.ai.provider");
        assert_eq!(checks[0].status, Status::Warn);
    }
}
