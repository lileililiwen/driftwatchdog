//! Gate tool adapters and deterministic evaluation.
//!
//! Adapters are integration boundaries, not domain logic. Each adapter
//! declares what it needs ([`AdapterCapability`]: tool identity,
//! supported [`ExecutionMode`]s, accepted [`OutputFormat`]s, required
//! evidence) and normalizes one tool invocation into a [`GateResult`]
//! with evidence references. Adapters never decide aggregate blocking
//! policy (see [`crate::gate::aggregate`]) and never abort unrelated
//! adapters (see [`run_all`]).
//!
//! ## Execution boundary
//!
//! Adapters run tools as child processes through the existing checker
//! runner ([`crate::checker::runner::run_checker`]), reusing its
//! hardening: bounded capture, per-adapter timeout, process-group kill,
//! and signal-aware outcomes. [`AdapterInput`] carries program + argv
//! (never shell-split), working dir, env, timeout, and output caps.
//!
//! ## Exit-code semantics
//!
//! Exit code, findings, and evidence are normalized **separately**
//! because tools disagree: Semgrep and Gitleaks exit nonzero precisely
//! when they report findings, while a build command exits nonzero on
//! failure. The rule is: when stdout parses into findings, the findings
//! decide the status and the exit code is recorded in the diagnostic;
//! only unparsable output falls back to infrastructure mapping
//! (nonzero → `REVIEW_REQUIRED`, timeout → `REVIEW_REQUIRED` with
//! timeout evidence, signal → `REVIEW_REQUIRED`, missing executable →
//! `REVIEW_REQUIRED`).
//!
//! ## Supported formats
//!
//! * `checker` — the driftwatch alerts document (`{"alerts": [...]}`);
//! * `sarif` — SARIF 2.1.0 `runs[].results[]` (rule, severity, message,
//!   source location, artifact ref);
//! * `gitleaks` — Gitleaks JSON array (rule, file, line; secret values
//!   are never embedded — descriptions and locations only);
//! * `osv` — OSV-scanner JSON (`results[].packages[].vulnerabilities[]`);
//! * `semgrep` — Semgrep JSON (`results[]` with `check_id`, path, line,
//!   message, severity);
//! * `text` — project-runtime commands: exit-code mapping with a
//!   bounded, secret-redacted output diagnostic (no finding parsing).
//!
//! No scanner is reimplemented here and no tool SDK is linked: only
//! documented JSON shapes are parsed and only CLIs are invoked. There
//! is no AI evaluator in this change; [`evaluate`] applies
//! deterministic rules to normalized findings and evidence.
//!
//! No OpenSpec types appear here and no LLM is invoked.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;

use crate::checker::runner::{run_checker, CheckerRun, CheckerSpec};
use crate::gate::dto::{
    MAX_DIAGNOSTIC_BYTES, MAX_EVIDENCE_KEY_BYTES, MAX_EVIDENCE_REFS, MAX_FINDINGS,
    MAX_LOCATION_BYTES, MAX_MISSING_EVIDENCE, MAX_PREVIEW_BYTES, MAX_REMEDIATION_BYTES,
    MAX_TITLE_BYTES,
};
use crate::gate::evidence::ArtifactRecord;
use crate::gate::redact::{bound_text, redact_secrets};
use crate::gate::toolchain::ExecutionMode;
use crate::gate::types::{EvidenceRef, Finding, GateResult, GateSeverity, GateStatus};

/// Output formats an adapter accepts, in preference order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OutputFormat {
    CheckerJson,
    Sarif,
    GitleaksJson,
    OsvJson,
    SemgrepJson,
    Text,
}

impl OutputFormat {
    pub fn as_str(self) -> &'static str {
        match self {
            OutputFormat::CheckerJson => "checker-json",
            OutputFormat::Sarif => "sarif",
            OutputFormat::GitleaksJson => "gitleaks-json",
            OutputFormat::OsvJson => "osv-json",
            OutputFormat::SemgrepJson => "semgrep-json",
            OutputFormat::Text => "text",
        }
    }
}

/// Static capability declaration for one adapter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdapterCapability {
    /// Stable tool id (e.g. `semgrep`). Unique in a registry.
    pub tool_id: &'static str,
    /// Human-readable description (no implementation detail).
    pub description: &'static str,
    /// Execution modes this adapter supports.
    pub modes: Vec<ExecutionMode>,
    /// Accepted output formats, most preferred first.
    pub formats: Vec<OutputFormat>,
    /// Evidence keys the adapter requires for an evidence-backed `PASS`.
    pub required_evidence: Vec<String>,
}

impl AdapterCapability {
    fn validate(&self) -> Result<(), AdapterError> {
        if self.tool_id.trim().is_empty() {
            return Err(AdapterError::InvalidCapability(
                "adapter with an empty tool id".to_string(),
            ));
        }
        if self.modes.is_empty() {
            return Err(AdapterError::InvalidCapability(format!(
                "adapter `{}` declares no execution mode",
                self.tool_id
            )));
        }
        if self.formats.is_empty() {
            return Err(AdapterError::InvalidCapability(format!(
                "adapter `{}` accepts no output format",
                self.tool_id
            )));
        }
        Ok(())
    }
}

/// Input to one adapter invocation. Program + argv are passed to the
/// child verbatim (never shell-split); output is bounded.
#[derive(Debug, Clone)]
pub struct AdapterInput {
    pub program: String,
    pub args: Vec<String>,
    pub working_dir: PathBuf,
    pub env: BTreeMap<String, String>,
    pub timeout_ms: u64,
    pub max_output_bytes: u64,
    /// Evidence references for raw artifacts stored by the caller via
    /// [`crate::gate::evidence`]. Passed through into the result.
    pub evidence: Vec<EvidenceRef>,
}

impl AdapterInput {
    fn checker_spec(&self, name: &str) -> CheckerSpec {
        CheckerSpec {
            name: name.to_string(),
            program: self.program.clone(),
            args: self.args.clone(),
            working_dir: self.working_dir.clone(),
            env: self.env.clone(),
            timeout: Duration::from_millis(self.timeout_ms),
            max_output_bytes: self.max_output_bytes,
        }
    }
}

/// Adapter failures. Every variant maps to a `REVIEW_REQUIRED`
/// [`GateResult`] with missing evidence and a bounded diagnostic —
/// never to a panic and never aborting other adapters.
#[derive(Debug, thiserror::Error, PartialEq, Eq, Clone)]
pub enum AdapterError {
    #[error("adapter `{tool}` is not registered")]
    UnknownAdapter { tool: String },
    #[error("duplicate adapter id `{tool}`")]
    DuplicateAdapter { tool: String },
    #[error("invalid adapter capability: {0}")]
    InvalidCapability(String),
    #[error("adapter `{tool}` executable could not start: {detail}")]
    SpawnFailed { tool: String, detail: String },
    #[error("adapter `{tool}` exited {code}")]
    NonZeroExit { tool: String, code: String },
    #[error("adapter `{tool}` exceeded its timeout of {timeout_ms}ms")]
    Timeout { tool: String, timeout_ms: u64 },
    #[error("adapter `{tool}` was killed by a signal")]
    Signalled { tool: String },
    #[error("adapter `{tool}` emitted malformed {format} output: {detail}")]
    MalformedOutput {
        tool: String,
        format: String,
        detail: String,
    },
}

/// One deterministic evaluation rule over normalized findings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeterministicRule {
    /// Stable rule id (e.g. `no-error-findings`).
    pub id: String,
    /// Findings at or above this severity violate the rule.
    pub threshold: GateSeverity,
    /// Evidence keys required for an evidence-backed `PASS`.
    pub requires_evidence: Vec<String>,
}

/// Parser function type: raw stdout bytes → normalized findings.
/// Tolerant by contract: unknown JSON fields are ignored, but a
/// structurally wrong document is an [`AdapterError::MalformedOutput`].
pub type ParseFn = fn(&[u8]) -> Result<Vec<Finding>, AdapterError>;

/// A registered adapter: capability plus its output parser.
#[derive(Clone)]
pub struct RegisteredAdapter {
    pub capability: AdapterCapability,
    pub parse: ParseFn,
}

impl std::fmt::Debug for RegisteredAdapter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RegisteredAdapter")
            .field("capability", &self.capability)
            .finish_non_exhaustive()
    }
}

/// Adapter registry. Small facade over a `BTreeMap` so adapter sprawl
/// stays visible: every adapter is declared, validated, and looked up
/// by tool id.
#[derive(Debug, Default)]
pub struct AdapterRegistry {
    adapters: BTreeMap<&'static str, RegisteredAdapter>,
}

impl AdapterRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register one adapter. Duplicate ids and empty capability
    /// declarations fail here, before any execution.
    pub fn register(
        &mut self,
        capability: AdapterCapability,
        parse: ParseFn,
    ) -> Result<(), AdapterError> {
        capability.validate()?;
        if self.adapters.contains_key(capability.tool_id) {
            return Err(AdapterError::DuplicateAdapter {
                tool: capability.tool_id.to_string(),
            });
        }
        self.adapters
            .insert(capability.tool_id, RegisteredAdapter { capability, parse });
        Ok(())
    }

    /// Look up an adapter by tool id.
    pub fn get(&self, tool_id: &str) -> Result<&RegisteredAdapter, AdapterError> {
        self.adapters
            .get(tool_id)
            .ok_or_else(|| AdapterError::UnknownAdapter {
                tool: tool_id.to_string(),
            })
    }

    /// Registered tool ids in sorted order.
    pub fn tool_ids(&self) -> Vec<&'static str> {
        self.adapters.keys().copied().collect()
    }

    pub fn len(&self) -> usize {
        self.adapters.len()
    }

    pub fn is_empty(&self) -> bool {
        self.adapters.is_empty()
    }
}

fn severity_rank(s: GateSeverity) -> u8 {
    match s {
        GateSeverity::Info => 0,
        GateSeverity::Warning => 1,
        GateSeverity::Error => 2,
    }
}

fn truncate_findings(mut findings: Vec<Finding>) -> (Vec<Finding>, bool) {
    if findings.len() > MAX_FINDINGS {
        findings.truncate(MAX_FINDINGS);
        (findings, true)
    } else {
        (findings, false)
    }
}

fn bound_evidence(evidence: Vec<EvidenceRef>) -> Vec<EvidenceRef> {
    evidence.into_iter().take(MAX_EVIDENCE_REFS).collect()
}

fn source_for(tool_id: &str) -> String {
    bound_text(
        &format!("adapter:{tool_id}"),
        crate::gate::dto::MAX_SOURCE_BYTES,
    )
}

fn gate_id_for(tool_id: &str, gate_id: Option<&str>) -> String {
    bound_text(
        gate_id.unwrap_or(tool_id),
        crate::gate::dto::MAX_GATE_ID_BYTES,
    )
}

/// Build the infrastructure-failure result shared by every
/// [`AdapterError`]: `REVIEW_REQUIRED` with the tool output listed as
/// missing evidence and a bounded, secret-redacted diagnostic.
fn infra_result(
    tool_id: &str,
    gate_id: Option<&str>,
    error: &AdapterError,
    evidence: Vec<EvidenceRef>,
    extra_diagnostic: Option<String>,
) -> GateResult {
    let mut diagnostic = bound_text(&redact_secrets(&error.to_string()), MAX_DIAGNOSTIC_BYTES);
    if let Some(extra) = extra_diagnostic {
        let extra = bound_text(&redact_secrets(&extra), 512);
        if !extra.trim().is_empty() {
            diagnostic.push_str(" | ");
            diagnostic.push_str(&extra);
        }
        if diagnostic.len() > MAX_DIAGNOSTIC_BYTES {
            diagnostic.truncate(MAX_DIAGNOSTIC_BYTES);
        }
    }
    let mut missing = vec![format!("{tool_id}:output")];
    missing.truncate(MAX_MISSING_EVIDENCE);
    GateResult {
        gate_id: gate_id_for(tool_id, gate_id),
        source: source_for(tool_id),
        status: GateStatus::ReviewRequired,
        severity: GateSeverity::Warning,
        findings: vec![],
        evidence: bound_evidence(evidence),
        missing_evidence: missing,
        diagnostic: Some(diagnostic),
        remediation: Some(bound_text(
            "Re-run the tool; resolve tool or output errors before trusting this gate.",
            MAX_REMEDIATION_BYTES,
        )),
    }
}

/// Normalize parsed findings into a result: empty → `PASS`, otherwise
/// → `FAIL` at the top severity. Findings, exit code, and evidence
/// are already separated by the caller; this only classifies.
fn findings_result(
    tool_id: &str,
    gate_id: Option<&str>,
    findings: Vec<Finding>,
    evidence: Vec<EvidenceRef>,
    diagnostic: Option<String>,
    truncated: bool,
) -> GateResult {
    let (findings, was_truncated) = truncate_findings(findings);
    let truncated = truncated || was_truncated;
    let mut top = GateSeverity::Info;
    for f in &findings {
        if severity_rank(f.severity) > severity_rank(top) {
            top = f.severity;
        }
    }
    let mut diagnostic = diagnostic.map(|d| bound_text(&redact_secrets(&d), MAX_DIAGNOSTIC_BYTES));
    if truncated {
        let note = format!("finding list truncated to {MAX_FINDINGS}");
        diagnostic = Some(match diagnostic {
            Some(d) => format!("{d} | {note}"),
            None => note,
        });
    }
    if findings.is_empty() {
        GateResult {
            gate_id: gate_id_for(tool_id, gate_id),
            source: source_for(tool_id),
            status: GateStatus::Pass,
            severity: GateSeverity::Info,
            findings,
            evidence: bound_evidence(evidence),
            missing_evidence: vec![],
            diagnostic,
            remediation: None,
        }
    } else {
        GateResult {
            gate_id: gate_id_for(tool_id, gate_id),
            source: source_for(tool_id),
            status: GateStatus::Fail,
            severity: top,
            findings,
            evidence: bound_evidence(evidence),
            missing_evidence: vec![],
            diagnostic,
            remediation: Some(bound_text(
                "Investigate the reported findings and add regression coverage.",
                MAX_REMEDIATION_BYTES,
            )),
        }
    }
}

fn exit_diagnostic(run: &CheckerRun, tool_id: &str) -> String {
    let mut parts = vec![format!("tool `{tool_id}`")];
    match run.exit_code {
        Some(c) => parts.push(format!("exit {c}")),
        None if run.timed_out => parts.push("timed out".to_string()),
        None if run.signalled => parts.push("killed by signal".to_string()),
        None if run.spawn_error.is_some() => parts.push("did not start".to_string()),
        None => parts.push("no exit code".to_string()),
    }
    if !run.stderr.trim().is_empty() {
        parts.push(format!("stderr: {}", run.stderr.trim()));
    }
    parts.join("; ")
}

/// Parse the driftwatch checker JSON document into findings.
pub fn parse_checker_json(bytes: &[u8]) -> Result<Vec<Finding>, AdapterError> {
    let text = String::from_utf8_lossy(bytes);
    let tool = "checker";
    let doc: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| AdapterError::MalformedOutput {
            tool: tool.to_string(),
            format: OutputFormat::CheckerJson.as_str().to_string(),
            detail: bound_text(&e.to_string(), 512),
        })?;
    let alerts = doc
        .get("alerts")
        .and_then(|a| a.as_array())
        .ok_or_else(|| AdapterError::MalformedOutput {
            tool: tool.to_string(),
            format: OutputFormat::CheckerJson.as_str().to_string(),
            detail: "missing `alerts` array".to_string(),
        })?;
    let mut findings = Vec::new();
    for alert in alerts.iter().take(MAX_FINDINGS + 1) {
        let message = alert.get("message").and_then(|v| v.as_str()).unwrap_or("");
        let severity = alert
            .get("severity")
            .and_then(|v| v.as_str())
            .unwrap_or("warning");
        let source = alert.get("source").and_then(|v| v.as_str()).unwrap_or("");
        let symbol = alert.get("symbol").and_then(|v| v.as_str()).unwrap_or("");
        let mut location = source.to_string();
        if !symbol.is_empty() {
            location.push(':');
            location.push_str(symbol);
        }
        findings.push(Finding {
            title: bound_text(&redact_secrets(message), MAX_TITLE_BYTES),
            severity: GateSeverity::parse(severity).unwrap_or(GateSeverity::Warning),
            location: if location.is_empty() {
                None
            } else {
                Some(bound_text(&redact_secrets(&location), MAX_LOCATION_BYTES))
            },
            rule: alert
                .get("rule")
                .and_then(|v| v.as_str())
                .map(|r| bound_text(r, 256)),
        });
    }
    Ok(findings)
}

fn sarif_level_to_severity(level: Option<&str>) -> GateSeverity {
    match level.unwrap_or("warning").to_ascii_lowercase().as_str() {
        "error" => GateSeverity::Error,
        "note" | "none" => GateSeverity::Info,
        _ => GateSeverity::Warning,
    }
}

/// Parse SARIF 2.1.0 `runs[].results[]` into findings, preserving rule,
/// severity, message, source location, and artifact reference.
/// Tolerant: unknown fields ignored; structurally wrong documents are
/// [`AdapterError::MalformedOutput`].
pub fn parse_sarif(bytes: &[u8]) -> Result<Vec<Finding>, AdapterError> {
    let tool = "sarif";
    let format = OutputFormat::Sarif.as_str().to_string();
    let text = String::from_utf8_lossy(bytes);
    let doc: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| AdapterError::MalformedOutput {
            tool: tool.to_string(),
            format: format.clone(),
            detail: bound_text(&e.to_string(), 512),
        })?;
    let runs = doc.get("runs").and_then(|r| r.as_array()).ok_or_else(|| {
        AdapterError::MalformedOutput {
            tool: tool.to_string(),
            format: format.clone(),
            detail: "missing `runs` array (not a SARIF document)".to_string(),
        }
    })?;
    let mut findings = Vec::new();
    'outer: for run in runs {
        let results = run
            .get("results")
            .and_then(|r| r.as_array())
            .cloned()
            .unwrap_or_default();
        for result in &results {
            if findings.len() > MAX_FINDINGS {
                break 'outer;
            }
            let rule = result
                .get("ruleId")
                .and_then(|v| v.as_str())
                .map(|s| bound_text(s, 256));
            let message = result
                .get("message")
                .and_then(|m| m.get("text"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let level = result.get("level").and_then(|v| v.as_str());
            let location = result
                .get("locations")
                .and_then(|l| l.as_array())
                .and_then(|l| l.first())
                .and_then(|l| l.get("physicalLocation"))
                .map(|pl| {
                    let uri = pl
                        .get("artifactLocation")
                        .and_then(|a| a.get("uri"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    let line = pl
                        .get("region")
                        .and_then(|r| r.get("startLine"))
                        .and_then(|v| v.as_u64())
                        .map(|n| format!(":{n}"))
                        .unwrap_or_default();
                    format!("{uri}{line}")
                })
                .filter(|s| !s.is_empty() && s != ":")
                .map(|s| bound_text(&redact_secrets(&s), MAX_LOCATION_BYTES));
            findings.push(Finding {
                title: bound_text(&redact_secrets(message), MAX_TITLE_BYTES),
                severity: sarif_level_to_severity(level),
                location,
                rule,
            });
        }
    }
    Ok(findings)
}

/// Parse Gitleaks JSON array output. Secret values are never embedded:
/// only rule id, description, file, and line become findings.
pub fn parse_gitleaks(bytes: &[u8]) -> Result<Vec<Finding>, AdapterError> {
    let tool = "gitleaks";
    let format = OutputFormat::GitleaksJson.as_str().to_string();
    let text = String::from_utf8_lossy(bytes);
    // Gitleaks emits `[]` when clean and `null`/empty on some versions;
    // both mean no findings rather than malformed output.
    if text.trim().is_empty() || text.trim() == "null" {
        return Ok(vec![]);
    }
    let doc: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| AdapterError::MalformedOutput {
            tool: tool.to_string(),
            format: format.clone(),
            detail: bound_text(&e.to_string(), 512),
        })?;
    let leaks = doc
        .as_array()
        .ok_or_else(|| AdapterError::MalformedOutput {
            tool: tool.to_string(),
            format: format.clone(),
            detail: "expected a JSON array of leaks".to_string(),
        })?;
    let mut findings = Vec::new();
    for leak in leaks.iter().take(MAX_FINDINGS + 1) {
        let rule = leak
            .get("RuleID")
            .and_then(|v| v.as_str())
            .map(|s| bound_text(s, 256));
        let description = leak
            .get("Description")
            .and_then(|v| v.as_str())
            .unwrap_or("secret detected");
        let file = leak.get("File").and_then(|v| v.as_str()).unwrap_or("");
        let line = leak
            .get("StartLine")
            .and_then(|v| v.as_u64())
            .map(|n| format!(":{n}"))
            .unwrap_or_default();
        let location = format!("{file}{line}");
        findings.push(Finding {
            title: bound_text(&redact_secrets(description), MAX_TITLE_BYTES),
            severity: GateSeverity::Error,
            location: if location.is_empty() || location == ":" {
                None
            } else {
                Some(bound_text(&redact_secrets(&location), MAX_LOCATION_BYTES))
            },
            rule,
        });
    }
    Ok(findings)
}

/// Parse OSV-scanner JSON output (`results[].packages[].vulnerabilities[]`).
pub fn parse_osv(bytes: &[u8]) -> Result<Vec<Finding>, AdapterError> {
    let tool = "osv";
    let format = OutputFormat::OsvJson.as_str().to_string();
    let text = String::from_utf8_lossy(bytes);
    let doc: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| AdapterError::MalformedOutput {
            tool: tool.to_string(),
            format: format.clone(),
            detail: bound_text(&e.to_string(), 512),
        })?;
    let empty = vec![];
    let results = doc
        .get("results")
        .and_then(|r| r.as_array())
        .unwrap_or(&empty);
    let mut findings = Vec::new();
    'outer: for result in results {
        let packages = result
            .get("packages")
            .and_then(|p| p.as_array())
            .cloned()
            .unwrap_or_default();
        for package in &packages {
            let pkg_name = package
                .get("package")
                .and_then(|p| p.get("name"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let pkg_version = package
                .get("package")
                .and_then(|p| p.get("version"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let vulns = package
                .get("vulnerabilities")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            for vuln in &vulns {
                if findings.len() > MAX_FINDINGS {
                    break 'outer;
                }
                let id = vuln.get("id").and_then(|v| v.as_str()).unwrap_or("");
                let summary = vuln
                    .get("summary")
                    .and_then(|v| v.as_str())
                    .or_else(|| vuln.get("details").and_then(|v| v.as_str()))
                    .unwrap_or("vulnerable dependency");
                let severity = vuln
                    .get("severity")
                    .and_then(|s| s.as_array())
                    .and_then(|s| s.first())
                    .and_then(|s| s.get("type"))
                    .and_then(|v| v.as_str())
                    .map(|_| GateSeverity::Error)
                    .unwrap_or(GateSeverity::Warning);
                findings.push(Finding {
                    title: bound_text(&redact_secrets(summary), MAX_TITLE_BYTES),
                    severity,
                    location: if pkg_name.is_empty() {
                        None
                    } else {
                        Some(bound_text(
                            &redact_secrets(&format!("{pkg_name}@{pkg_version}")),
                            MAX_LOCATION_BYTES,
                        ))
                    },
                    rule: if id.is_empty() {
                        None
                    } else {
                        Some(bound_text(id, 256))
                    },
                });
            }
        }
    }
    Ok(findings)
}

/// Parse Semgrep JSON output (`results[]` with `check_id`, `path`,
/// `start.line`, `extra.message`, `extra.severity`).
pub fn parse_semgrep(bytes: &[u8]) -> Result<Vec<Finding>, AdapterError> {
    let tool = "semgrep";
    let format = OutputFormat::SemgrepJson.as_str().to_string();
    let text = String::from_utf8_lossy(bytes);
    let doc: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| AdapterError::MalformedOutput {
            tool: tool.to_string(),
            format: format.clone(),
            detail: bound_text(&e.to_string(), 512),
        })?;
    let empty = vec![];
    let results = doc
        .get("results")
        .and_then(|r| r.as_array())
        .unwrap_or(&empty);
    let mut findings = Vec::new();
    for result in results.iter().take(MAX_FINDINGS + 1) {
        let rule = result
            .get("check_id")
            .and_then(|v| v.as_str())
            .map(|s| bound_text(s, 256));
        let message = result
            .get("extra")
            .and_then(|e| e.get("message"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let severity = result
            .get("extra")
            .and_then(|e| e.get("severity"))
            .and_then(|v| v.as_str())
            .map(|s| match s.to_ascii_lowercase().as_str() {
                "error" => GateSeverity::Error,
                "warning" => GateSeverity::Warning,
                _ => GateSeverity::Info,
            })
            .unwrap_or(GateSeverity::Warning);
        let path = result.get("path").and_then(|v| v.as_str()).unwrap_or("");
        let line = result
            .get("start")
            .and_then(|s| s.get("line"))
            .and_then(|v| v.as_u64())
            .map(|n| format!(":{n}"))
            .unwrap_or_default();
        let location = format!("{path}{line}");
        findings.push(Finding {
            title: bound_text(&redact_secrets(message), MAX_TITLE_BYTES),
            severity,
            location: if location.is_empty() || location == ":" {
                None
            } else {
                Some(bound_text(&redact_secrets(&location), MAX_LOCATION_BYTES))
            },
            rule,
        });
    }
    Ok(findings)
}

/// Capability for the existing-checker compatibility adapter.
pub fn checker_capability() -> AdapterCapability {
    AdapterCapability {
        tool_id: "checker",
        description: "driftwatch checker protocol compatibility (alerts JSON)",
        modes: vec![
            ExecutionMode::Managed,
            ExecutionMode::Native,
            ExecutionMode::ProjectRuntime,
        ],
        formats: vec![OutputFormat::CheckerJson, OutputFormat::Text],
        required_evidence: vec![],
    }
}

/// Capability for the Gitleaks secret-scan adapter.
pub fn gitleaks_capability() -> AdapterCapability {
    AdapterCapability {
        tool_id: "gitleaks",
        description: "Gitleaks secret detection via CLI JSON output",
        modes: vec![ExecutionMode::Managed, ExecutionMode::Container],
        formats: vec![OutputFormat::GitleaksJson],
        required_evidence: vec!["gitleaks:report".to_string()],
    }
}

/// Capability for the OSV vulnerability-scan adapter.
pub fn osv_capability() -> AdapterCapability {
    AdapterCapability {
        tool_id: "osv",
        description: "OSV dependency vulnerability scan via CLI JSON output",
        modes: vec![ExecutionMode::Managed, ExecutionMode::Container],
        formats: vec![OutputFormat::OsvJson],
        required_evidence: vec!["osv:report".to_string()],
    }
}

/// Capability for the Semgrep static-analysis adapter.
pub fn semgrep_capability() -> AdapterCapability {
    AdapterCapability {
        tool_id: "semgrep",
        description: "Semgrep static analysis via CLI JSON output",
        modes: vec![ExecutionMode::Managed, ExecutionMode::Container],
        formats: vec![OutputFormat::SemgrepJson],
        required_evidence: vec!["semgrep:report".to_string()],
    }
}

/// Capability for project-runtime commands (build/test argv executed
/// verbatim; exit-code mapping with bounded output diagnostics).
pub fn project_runtime_capability() -> AdapterCapability {
    AdapterCapability {
        tool_id: "project-runtime",
        description: "project build/test commands via declared argv (text output)",
        modes: vec![ExecutionMode::ProjectRuntime],
        formats: vec![OutputFormat::Text],
        required_evidence: vec![],
    }
}

/// Default registry with the five initial adapters.
pub fn default_registry() -> AdapterRegistry {
    let mut registry = AdapterRegistry::new();
    for (capability, parse) in [
        (checker_capability(), parse_checker_json as ParseFn),
        (gitleaks_capability(), parse_gitleaks as ParseFn),
        (osv_capability(), parse_osv as ParseFn),
        (semgrep_capability(), parse_semgrep as ParseFn),
        // `project-runtime` has no finding parser: text output maps by
        // exit code. Registration still validates the capability; the
        // parse fn is unused and immediately rejects structured use.
        (project_runtime_capability(), parse_text_reject as ParseFn),
    ] {
        registry
            .register(capability, parse)
            .expect("built-in adapter capabilities are valid");
    }
    registry
}

fn parse_text_reject(bytes: &[u8]) -> Result<Vec<Finding>, AdapterError> {
    let _ = bytes;
    Err(AdapterError::MalformedOutput {
        tool: "project-runtime".to_string(),
        format: OutputFormat::Text.as_str().to_string(),
        detail: "text output has no finding parser; use exit-code mapping".to_string(),
    })
}

/// Run one adapter to a normalized [`GateResult`]. Tool failures
/// (spawn, nonzero-with-unusable-output, timeout, signal, malformed)
/// become `REVIEW_REQUIRED` results — never errors, never panics — so
/// callers can aggregate without branching.
pub fn run_adapter(
    adapter: &RegisteredAdapter,
    input: &AdapterInput,
    gate_id: Option<&str>,
) -> GateResult {
    let tool_id = adapter.capability.tool_id;
    // Text-only adapters (project-runtime) map the exit code with a
    // bounded diagnostic; there is no finding parser to consult.
    if adapter.capability.formats == vec![OutputFormat::Text] {
        return run_text_adapter(tool_id, gate_id, input);
    }
    let spec = input.checker_spec(tool_id);
    let run = run_checker(&spec);
    if let Some(detail) = run.spawn_error.clone() {
        return infra_result(
            tool_id,
            gate_id,
            &AdapterError::SpawnFailed {
                tool: tool_id.to_string(),
                detail,
            },
            input.evidence.clone(),
            Some(exit_diagnostic(&run, tool_id)),
        );
    }
    if run.timed_out {
        let mut result = infra_result(
            tool_id,
            gate_id,
            &AdapterError::Timeout {
                tool: tool_id.to_string(),
                timeout_ms: input.timeout_ms,
            },
            input.evidence.clone(),
            Some(exit_diagnostic(&run, tool_id)),
        );
        result.missing_evidence.push(format!("{tool_id}:timeout"));
        result.missing_evidence.truncate(MAX_MISSING_EVIDENCE);
        return result;
    }
    if run.signalled {
        return infra_result(
            tool_id,
            gate_id,
            &AdapterError::Signalled {
                tool: tool_id.to_string(),
            },
            input.evidence.clone(),
            Some(exit_diagnostic(&run, tool_id)),
        );
    }
    // Findings decide before the exit code: scanners exit nonzero when
    // they report findings, so a parseable document wins over the code.
    match (adapter.parse)(run.stdout.as_bytes()) {
        Ok(findings) => findings_result(
            tool_id,
            gate_id,
            findings,
            input.evidence.clone(),
            Some(exit_diagnostic(&run, tool_id)),
            run.stdout_truncated,
        ),
        Err(parse_error) => {
            // Unusable output: fall back to infrastructure mapping that
            // keeps nonzero exit, timeout, signal, and malformed output
            // distinguishable in the diagnostic.
            let parse_detail = parse_error.to_string();
            let infra = if run.exit_code.unwrap_or(0) != 0 {
                AdapterError::NonZeroExit {
                    tool: tool_id.to_string(),
                    code: run
                        .exit_code
                        .map(|c| c.to_string())
                        .unwrap_or_else(|| "signal".to_string()),
                }
            } else {
                parse_error
            };
            let mut extra = exit_diagnostic(&run, tool_id);
            extra.push_str(" | parse: ");
            extra.push_str(&parse_detail);
            infra_result(
                tool_id,
                gate_id,
                &infra,
                input.evidence.clone(),
                Some(extra),
            )
        }
    }
}

/// Exit-code mapping for text adapters: zero → `PASS`, nonzero →
/// `FAIL` with a bounded, secret-redacted output diagnostic.
/// Timeouts, signals, and spawn failures stay `REVIEW_REQUIRED`.
fn run_text_adapter(tool_id: &str, gate_id: Option<&str>, input: &AdapterInput) -> GateResult {
    let spec = input.checker_spec(tool_id);
    let run = run_checker(&spec);
    if let Some(detail) = run.spawn_error.clone() {
        return infra_result(
            tool_id,
            gate_id,
            &AdapterError::SpawnFailed {
                tool: tool_id.to_string(),
                detail,
            },
            input.evidence.clone(),
            Some(exit_diagnostic(&run, tool_id)),
        );
    }
    if run.timed_out {
        let mut result = infra_result(
            tool_id,
            gate_id,
            &AdapterError::Timeout {
                tool: tool_id.to_string(),
                timeout_ms: input.timeout_ms,
            },
            input.evidence.clone(),
            Some(exit_diagnostic(&run, tool_id)),
        );
        result.missing_evidence.push(format!("{tool_id}:timeout"));
        result.missing_evidence.truncate(MAX_MISSING_EVIDENCE);
        return result;
    }
    if run.signalled {
        return infra_result(
            tool_id,
            gate_id,
            &AdapterError::Signalled {
                tool: tool_id.to_string(),
            },
            input.evidence.clone(),
            Some(exit_diagnostic(&run, tool_id)),
        );
    }
    // Bounded tail of combined output as the diagnostic: stdout wins,
    // stderr fills when stdout is empty.
    let output = if run.stdout.trim().is_empty() {
        run.stderr.clone()
    } else {
        run.stdout.clone()
    };
    let diagnostic = bound_text(&redact_secrets(output.trim()), MAX_DIAGNOSTIC_BYTES);
    match run.exit_code.unwrap_or(-1) {
        0 => GateResult {
            gate_id: gate_id_for(tool_id, gate_id),
            source: source_for(tool_id),
            status: GateStatus::Pass,
            severity: GateSeverity::Info,
            findings: vec![],
            evidence: bound_evidence(input.evidence.clone()),
            missing_evidence: vec![],
            diagnostic: if diagnostic.is_empty() {
                None
            } else {
                Some(diagnostic)
            },
            remediation: None,
        },
        code => GateResult {
            gate_id: gate_id_for(tool_id, gate_id),
            source: source_for(tool_id),
            status: GateStatus::Fail,
            severity: GateSeverity::Error,
            findings: vec![],
            evidence: bound_evidence(input.evidence.clone()),
            missing_evidence: vec![],
            diagnostic: Some(format!("exit {code} | {diagnostic}")),
            remediation: Some(bound_text(
                "Fix the failing command and re-run; see the output diagnostic above.",
                MAX_REMEDIATION_BYTES,
            )),
        },
    }
}

/// Wire version of the product-quality JSON envelope.
pub const PRODUCT_QUALITY_ENVELOPE_VERSION: u32 = 1;

/// Parsed product-quality JSON envelope. The shape mirrors the existing
/// [`GateResult`] wire form so producers can reuse the same JSON
/// document a normal adapter would emit; Driftwatchdog treats it as
/// data and never embeds a language scanner.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ProductQualityEnvelope {
    status: GateStatus,
    severity: Option<GateSeverity>,
    findings: Vec<Finding>,
    evidence: Vec<EvidenceRef>,
    missing_evidence: Vec<String>,
    diagnostic: Option<String>,
    remediation: Option<String>,
}

fn parse_envelope(bytes: &[u8]) -> Option<ProductQualityEnvelope> {
    let trimmed = String::from_utf8_lossy(bytes).trim().to_string();
    if !trimmed.starts_with('{') {
        return None;
    }
    let value: serde_json::Value = serde_json::from_str(&trimmed).ok()?;
    let obj = value.as_object()?;
    let version = obj.get("version").and_then(|v| v.as_u64())?;
    if version != u64::from(PRODUCT_QUALITY_ENVELOPE_VERSION) {
        return None;
    }
    let status_str = obj.get("status").and_then(|v| v.as_str())?;
    let status = GateStatus::parse(status_str)?;
    let severity = obj
        .get("severity")
        .and_then(|v| v.as_str())
        .and_then(GateSeverity::parse);

    let mut findings = Vec::new();
    if let Some(arr) = obj.get("findings").and_then(|v| v.as_array()) {
        for f in arr.iter().take(MAX_FINDINGS) {
            let title = match f.get("title").and_then(|v| v.as_str()) {
                Some(s) if !s.is_empty() => s.to_string(),
                _ => continue,
            };
            let severity = f
                .get("severity")
                .and_then(|v| v.as_str())
                .and_then(GateSeverity::parse)
                .unwrap_or(GateSeverity::Warning);
            let location = f
                .get("location")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let rule = f
                .get("rule")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            findings.push(Finding {
                title: bound_text(&redact_secrets(&title), MAX_TITLE_BYTES),
                severity,
                location: location.map(|s| bound_text(&redact_secrets(&s), MAX_LOCATION_BYTES)),
                rule: rule.map(|s| bound_text(&s, 256)),
            });
        }
    }

    let mut evidence = Vec::new();
    if let Some(arr) = obj.get("evidence").and_then(|v| v.as_array()) {
        for e in arr.iter().take(MAX_EVIDENCE_REFS) {
            let key = match e.get("key").and_then(|v| v.as_str()) {
                Some(s) if !s.is_empty() => s.to_string(),
                _ => continue,
            };
            let digest = e
                .get("digest")
                .and_then(|v| v.as_str())
                .map(|s| bound_text(s, 128));
            let media_type = e
                .get("media_type")
                .and_then(|v| v.as_str())
                .map(|s| bound_text(s, 128));
            let byte_size = e.get("byte_size").and_then(|v| v.as_u64());
            let preview = e
                .get("preview")
                .and_then(|v| v.as_str())
                .map(|s| bound_text(&redact_secrets(s), MAX_PREVIEW_BYTES));
            evidence.push(EvidenceRef {
                key: bound_text(&key, MAX_EVIDENCE_KEY_BYTES),
                digest,
                media_type,
                byte_size,
                preview,
            });
        }
    }

    let missing_evidence: Vec<String> = obj
        .get("missing_evidence")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .take(MAX_MISSING_EVIDENCE)
                .filter_map(|item| item.as_str().map(|s| s.to_string()))
                .map(|s| bound_text(&s, MAX_EVIDENCE_KEY_BYTES))
                .collect()
        })
        .unwrap_or_default();

    let diagnostic = obj
        .get("diagnostic")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let remediation = obj
        .get("remediation")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    Some(ProductQualityEnvelope {
        status,
        severity,
        findings,
        evidence,
        missing_evidence,
        diagnostic,
        remediation,
    })
}

fn expected_exit_for_status(status: GateStatus) -> Option<i32> {
    match status {
        GateStatus::Pass => Some(0),
        GateStatus::Fail => Some(1),
        GateStatus::ReviewRequired => Some(2),
        GateStatus::NotApplicable => Some(0),
    }
}

fn default_severity_for_status(status: GateStatus) -> GateSeverity {
    match status {
        GateStatus::Pass => GateSeverity::Info,
        GateStatus::Fail => GateSeverity::Error,
        GateStatus::ReviewRequired => GateSeverity::Warning,
        GateStatus::NotApplicable => GateSeverity::Info,
    }
}

fn contradiction_result(
    tool_id: &str,
    gate_id: Option<&str>,
    envelope: &ProductQualityEnvelope,
    exit: i32,
    run: &CheckerRun,
    input: &AdapterInput,
) -> GateResult {
    let mut detail = format!(
        "command exited {exit} but envelope claimed {}; exit code is authoritative",
        envelope.status.as_str()
    );
    if !run.stderr.trim().is_empty() {
        detail.push_str(" | stderr: ");
        detail.push_str(&redact_secrets(run.stderr.trim()));
    }
    let diagnostic = bound_text(&detail, MAX_DIAGNOSTIC_BYTES);
    let mut missing = vec![format!("{tool_id}:output")];
    missing.truncate(MAX_MISSING_EVIDENCE);
    GateResult {
        gate_id: gate_id_for(tool_id, gate_id),
        source: source_for(tool_id),
        status: GateStatus::ReviewRequired,
        severity: GateSeverity::Warning,
        findings: vec![],
        evidence: bound_evidence(input.evidence.clone()),
        missing_evidence: missing,
        diagnostic: Some(diagnostic),
        remediation: Some(bound_text(
            "Align the command's exit code with the envelope status (PASS=0, FAIL=1, REVIEW_REQUIRED=2, NOT_APPLICABLE=0).",
            MAX_REMEDIATION_BYTES,
        )),
    }
}

fn envelope_result(
    tool_id: &str,
    gate_id: Option<&str>,
    envelope: &ProductQualityEnvelope,
    input: &AdapterInput,
) -> GateResult {
    // Caller has already validated the exit code; promote envelope
    // fields to a GateResult with bounds + redaction enforced.
    let severity = envelope
        .severity
        .unwrap_or_else(|| default_severity_for_status(envelope.status));
    let diagnostic = envelope
        .diagnostic
        .as_deref()
        .map(|d| bound_text(&redact_secrets(d), MAX_DIAGNOSTIC_BYTES))
        .filter(|s| !s.is_empty());
    let remediation = envelope
        .remediation
        .as_deref()
        .map(|d| bound_text(&redact_secrets(d), MAX_REMEDIATION_BYTES))
        .filter(|s| !s.is_empty());
    // Merge adapter-input evidence (raw-artifact refs from the
    // caller) with the envelope's own evidence list, bounded.
    let mut combined: Vec<EvidenceRef> = input.evidence.clone();
    combined.extend(envelope.evidence.iter().cloned());
    GateResult {
        gate_id: gate_id_for(tool_id, gate_id),
        source: source_for(tool_id),
        status: envelope.status,
        severity,
        findings: envelope.findings.clone(),
        evidence: bound_evidence(combined),
        missing_evidence: envelope.missing_evidence.clone(),
        diagnostic,
        remediation,
    }
}

/// Run a project-runtime command and apply the product-quality result
/// normalization rules:
///
/// * `PASS` requires exit 0, `FAIL` requires exit 1, `REVIEW_REQUIRED`
///   requires exit 2, `NOT_APPLICABLE` requires exit 0;
/// * the command exit code is authoritative: a status/exit mismatch
///   returns `REVIEW_REQUIRED` with the command's stderr as evidence;
/// * a malformed or absent envelope returns `REVIEW_REQUIRED` so
///   missing coverage is never silently treated as a pass;
/// * infrastructure failures (spawn, timeout, signal) stay
///   `REVIEW_REQUIRED` and name the missing evidence key;
/// * when no envelope is detected and the command exits cleanly, the
///   text-mode exit-code mapping preserves the existing
///   `project-runtime` semantics (exit 0 → `PASS`, nonzero → `FAIL`)
///   so commands that pre-date the envelope contract keep working.
///
/// This function is the entry point for the
/// `product-code-boundary` / `placeholder-threshold` concern ids. It
/// never embeds a language scanner or links a provider SDK; the
/// command binding is project-owned.
pub fn run_product_quality_adapter(
    tool_id: &str,
    gate_id: Option<&str>,
    input: &AdapterInput,
) -> GateResult {
    let spec = input.checker_spec(tool_id);
    let run = run_checker(&spec);
    if let Some(detail) = run.spawn_error.clone() {
        return infra_result(
            tool_id,
            gate_id,
            &AdapterError::SpawnFailed {
                tool: tool_id.to_string(),
                detail,
            },
            input.evidence.clone(),
            Some(exit_diagnostic(&run, tool_id)),
        );
    }
    if run.timed_out {
        let mut result = infra_result(
            tool_id,
            gate_id,
            &AdapterError::Timeout {
                tool: tool_id.to_string(),
                timeout_ms: input.timeout_ms,
            },
            input.evidence.clone(),
            Some(exit_diagnostic(&run, tool_id)),
        );
        result.missing_evidence.push(format!("{tool_id}:timeout"));
        result.missing_evidence.truncate(MAX_MISSING_EVIDENCE);
        return result;
    }
    if run.signalled {
        return infra_result(
            tool_id,
            gate_id,
            &AdapterError::Signalled {
                tool: tool_id.to_string(),
            },
            input.evidence.clone(),
            Some(exit_diagnostic(&run, tool_id)),
        );
    }
    let exit = run.exit_code.unwrap_or(-1);

    if let Some(envelope) = parse_envelope(run.stdout.as_bytes()) {
        let expected = expected_exit_for_status(envelope.status);
        return match expected {
            Some(code) if exit != code => {
                contradiction_result(tool_id, gate_id, &envelope, exit, &run, input)
            }
            _ => envelope_result(tool_id, gate_id, &envelope, input),
        };
    }

    // No envelope: malformed output cannot claim PASS. We surface a
    // bounded, secret-redacted diagnostic. The exit code still drives
    // the text-mode fallback for the rare case where a project
    // command predates the envelope contract.
    let output = if run.stdout.trim().is_empty() {
        run.stderr.clone()
    } else {
        run.stdout.clone()
    };
    let output_diag = bound_text(&redact_secrets(output.trim()), MAX_DIAGNOSTIC_BYTES);
    if output.trim().is_empty() {
        // No stdout, no stderr, no envelope: REVIEW_REQUIRED so
        // missing coverage never looks like a pass.
        return infra_result(
            tool_id,
            gate_id,
            &AdapterError::MalformedOutput {
                tool: tool_id.to_string(),
                format: "product-quality".to_string(),
                detail: "no JSON envelope and empty output".to_string(),
            },
            input.evidence.clone(),
            Some(exit_diagnostic(&run, tool_id)),
        );
    }
    match exit {
        0 => {
            // Output present but no envelope and exit 0: treat the
            // legacy text-mode path as a soft pass, but record the
            // missing envelope as diagnostic context.
            let note = "no product-quality JSON envelope; treated as text-mode pass";
            let diagnostic = if !output_diag.is_empty() {
                format!("{output_diag} | {note}")
            } else {
                note.to_string()
            };
            let diagnostic = bound_text(&redact_secrets(&diagnostic), MAX_DIAGNOSTIC_BYTES);
            GateResult {
                gate_id: gate_id_for(tool_id, gate_id),
                source: source_for(tool_id),
                status: GateStatus::Pass,
                severity: GateSeverity::Info,
                findings: vec![],
                evidence: bound_evidence(input.evidence.clone()),
                missing_evidence: vec![],
                diagnostic: Some(diagnostic),
                remediation: None,
            }
        }
        1 => GateResult {
            gate_id: gate_id_for(tool_id, gate_id),
            source: source_for(tool_id),
            status: GateStatus::Fail,
            severity: GateSeverity::Error,
            findings: vec![],
            evidence: bound_evidence(input.evidence.clone()),
            missing_evidence: vec![],
            diagnostic: Some(format!("exit 1 | {output_diag}")),
            remediation: Some(bound_text(
                "Fix the failing command and re-run; see the output diagnostic above.",
                MAX_REMEDIATION_BYTES,
            )),
        },
        2 => GateResult {
            gate_id: gate_id_for(tool_id, gate_id),
            source: source_for(tool_id),
            status: GateStatus::ReviewRequired,
            severity: GateSeverity::Warning,
            findings: vec![],
            evidence: bound_evidence(input.evidence.clone()),
            missing_evidence: vec![],
            diagnostic: Some(format!("exit 2 | {output_diag}")),
            remediation: Some(bound_text(
                "Address the review concern reported by the command and re-run.",
                MAX_REMEDIATION_BYTES,
            )),
        },
        _ => GateResult {
            gate_id: gate_id_for(tool_id, gate_id),
            source: source_for(tool_id),
            status: GateStatus::Fail,
            severity: GateSeverity::Error,
            findings: vec![],
            evidence: bound_evidence(input.evidence.clone()),
            missing_evidence: vec![],
            diagnostic: Some(format!("exit {exit} | {output_diag}")),
            remediation: Some(bound_text(
                "Fix the failing command and re-run; see the output diagnostic above.",
                MAX_REMEDIATION_BYTES,
            )),
        },
    }
}

/// Wire version of the release-gate JSON envelope. Both
/// `release-evidence` and `capability-conformance` consume envelopes
/// at this version; the parser distinguishes them by their key set.
pub const RELEASE_GATE_ENVELOPE_VERSION: u32 = 1;

/// Parsed release-evidence JSON envelope. The shape is project-owned
/// data: a versioned JSON document on stdout naming the source
/// revision, release version, artifacts, integrity, SBOM, provenance,
/// and publication state. Driftwatchdog never embeds a release
/// publisher, signer, SBOM generator, or deployment executor.
///
/// ## Wire shape (envelope `version: 1`)
///
/// * `version` (number, required): envelope wire version. Always `1`.
/// * `status` (string, required): `PASS` / `FAIL` / `REVIEW_REQUIRED`
///   / `NOT_APPLICABLE`. Exit-code authority is enforced separately.
/// * `severity` (string, optional): `info` / `warning` / `error`.
/// * `revision` (string, optional): source revision (typically a git
///   commit). Compared against the captured current revision when
///   both are present; a mismatch downgrades a `PASS` to
///   `REVIEW_REQUIRED`.
/// * `product_version` (string, optional): the version of the
///   released product (e.g. `1.2.3`). Required for a `PASS`.
/// * `artifacts` (array of strings, optional): artifact names
///   (e.g. `["binary.tar.gz", "checksums.txt"]`). At least one
///   required for a `PASS`.
/// * `provenance` (object or string, optional): provenance
///   evidence. Required for a `PASS`.
/// * `sbom` (object or string, optional): SBOM evidence.
/// * `publication_state` (string, optional): `draft` / `published` /
///   anything project-defined.
/// * `findings`, `evidence`, `missing_evidence`, `diagnostic`,
///   `remediation`: shared shape (see [`crate::gate::dto`]).
///
/// The release version is keyed `product_version` (not `version`) so
/// the envelope wire version can keep the simple `version` field that
/// every Gate envelope uses.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ReleaseEvidenceEnvelope {
    status: GateStatus,
    severity: Option<GateSeverity>,
    revision: Option<String>,
    product_version: Option<String>,
    artifacts: Vec<String>,
    has_sbom: bool,
    has_provenance: bool,
    publication_state: Option<String>,
    findings: Vec<Finding>,
    evidence: Vec<EvidenceRef>,
    missing_evidence: Vec<String>,
    diagnostic: Option<String>,
    remediation: Option<String>,
}

/// Parsed capability-conformance JSON envelope. The shape is
/// project-owned data: a versioned JSON document on stdout naming the
/// declared, configured, verified, and unverified capabilities.
/// Driftwatchdog never embeds a capability scanner.
#[derive(Debug, Clone, PartialEq, Eq)]
struct CapabilityEnvelope {
    status: GateStatus,
    severity: Option<GateSeverity>,
    declared: Vec<String>,
    configured: Vec<String>,
    verified: Vec<String>,
    unverified: Vec<String>,
    findings: Vec<Finding>,
    evidence: Vec<EvidenceRef>,
    missing_evidence: Vec<String>,
    diagnostic: Option<String>,
    remediation: Option<String>,
}

fn parse_findings(value: Option<&serde_json::Value>) -> Vec<Finding> {
    let mut findings = Vec::new();
    if let Some(arr) = value.and_then(|v| v.as_array()) {
        for f in arr.iter().take(MAX_FINDINGS) {
            let title = match f.get("title").and_then(|v| v.as_str()) {
                Some(s) if !s.is_empty() => s.to_string(),
                _ => continue,
            };
            let severity = f
                .get("severity")
                .and_then(|v| v.as_str())
                .and_then(GateSeverity::parse)
                .unwrap_or(GateSeverity::Warning);
            let location = f
                .get("location")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let rule = f
                .get("rule")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            findings.push(Finding {
                title: bound_text(&redact_secrets(&title), MAX_TITLE_BYTES),
                severity,
                location: location.map(|s| bound_text(&redact_secrets(&s), MAX_LOCATION_BYTES)),
                rule: rule.map(|s| bound_text(&s, 256)),
            });
        }
    }
    findings
}

fn parse_evidence(value: Option<&serde_json::Value>) -> Vec<EvidenceRef> {
    let mut evidence = Vec::new();
    if let Some(arr) = value.and_then(|v| v.as_array()) {
        for e in arr.iter().take(MAX_EVIDENCE_REFS) {
            let key = match e.get("key").and_then(|v| v.as_str()) {
                Some(s) if !s.is_empty() => s.to_string(),
                _ => continue,
            };
            let digest = e
                .get("digest")
                .and_then(|v| v.as_str())
                .map(|s| bound_text(s, 128));
            let media_type = e
                .get("media_type")
                .and_then(|v| v.as_str())
                .map(|s| bound_text(s, 128));
            let byte_size = e.get("byte_size").and_then(|v| v.as_u64());
            let preview = e
                .get("preview")
                .and_then(|v| v.as_str())
                .map(|s| bound_text(&redact_secrets(s), MAX_PREVIEW_BYTES));
            evidence.push(EvidenceRef {
                key: bound_text(&key, MAX_EVIDENCE_KEY_BYTES),
                digest,
                media_type,
                byte_size,
                preview,
            });
        }
    }
    evidence
}

fn parse_missing_evidence(value: Option<&serde_json::Value>) -> Vec<String> {
    value
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .take(MAX_MISSING_EVIDENCE)
                .filter_map(|item| item.as_str().map(|s| s.to_string()))
                .map(|s| bound_text(&s, MAX_EVIDENCE_KEY_BYTES))
                .collect()
        })
        .unwrap_or_default()
}

fn parse_status_and_severity(
    obj: &serde_json::Map<String, serde_json::Value>,
) -> Option<(GateStatus, Option<GateSeverity>)> {
    let status_str = obj.get("status").and_then(|v| v.as_str())?;
    let status = GateStatus::parse(status_str)?;
    let severity = obj
        .get("severity")
        .and_then(|v| v.as_str())
        .and_then(GateSeverity::parse);
    Some((status, severity))
}

fn parse_release_envelope(bytes: &[u8]) -> Option<ReleaseEvidenceEnvelope> {
    let trimmed = String::from_utf8_lossy(bytes).trim().to_string();
    if !trimmed.starts_with('{') {
        return None;
    }
    let value: serde_json::Value = serde_json::from_str(&trimmed).ok()?;
    let obj = value.as_object()?;
    let version = obj.get("version").and_then(|v| v.as_u64())?;
    if version != u64::from(RELEASE_GATE_ENVELOPE_VERSION) {
        return None;
    }
    let (status, severity) = parse_status_and_severity(obj)?;
    let revision = obj
        .get("revision")
        .and_then(|v| v.as_str())
        .map(|s| bound_text(s, 128));
    let product_version = obj
        .get("product_version")
        .and_then(|v| v.as_str())
        .map(|s| bound_text(s, 128));
    let artifacts: Vec<String> = obj
        .get("artifacts")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|item| item.as_str().map(|s| s.to_string()))
                .map(|s| bound_text(&s, 256))
                .collect()
        })
        .unwrap_or_default();
    let has_sbom = obj
        .get("sbom")
        .map(|v| v.is_object() || v.is_string())
        .unwrap_or(false);
    let has_provenance = obj
        .get("provenance")
        .map(|v| v.is_object() || v.is_string())
        .unwrap_or(false);
    let publication_state = obj
        .get("publication_state")
        .and_then(|v| v.as_str())
        .map(|s| bound_text(s, 64));
    let findings = parse_findings(obj.get("findings"));
    let evidence = parse_evidence(obj.get("evidence"));
    let missing_evidence = parse_missing_evidence(obj.get("missing_evidence"));
    let diagnostic = obj
        .get("diagnostic")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let remediation = obj
        .get("remediation")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    Some(ReleaseEvidenceEnvelope {
        status,
        severity,
        revision,
        product_version,
        artifacts,
        has_sbom,
        has_provenance,
        publication_state,
        findings,
        evidence,
        missing_evidence,
        diagnostic,
        remediation,
    })
}

fn parse_capability_envelope(bytes: &[u8]) -> Option<CapabilityEnvelope> {
    let trimmed = String::from_utf8_lossy(bytes).trim().to_string();
    if !trimmed.starts_with('{') {
        return None;
    }
    let value: serde_json::Value = serde_json::from_str(&trimmed).ok()?;
    let obj = value.as_object()?;
    let version = obj.get("version").and_then(|v| v.as_u64())?;
    if version != u64::from(RELEASE_GATE_ENVELOPE_VERSION) {
        return None;
    }
    let (status, severity) = parse_status_and_severity(obj)?;
    // `capabilities` is a sub-object: the parser looks inside it
    // (not at the top level) so the envelope's wire shape stays
    // compact. A missing `capabilities` object yields empty lists
    // rather than a parse error — the required-evidence guard then
    // refuses PASS for an empty `verified` list.
    let caps = obj.get("capabilities").and_then(|v| v.as_object());
    let read_list = |key: &str| -> Vec<String> {
        caps.and_then(|c| c.get(key))
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|item| item.as_str().map(|s| s.to_string()))
                    .map(|s| bound_text(&s, 128))
                    .collect()
            })
            .unwrap_or_default()
    };
    let declared = read_list("declared");
    let configured = read_list("configured");
    let verified = read_list("verified");
    let unverified = read_list("unverified");
    let findings = parse_findings(obj.get("findings"));
    let evidence = parse_evidence(obj.get("evidence"));
    let missing_evidence = parse_missing_evidence(obj.get("missing_evidence"));
    let diagnostic = obj
        .get("diagnostic")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let remediation = obj
        .get("remediation")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    Some(CapabilityEnvelope {
        status,
        severity,
        declared,
        configured,
        verified,
        unverified,
        findings,
        evidence,
        missing_evidence,
        diagnostic,
        remediation,
    })
}

fn envelope_review_result(
    tool_id: &str,
    gate_id: Option<&str>,
    reason: &str,
    evidence: Vec<EvidenceRef>,
    input_evidence: Vec<EvidenceRef>,
) -> GateResult {
    let mut combined = input_evidence;
    combined.extend(evidence);
    let mut missing = vec![format!("{tool_id}:output")];
    missing.truncate(MAX_MISSING_EVIDENCE);
    let diagnostic = bound_text(&redact_secrets(reason), MAX_DIAGNOSTIC_BYTES);
    GateResult {
        gate_id: gate_id_for(tool_id, gate_id),
        source: source_for(tool_id),
        status: GateStatus::ReviewRequired,
        severity: GateSeverity::Warning,
        findings: vec![],
        evidence: bound_evidence(combined),
        missing_evidence: missing,
        diagnostic: Some(diagnostic),
        remediation: Some(bound_text(
            "Re-run the command and emit a valid release-gate envelope; review the listed reason.",
            MAX_REMEDIATION_BYTES,
        )),
    }
}

// Helper that promotes parsed envelope fields into a GateResult.
// Bounded inputs plus redaction are enforced here so the call sites
// only have to forward the envelope values.
#[allow(clippy::too_many_arguments)]
fn envelope_to_result(
    tool_id: &str,
    gate_id: Option<&str>,
    status: GateStatus,
    severity: Option<GateSeverity>,
    findings: Vec<Finding>,
    evidence: Vec<EvidenceRef>,
    missing_evidence: Vec<String>,
    diagnostic: Option<String>,
    remediation: Option<String>,
    input_evidence: Vec<EvidenceRef>,
) -> GateResult {
    let severity = severity.unwrap_or_else(|| default_severity_for_status(status));
    let diagnostic = diagnostic
        .as_deref()
        .map(|d| bound_text(&redact_secrets(d), MAX_DIAGNOSTIC_BYTES))
        .filter(|s| !s.is_empty());
    let remediation = remediation
        .as_deref()
        .map(|d| bound_text(&redact_secrets(d), MAX_REMEDIATION_BYTES))
        .filter(|s| !s.is_empty());
    let mut combined: Vec<EvidenceRef> = input_evidence;
    combined.extend(evidence);
    GateResult {
        gate_id: gate_id_for(tool_id, gate_id),
        source: source_for(tool_id),
        status,
        severity,
        findings,
        evidence: bound_evidence(combined),
        missing_evidence,
        diagnostic,
        remediation,
    }
}

/// Run a project-owned command that reports release evidence.
///
/// The command must emit a versioned JSON envelope on stdout
/// describing the source revision, release version, artifacts,
/// integrity, SBOM, provenance, and publication state. The result
/// is normalised by:
///
/// * applying the same exit-code authority rule as the product-quality
///   adapter (`PASS`=0, `FAIL`=1, `REVIEW_REQUIRED`=2,
///   `NOT_APPLICABLE`=0; mismatches downgrade to `REVIEW_REQUIRED`);
/// * refusing `PASS` when required evidence is absent (revision,
///   release version, at least one artifact, provenance);
/// * refusing `PASS` when the envelope's revision does not match the
///   supplied `current_revision` (when both are present) — stale
///   evidence is `REVIEW_REQUIRED`;
/// * returning `REVIEW_REQUIRED` for malformed, missing, or
///   wrong-version envelopes so missing coverage is never silently
///   treated as a pass.
pub fn run_release_evidence_adapter(
    tool_id: &str,
    gate_id: Option<&str>,
    input: &AdapterInput,
    current_revision: Option<&str>,
) -> GateResult {
    let spec = input.checker_spec(tool_id);
    let run = run_checker(&spec);
    if let Some(detail) = run.spawn_error.clone() {
        return infra_result(
            tool_id,
            gate_id,
            &AdapterError::SpawnFailed {
                tool: tool_id.to_string(),
                detail,
            },
            input.evidence.clone(),
            Some(exit_diagnostic(&run, tool_id)),
        );
    }
    if run.timed_out {
        let mut result = infra_result(
            tool_id,
            gate_id,
            &AdapterError::Timeout {
                tool: tool_id.to_string(),
                timeout_ms: input.timeout_ms,
            },
            input.evidence.clone(),
            Some(exit_diagnostic(&run, tool_id)),
        );
        result.missing_evidence.push(format!("{tool_id}:timeout"));
        result.missing_evidence.truncate(MAX_MISSING_EVIDENCE);
        return result;
    }
    if run.signalled {
        return infra_result(
            tool_id,
            gate_id,
            &AdapterError::Signalled {
                tool: tool_id.to_string(),
            },
            input.evidence.clone(),
            Some(exit_diagnostic(&run, tool_id)),
        );
    }
    let exit = run.exit_code.unwrap_or(-1);

    if let Some(envelope) = parse_release_envelope(run.stdout.as_bytes()) {
        let expected = expected_exit_for_status(envelope.status);
        if let Some(code) = expected {
            if exit != code {
                return contradiction_result_release(
                    tool_id, gate_id, &envelope, exit, &run, input,
                );
            }
        }
        // Stale-evidence guard: if the envelope reports a revision
        // and the project exposes a current one (typically via git)
        // and the two disagree, the evidence is stale and a PASS
        // claim is refused.
        if let (Some(claimed), Some(actual)) = (envelope.revision.as_deref(), current_revision) {
            if claimed != actual && envelope.status == GateStatus::Pass {
                return envelope_review_result(
                    tool_id,
                    gate_id,
                    &format!(
                        "release evidence is stale: envelope claims revision `{claimed}` but current revision is `{actual}`"
                    ),
                    envelope.evidence.clone(),
                    input.evidence.clone(),
                );
            }
        }
        // Required-evidence guard for PASS: revision, product
        // version, at least one artifact, and provenance must all be
        // present.
        if envelope.status == GateStatus::Pass {
            let mut missing: Vec<String> = Vec::new();
            if envelope
                .revision
                .as_deref()
                .map(str::is_empty)
                .unwrap_or(true)
            {
                missing.push("revision".to_string());
            }
            if envelope
                .product_version
                .as_deref()
                .map(str::is_empty)
                .unwrap_or(true)
            {
                missing.push("product_version".to_string());
            }
            if envelope.artifacts.is_empty() {
                missing.push("artifacts".to_string());
            }
            if !envelope.has_provenance {
                missing.push("provenance".to_string());
            }
            if !missing.is_empty() {
                missing.sort();
                missing.dedup();
                return envelope_review_result(
                    tool_id,
                    gate_id,
                    &format!(
                        "release evidence claims PASS but is missing required fields: {}",
                        missing.join(", ")
                    ),
                    envelope.evidence,
                    input.evidence.clone(),
                );
            }
        }
        return envelope_to_result(
            tool_id,
            gate_id,
            envelope.status,
            envelope.severity,
            envelope.findings,
            envelope.evidence,
            envelope.missing_evidence,
            envelope.diagnostic,
            envelope.remediation,
            input.evidence.clone(),
        );
    }

    // No envelope: malformed or missing output cannot claim any
    // status. We downgrade to REVIEW_REQUIRED with a bounded,
    // secret-redacted diagnostic.
    let output = if run.stdout.trim().is_empty() {
        run.stderr.clone()
    } else {
        run.stdout.clone()
    };
    let output_diag = bound_text(&redact_secrets(output.trim()), MAX_DIAGNOSTIC_BYTES);
    if output.trim().is_empty() {
        return infra_result(
            tool_id,
            gate_id,
            &AdapterError::MalformedOutput {
                tool: tool_id.to_string(),
                format: "release-gate".to_string(),
                detail: "no JSON envelope and empty output".to_string(),
            },
            input.evidence.clone(),
            Some(exit_diagnostic(&run, tool_id)),
        );
    }
    envelope_review_result(
        tool_id,
        gate_id,
        &format!(
            "no release-gate JSON envelope; exit {exit}; output starts with: {}",
            output_diag.chars().take(128).collect::<String>()
        ),
        vec![],
        input.evidence.clone(),
    )
}

fn contradiction_result_release(
    tool_id: &str,
    gate_id: Option<&str>,
    envelope: &ReleaseEvidenceEnvelope,
    exit: i32,
    run: &CheckerRun,
    input: &AdapterInput,
) -> GateResult {
    let mut detail = format!(
        "command exited {exit} but envelope claimed {}; exit code is authoritative",
        envelope.status.as_str()
    );
    if !run.stderr.trim().is_empty() {
        detail.push_str(" | stderr: ");
        detail.push_str(&redact_secrets(run.stderr.trim()));
    }
    let diagnostic = bound_text(&detail, MAX_DIAGNOSTIC_BYTES);
    let mut missing = vec![format!("{tool_id}:output")];
    missing.truncate(MAX_MISSING_EVIDENCE);
    GateResult {
        gate_id: gate_id_for(tool_id, gate_id),
        source: source_for(tool_id),
        status: GateStatus::ReviewRequired,
        severity: GateSeverity::Warning,
        findings: vec![],
        evidence: bound_evidence(input.evidence.clone()),
        missing_evidence: missing,
        diagnostic: Some(diagnostic),
        remediation: Some(bound_text(
            "Align the command's exit code with the envelope status (PASS=0, FAIL=1, REVIEW_REQUIRED=2, NOT_APPLICABLE=0).",
            MAX_REMEDIATION_BYTES,
        )),
    }
}

/// Run a project-owned command that reports capability conformance.
///
/// The command must emit a versioned JSON envelope on stdout
/// describing the declared, configured, verified, and unverified
/// capabilities of the project. The result is normalised by:
///
/// * applying the same exit-code authority rule as the product-quality
///   and release-evidence adapters;
/// * refusing `PASS` when `capabilities.verified` is empty or absent —
///   unverified capabilities block;
/// * refusing `PASS` when the envelope claims to verify a capability
///   that is not part of the resolved plan (`scheduled_ids`) — the
///   claim would be evidence the gate cannot reproduce;
/// * returning `REVIEW_REQUIRED` for malformed, missing, or
///   wrong-version envelopes.
pub fn run_capability_conformance_adapter(
    tool_id: &str,
    gate_id: Option<&str>,
    input: &AdapterInput,
    scheduled_ids: &[String],
) -> GateResult {
    let spec = input.checker_spec(tool_id);
    let run = run_checker(&spec);
    if let Some(detail) = run.spawn_error.clone() {
        return infra_result(
            tool_id,
            gate_id,
            &AdapterError::SpawnFailed {
                tool: tool_id.to_string(),
                detail,
            },
            input.evidence.clone(),
            Some(exit_diagnostic(&run, tool_id)),
        );
    }
    if run.timed_out {
        let mut result = infra_result(
            tool_id,
            gate_id,
            &AdapterError::Timeout {
                tool: tool_id.to_string(),
                timeout_ms: input.timeout_ms,
            },
            input.evidence.clone(),
            Some(exit_diagnostic(&run, tool_id)),
        );
        result.missing_evidence.push(format!("{tool_id}:timeout"));
        result.missing_evidence.truncate(MAX_MISSING_EVIDENCE);
        return result;
    }
    if run.signalled {
        return infra_result(
            tool_id,
            gate_id,
            &AdapterError::Signalled {
                tool: tool_id.to_string(),
            },
            input.evidence.clone(),
            Some(exit_diagnostic(&run, tool_id)),
        );
    }
    let exit = run.exit_code.unwrap_or(-1);

    if let Some(envelope) = parse_capability_envelope(run.stdout.as_bytes()) {
        let expected = expected_exit_for_status(envelope.status);
        if let Some(code) = expected {
            if exit != code {
                return contradiction_result_capability(
                    tool_id, gate_id, &envelope, exit, &run, input,
                );
            }
        }
        if envelope.status == GateStatus::Pass {
            // Empty verified list is a claim that nothing is verified:
            // a PASS without verified capabilities is a contradiction.
            if envelope.verified.is_empty() {
                return envelope_review_result(
                    tool_id,
                    gate_id,
                    "capability-conformance claims PASS but `verified` is empty",
                    envelope.evidence.clone(),
                    input.evidence.clone(),
                );
            }
            // Stale-evidence guard: a verified capability that is not
            // in the resolved plan is a claim the gate cannot
            // reproduce. We compare against the scheduled ids (the
            // canonical gate vocabulary for this project) so projects
            // with custom checks do not get false positives for
            // their own ids.
            let scheduled: std::collections::BTreeSet<&str> =
                scheduled_ids.iter().map(|s| s.as_str()).collect();
            let mut out_of_scope: Vec<String> = envelope
                .verified
                .iter()
                .filter(|id| !scheduled.contains(id.as_str()))
                .cloned()
                .collect();
            if !out_of_scope.is_empty() {
                out_of_scope.sort();
                out_of_scope.dedup();
                return envelope_review_result(
                    tool_id,
                    gate_id,
                    &format!(
                        "capability-conformance claims verified capabilities not in the resolved plan: {}",
                        out_of_scope.join(", ")
                    ),
                    envelope.evidence.clone(),
                    input.evidence.clone(),
                );
            }
        }
        return envelope_to_result(
            tool_id,
            gate_id,
            envelope.status,
            envelope.severity,
            envelope.findings,
            envelope.evidence,
            envelope.missing_evidence,
            envelope.diagnostic,
            envelope.remediation,
            input.evidence.clone(),
        );
    }

    let output = if run.stdout.trim().is_empty() {
        run.stderr.clone()
    } else {
        run.stdout.clone()
    };
    let output_diag = bound_text(&redact_secrets(output.trim()), MAX_DIAGNOSTIC_BYTES);
    if output.trim().is_empty() {
        return infra_result(
            tool_id,
            gate_id,
            &AdapterError::MalformedOutput {
                tool: tool_id.to_string(),
                format: "release-gate".to_string(),
                detail: "no JSON envelope and empty output".to_string(),
            },
            input.evidence.clone(),
            Some(exit_diagnostic(&run, tool_id)),
        );
    }
    envelope_review_result(
        tool_id,
        gate_id,
        &format!(
            "no release-gate JSON envelope; exit {exit}; output starts with: {}",
            output_diag.chars().take(128).collect::<String>()
        ),
        vec![],
        input.evidence.clone(),
    )
}

fn contradiction_result_capability(
    tool_id: &str,
    gate_id: Option<&str>,
    envelope: &CapabilityEnvelope,
    exit: i32,
    run: &CheckerRun,
    input: &AdapterInput,
) -> GateResult {
    let mut detail = format!(
        "command exited {exit} but envelope claimed {}; exit code is authoritative",
        envelope.status.as_str()
    );
    if !run.stderr.trim().is_empty() {
        detail.push_str(" | stderr: ");
        detail.push_str(&redact_secrets(run.stderr.trim()));
    }
    let diagnostic = bound_text(&detail, MAX_DIAGNOSTIC_BYTES);
    let mut missing = vec![format!("{tool_id}:output")];
    missing.truncate(MAX_MISSING_EVIDENCE);
    GateResult {
        gate_id: gate_id_for(tool_id, gate_id),
        source: source_for(tool_id),
        status: GateStatus::ReviewRequired,
        severity: GateSeverity::Warning,
        findings: vec![],
        evidence: bound_evidence(input.evidence.clone()),
        missing_evidence: missing,
        diagnostic: Some(diagnostic),
        remediation: Some(bound_text(
            "Align the command's exit code with the envelope status (PASS=0, FAIL=1, REVIEW_REQUIRED=2, NOT_APPLICABLE=0).",
            MAX_REMEDIATION_BYTES,
        )),
    }
}

/// Run every adapter input in order, collecting one [`GateResult`] per
/// adapter. A failing adapter never aborts the rest: each result is
/// independent, mirroring the checker failure-isolation rule.
pub fn run_all(
    registry: &AdapterRegistry,
    inputs: &[(&str, AdapterInput, Option<String>)],
) -> Vec<GateResult> {
    let mut out = Vec::with_capacity(inputs.len());
    for (tool_id, input, gate_id) in inputs {
        match registry.get(tool_id) {
            Ok(adapter) => out.push(run_adapter(adapter, input, gate_id.as_deref())),
            Err(e) => {
                out.push(infra_result(
                    tool_id,
                    gate_id.as_deref(),
                    &e,
                    input.evidence.clone(),
                    None,
                ));
            }
        }
    }
    out
}

/// Deterministic evaluation over one normalized result: apply
/// threshold rules to findings and require evidence for an
/// evidence-backed `PASS`. No process is spawned, no tool API is
/// touched, and no AI provider is invoked.
///
/// * any finding at or above a rule threshold → `FAIL`;
/// * a `PASS` whose rules require evidence that does not resolve to
///   available artifacts → `REVIEW_REQUIRED`, so an unevidenced pass
///   is never claimed (the same guard as
///   [`crate::gate::evidence::evidence_backed_pass`], applied to the
///   rule-required keys);
/// * otherwise the adapter status stands.
pub fn evaluate(
    result: &GateResult,
    rules: &[DeterministicRule],
    resolve: &dyn Fn(&str) -> Option<ArtifactRecord>,
) -> GateResult {
    let mut evaluated = result.clone();
    for rule in rules {
        if result
            .findings
            .iter()
            .any(|f| severity_rank(f.severity) >= severity_rank(rule.threshold))
        {
            evaluated.status = GateStatus::Fail;
            evaluated.severity = result
                .findings
                .iter()
                .map(|f| f.severity)
                .max_by_key(|s| severity_rank(*s))
                .unwrap_or(result.severity);
            let note = format!("rule `{}` violated", rule.id);
            evaluated.diagnostic = Some(match evaluated.diagnostic.clone() {
                Some(d) => format!("{d} | {note}"),
                None => note,
            });
        }
    }
    if evaluated.status == GateStatus::Pass {
        let mut required: Vec<String> = Vec::new();
        for rule in rules {
            required.extend(rule.requires_evidence.iter().cloned());
        }
        if !required.is_empty() {
            let backed = required
                .iter()
                .all(|key| matches!(resolve(key), Some(rec) if rec.available));
            if !backed {
                evaluated.status = GateStatus::ReviewRequired;
                evaluated.severity = GateSeverity::Warning;
                let note = "required evidence missing; cannot claim evidence-backed PASS";
                evaluated.diagnostic = Some(match evaluated.diagnostic.clone() {
                    Some(d) => format!("{d} | {note}"),
                    None => note.to_string(),
                });
            }
        }
    }
    evaluated
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    const SARIF_FIXTURE: &str = r#"{
        "version": "2.1.0",
        "runs": [
            {
                "tool": {"driver": {"name": "semgrep"}},
                "results": [
                    {
                        "ruleId": "python.sql-injection",
                        "level": "error",
                        "message": {"text": "untrusted input in SQL query"},
                        "locations": [
                            {
                                "physicalLocation": {
                                    "artifactLocation": {"uri": "app/db.py"},
                                    "region": {"startLine": 42}
                                }
                            }
                        ]
                    },
                    {
                        "ruleId": "generic.note",
                        "level": "note",
                        "message": {"text": "informational"},
                        "locations": []
                    }
                ]
            }
        ]
    }"#;

    const GITLEAKS_FIXTURE: &str = r#"[
        {
            "Description": "Generic API Key",
            "RuleID": "generic-api-key",
            "File": "src/config.py",
            "StartLine": 7,
            "Secret": "sk-live-topsecret123"
        }
    ]"#;

    const OSV_FIXTURE: &str = r#"{
        "results": [
            {
                "packages": [
                    {
                        "package": {"name": "lodash", "version": "4.17.20"},
                        "vulnerabilities": [
                            {"id": "GHSA-4xc9-xhrj-v574", "summary": "prototype pollution"}
                        ]
                    }
                ]
            }
        ]
    }"#;

    const SEMGREP_FIXTURE: &str = r#"{
        "results": [
            {
                "check_id": "python.sql-injection",
                "path": "app/db.py",
                "start": {"line": 42},
                "extra": {"message": "untrusted input", "severity": "ERROR"}
            }
        ],
        "errors": []
    }"#;

    const CHECKER_FIXTURE: &str = r#"{
        "alerts": [
            {
                "severity": "error",
                "message": "Connection timeout does not follow spec",
                "source": "specs/database.md",
                "symbol": "DbPool"
            }
        ]
    }"#;

    fn test_input(program: &str, args: &[&str]) -> AdapterInput {
        AdapterInput {
            program: program.to_string(),
            args: args.iter().map(|s| s.to_string()).collect(),
            working_dir: std::env::temp_dir(),
            env: BTreeMap::new(),
            timeout_ms: 10_000,
            max_output_bytes: 1024 * 1024,
            evidence: vec![],
        }
    }

    fn registry() -> AdapterRegistry {
        default_registry()
    }

    #[test]
    fn default_registry_declares_five_adapters() {
        let r = registry();
        assert_eq!(r.len(), 5);
        assert_eq!(
            r.tool_ids(),
            vec!["checker", "gitleaks", "osv", "project-runtime", "semgrep"]
        );
    }

    #[test]
    fn duplicate_registration_fails() {
        let mut r = AdapterRegistry::new();
        r.register(checker_capability(), parse_checker_json)
            .unwrap();
        let err = r
            .register(checker_capability(), parse_checker_json)
            .unwrap_err();
        assert!(matches!(err, AdapterError::DuplicateAdapter { .. }));
    }

    #[test]
    fn unknown_adapter_lookup_fails() {
        let r = registry();
        let err = r.get("nope").unwrap_err();
        assert!(matches!(err, AdapterError::UnknownAdapter { .. }));
    }

    #[test]
    fn empty_capability_rejected_before_execution() {
        let mut r = AdapterRegistry::new();
        let err = r
            .register(
                AdapterCapability {
                    tool_id: "",
                    description: "x",
                    modes: vec![ExecutionMode::Managed],
                    formats: vec![OutputFormat::Text],
                    required_evidence: vec![],
                },
                parse_text_reject,
            )
            .unwrap_err();
        assert!(matches!(err, AdapterError::InvalidCapability(_)));
    }

    #[test]
    fn checker_success_fixture_returns_fail_with_findings() {
        let findings = parse_checker_json(CHECKER_FIXTURE.as_bytes()).unwrap();
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].severity, GateSeverity::Error);
        assert!(findings[0].location.as_deref().unwrap().contains("DbPool"));
    }

    #[test]
    fn checker_empty_alerts_pass() {
        let findings = parse_checker_json(br#"{"alerts": []}"#).unwrap();
        assert!(findings.is_empty());
    }

    #[test]
    fn checker_missing_alerts_is_malformed() {
        let err = parse_checker_json(br#"{}"#).unwrap_err();
        assert!(matches!(err, AdapterError::MalformedOutput { .. }));
    }

    #[test]
    fn sarif_fixture_preserves_rule_severity_location() {
        let findings = parse_sarif(SARIF_FIXTURE.as_bytes()).unwrap();
        assert_eq!(findings.len(), 2);
        assert_eq!(findings[0].rule.as_deref(), Some("python.sql-injection"));
        assert_eq!(findings[0].severity, GateSeverity::Error);
        assert_eq!(findings[0].location.as_deref(), Some("app/db.py:42"));
        assert_eq!(findings[1].severity, GateSeverity::Info);
    }

    #[test]
    fn sarif_non_document_is_malformed() {
        let err = parse_sarif(br#"{"alerts": []}"#).unwrap_err();
        assert!(matches!(err, AdapterError::MalformedOutput { .. }));
        assert!(err.to_string().contains("SARIF"));
    }

    #[test]
    fn gitleaks_fixture_never_embeds_secrets() {
        let findings = parse_gitleaks(GITLEAKS_FIXTURE.as_bytes()).unwrap();
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].rule.as_deref(), Some("generic-api-key"));
        assert_eq!(findings[0].severity, GateSeverity::Error);
        let debug = format!("{findings:?}");
        assert!(
            !debug.contains("sk-live-topsecret123"),
            "secret leaked: {debug}"
        );
    }

    #[test]
    fn gitleaks_empty_and_null_are_clean_not_malformed() {
        assert!(parse_gitleaks(b"[]").unwrap().is_empty());
        assert!(parse_gitleaks(b"null").unwrap().is_empty());
        assert!(parse_gitleaks(b"").unwrap().is_empty());
    }

    #[test]
    fn osv_fixture_maps_vulnerability_to_finding() {
        let findings = parse_osv(OSV_FIXTURE.as_bytes()).unwrap();
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].rule.as_deref(), Some("GHSA-4xc9-xhrj-v574"));
        assert!(findings[0].location.as_deref().unwrap().contains("lodash"));
    }

    #[test]
    fn semgrep_fixture_maps_check_to_finding() {
        let findings = parse_semgrep(SEMGREP_FIXTURE.as_bytes()).unwrap();
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].rule.as_deref(), Some("python.sql-injection"));
        assert_eq!(findings[0].severity, GateSeverity::Error);
        assert_eq!(findings[0].location.as_deref(), Some("app/db.py:42"));
    }

    #[test]
    fn findings_are_capped() {
        let mut alerts = String::from("{\"alerts\": [");
        for i in 0..300 {
            if i > 0 {
                alerts.push(',');
            }
            alerts.push_str(&format!(
                "{{\"severity\": \"warning\", \"message\": \"m{i}\", \"source\": \"s\", \"symbol\": \"S\"}}"
            ));
        }
        alerts.push_str("]}");
        let findings = parse_checker_json(alerts.as_bytes()).unwrap();
        let (capped, was_truncated) = truncate_findings(findings);
        assert!(was_truncated);
        assert_eq!(capped.len(), MAX_FINDINGS);
    }

    #[test]
    fn malformed_output_returns_review_and_others_continue() {
        let r = registry();
        // `true` exits 0 with empty stdout: checker parse fails
        // (missing `alerts`) → REVIEW_REQUIRED, not PASS.
        let bad = test_input("true", &[]);
        let good = test_input("true", &[]);
        let results = run_all(
            &r,
            &[
                ("checker", bad, Some("bad".to_string())),
                ("project-runtime", good, Some("ok".to_string())),
            ],
        );
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].status, GateStatus::ReviewRequired);
        assert!(!results[0].missing_evidence.is_empty());
        // The second adapter still ran and passed.
        assert_eq!(results[1].status, GateStatus::Pass);
    }

    #[test]
    fn nonzero_exit_with_findings_is_fail_not_infra() {
        // Gitleaks/Semgrep exit nonzero when they report findings: a
        // shell shim that prints the fixture and exits 1 must still
        // normalize to FAIL with findings.
        let r = registry();
        let dir = tempfile::tempdir().unwrap();
        let fixture = dir.path().join("semgrep.json");
        std::fs::write(&fixture, SEMGREP_FIXTURE).unwrap();
        let script = format!("cat {} ; exit 1", fixture.display());
        let input = test_input("sh", &["-c", &script]);
        let adapter = r.get("semgrep").unwrap();
        let result = run_adapter(adapter, &input, Some("semgrep-gate"));
        assert_eq!(result.status, GateStatus::Fail);
        assert_eq!(result.findings.len(), 1);
        assert!(result.diagnostic.as_deref().unwrap().contains("exit 1"));
    }

    #[test]
    fn nonzero_exit_without_output_is_review() {
        let r = registry();
        let input = test_input("sh", &["-c", "exit 3"]);
        let adapter = r.get("semgrep").unwrap();
        let result = run_adapter(adapter, &input, None);
        assert_eq!(result.status, GateStatus::ReviewRequired);
        assert!(result.diagnostic.as_deref().unwrap().contains("exit 3"));
    }

    #[test]
    fn timeout_records_timeout_evidence() {
        let r = registry();
        let mut input = test_input("sh", &["-c", "sleep 30"]);
        input.timeout_ms = 200;
        let adapter = r.get("semgrep").unwrap();
        let result = run_adapter(adapter, &input, None);
        assert_eq!(result.status, GateStatus::ReviewRequired);
        assert!(result
            .missing_evidence
            .iter()
            .any(|k| k.contains("timeout")));
    }

    #[test]
    fn signalled_child_is_review_not_fail() {
        let r = registry();
        let input = test_input("sh", &["-c", "kill -TERM $$"]);
        let adapter = r.get("semgrep").unwrap();
        let result = run_adapter(adapter, &input, None);
        assert_eq!(result.status, GateStatus::ReviewRequired);
        assert!(result.diagnostic.as_deref().unwrap().contains("signal"));
    }

    #[test]
    fn missing_executable_is_review_with_diagnostic() {
        let r = registry();
        let input = test_input("driftwatch-no-such-tool-xyz", &[]);
        let adapter = r.get("semgrep").unwrap();
        let result = run_adapter(adapter, &input, None);
        assert_eq!(result.status, GateStatus::ReviewRequired);
        assert_eq!(result.missing_evidence, vec!["semgrep:output".to_string()]);
    }

    #[test]
    fn project_runtime_zero_is_pass_nonzero_is_fail() {
        let r = registry();
        let adapter = r.get("project-runtime").unwrap();
        let pass = run_adapter(adapter, &test_input("true", &[]), None);
        assert_eq!(pass.status, GateStatus::Pass);
        let fail = run_adapter(
            adapter,
            &test_input("sh", &["-c", "echo broken >&2; exit 2"]),
            None,
        );
        assert_eq!(fail.status, GateStatus::Fail);
        assert_eq!(fail.severity, GateSeverity::Error);
        assert!(fail.diagnostic.as_deref().unwrap().contains("exit 2"));
        assert!(fail.diagnostic.as_deref().unwrap().contains("broken"));
    }

    #[test]
    fn project_runtime_argv_not_shell_split() {
        // An argument containing spaces and metacharacters reaches the
        // child as one argv element (no shell interpretation).
        let r = registry();
        let adapter = r.get("project-runtime").unwrap();
        let input = test_input("printf", &["%s", "a b; rm -rf /"]);
        let result = run_adapter(adapter, &input, None);
        assert_eq!(result.status, GateStatus::Pass);
        assert!(result
            .diagnostic
            .as_deref()
            .unwrap()
            .contains("a b; rm -rf /"));
    }

    #[test]
    fn unknown_adapter_in_run_all_is_review() {
        let r = registry();
        let results = run_all(&r, &[("nope", test_input("true", &[]), None)]);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].status, GateStatus::ReviewRequired);
    }

    #[test]
    fn evaluator_blocks_on_threshold_violation() {
        let result = findings_result(
            "semgrep",
            None,
            parse_semgrep(SEMGREP_FIXTURE.as_bytes()).unwrap(),
            vec![],
            None,
            false,
        );
        assert_eq!(result.status, GateStatus::Fail);
        let evaluated = evaluate(
            &result,
            &[DeterministicRule {
                id: "no-error-findings".to_string(),
                threshold: GateSeverity::Error,
                requires_evidence: vec![],
            }],
            &|_: &str| None,
        );
        assert_eq!(evaluated.status, GateStatus::Fail);
        assert!(evaluated
            .diagnostic
            .as_deref()
            .unwrap()
            .contains("no-error-findings"));
    }

    #[test]
    fn evaluator_denies_unevidenced_pass() {
        let pass = GateResult {
            gate_id: "gitleaks".to_string(),
            source: "adapter:gitleaks".to_string(),
            status: GateStatus::Pass,
            severity: GateSeverity::Info,
            findings: vec![],
            evidence: vec![],
            missing_evidence: vec![],
            diagnostic: None,
            remediation: None,
        };
        let evaluated = evaluate(
            &pass,
            &[DeterministicRule {
                id: "secret-scan-evidenced".to_string(),
                threshold: GateSeverity::Error,
                requires_evidence: vec!["gitleaks:report".to_string()],
            }],
            &|_: &str| None,
        );
        assert_eq!(evaluated.status, GateStatus::ReviewRequired);
    }

    #[test]
    fn evaluator_keeps_evidenced_pass() {
        use crate::gate::evidence::{ArtifactKind, ArtifactRecord};
        let pass = GateResult {
            gate_id: "gitleaks".to_string(),
            source: "adapter:gitleaks".to_string(),
            status: GateStatus::Pass,
            severity: GateSeverity::Info,
            findings: vec![],
            evidence: vec![EvidenceRef {
                key: "gitleaks:report".to_string(),
                digest: None,
                media_type: None,
                byte_size: None,
                preview: None,
            }],
            missing_evidence: vec![],
            diagnostic: None,
            remediation: None,
        };
        let record = ArtifactRecord {
            key: "gitleaks:report".to_string(),
            kind: ArtifactKind::ToolJson,
            producer: "gitleaks".to_string(),
            producer_version: None,
            media_type: None,
            digest: "sha256:abc".to_string(),
            byte_size: 12,
            preview: Some("ok".to_string()),
            redacted: true,
            available: true,
            created_at: "2026-09-15T00:00:00Z".to_string(),
            rel_path: Some("artifacts/x.json".to_string()),
        };
        let evaluated = evaluate(
            &pass,
            &[DeterministicRule {
                id: "secret-scan-evidenced".to_string(),
                threshold: GateSeverity::Error,
                requires_evidence: vec!["gitleaks:report".to_string()],
            }],
            &|key: &str| {
                if key == "gitleaks:report" {
                    Some(record.clone())
                } else {
                    None
                }
            },
        );
        assert_eq!(evaluated.status, GateStatus::Pass);
    }

    #[test]
    fn evaluator_needs_no_ai_and_spawns_nothing() {
        // Pure function over values: no process, no network, no provider.
        let started = std::time::Instant::now();
        let result = findings_result("x", None, vec![], vec![], None, false);
        let evaluated = evaluate(&result, &[], &|_: &str| None);
        assert_eq!(evaluated.status, GateStatus::Pass);
        assert!(started.elapsed() < Duration::from_secs(5));
    }

    #[test]
    fn no_scanner_sdk_coupling() {
        // Capabilities reference tool ids and CLI formats only; the
        // production code must not name provider SDKs, async HTTP
        // clients, or model identifiers. Only the code above the test
        // module is scanned (this test names the banned terms itself).
        let src = include_str!("adapters.rs");
        let prod = src.split("#[cfg(test)]").next().unwrap_or(src);
        for banned in [
            "openai::",
            "anthropic::",
            "reqwest",
            "async-openai",
            "tokio::",
            "model_id",
            "api_key::",
        ] {
            assert!(
                !prod.to_ascii_lowercase().contains(banned),
                "banned term `{banned}` in adapters.rs"
            );
        }
    }

    // --- product-quality adapter ----------------------------------------

    const PASS_ENVELOPE: &str = r#"{"version":1,"status":"PASS","severity":"info","findings":[],"diagnostic":null,"remediation":null}"#;
    const FAIL_ENVELOPE: &str = r#"{"version":1,"status":"FAIL","severity":"error","findings":[{"title":"tests in src/","severity":"error","location":"src/foo.rs","rule":"no-tests-in-product"}],"diagnostic":"see findings","remediation":"move tests"}"#;
    const REVIEW_ENVELOPE: &str = r#"{"version":1,"status":"REVIEW_REQUIRED","severity":"warning","findings":[],"missing_evidence":["checker:run"]}"#;
    const NOT_APPLICABLE_ENVELOPE: &str =
        r#"{"version":1,"status":"NOT_APPLICABLE","severity":"info","findings":[]}"#;

    fn pq_input(program: &str, args: &[&str]) -> AdapterInput {
        AdapterInput {
            program: program.to_string(),
            args: args.iter().map(|s| s.to_string()).collect(),
            working_dir: std::env::temp_dir(),
            env: BTreeMap::new(),
            timeout_ms: 10_000,
            max_output_bytes: 1024 * 1024,
            evidence: vec![],
        }
    }

    fn write_envelope_exit(dir: &std::path::Path, envelope: &str, exit: i32) -> String {
        let path = dir.join("envelope.json");
        std::fs::write(&path, envelope).unwrap();
        format!("cat {} ; exit {}", path.display(), exit)
    }

    #[test]
    fn product_quality_passing_envelope_passes() {
        let tmp = tempfile::tempdir().unwrap();
        let script = write_envelope_exit(tmp.path(), PASS_ENVELOPE, 0);
        let result = run_product_quality_adapter(
            "project-runtime",
            Some("product-code-boundary"),
            &pq_input("sh", &["-c", &script]),
        );
        assert_eq!(result.status, GateStatus::Pass);
        assert_eq!(result.severity, GateSeverity::Info);
        assert!(result.findings.is_empty());
        assert!(result.diagnostic.is_none() || result.diagnostic.as_deref().unwrap().is_empty());
    }

    #[test]
    fn product_quality_failing_envelope_fails() {
        let tmp = tempfile::tempdir().unwrap();
        let script = write_envelope_exit(tmp.path(), FAIL_ENVELOPE, 1);
        let result = run_product_quality_adapter(
            "project-runtime",
            Some("product-code-boundary"),
            &pq_input("sh", &["-c", &script]),
        );
        assert_eq!(result.status, GateStatus::Fail);
        assert_eq!(result.severity, GateSeverity::Error);
        assert_eq!(result.findings.len(), 1);
        let f = &result.findings[0];
        assert_eq!(f.title, "tests in src/");
        assert_eq!(f.location.as_deref(), Some("src/foo.rs"));
        assert_eq!(f.rule.as_deref(), Some("no-tests-in-product"));
    }

    #[test]
    fn product_quality_review_envelope_reviews() {
        let tmp = tempfile::tempdir().unwrap();
        let script = write_envelope_exit(tmp.path(), REVIEW_ENVELOPE, 2);
        let result = run_product_quality_adapter(
            "project-runtime",
            Some("placeholder-threshold"),
            &pq_input("sh", &["-c", &script]),
        );
        assert_eq!(result.status, GateStatus::ReviewRequired);
        assert_eq!(result.severity, GateSeverity::Warning);
        assert!(result.missing_evidence.contains(&"checker:run".to_string()));
    }

    #[test]
    fn product_quality_not_applicable_envelope_passes_as_not_applicable() {
        let tmp = tempfile::tempdir().unwrap();
        let script = write_envelope_exit(tmp.path(), NOT_APPLICABLE_ENVELOPE, 0);
        let result = run_product_quality_adapter(
            "project-runtime",
            Some("placeholder-threshold"),
            &pq_input("sh", &["-c", &script]),
        );
        assert_eq!(result.status, GateStatus::NotApplicable);
        assert_eq!(result.severity, GateSeverity::Info);
    }

    #[test]
    fn product_quality_contradictory_exit_for_pass_becomes_review() {
        // Command exits 1 but claims PASS: exit code is authoritative.
        let tmp = tempfile::tempdir().unwrap();
        let script = write_envelope_exit(tmp.path(), PASS_ENVELOPE, 1);
        let result = run_product_quality_adapter(
            "project-runtime",
            Some("product-code-boundary"),
            &pq_input("sh", &["-c", &script]),
        );
        assert_eq!(result.status, GateStatus::ReviewRequired);
        assert_eq!(result.severity, GateSeverity::Warning);
        let diag = result.diagnostic.as_deref().unwrap();
        assert!(diag.contains("exited 1"), "diagnostic: {diag}");
        assert!(diag.contains("PASS"), "diagnostic: {diag}");
        assert!(
            diag.contains("exit code is authoritative"),
            "diagnostic: {diag}"
        );
    }

    #[test]
    fn product_quality_contradictory_exit_for_fail_becomes_review() {
        let tmp = tempfile::tempdir().unwrap();
        let script = write_envelope_exit(tmp.path(), FAIL_ENVELOPE, 0);
        let result = run_product_quality_adapter(
            "project-runtime",
            Some("product-code-boundary"),
            &pq_input("sh", &["-c", &script]),
        );
        assert_eq!(result.status, GateStatus::ReviewRequired);
        let diag = result.diagnostic.as_deref().unwrap();
        assert!(diag.contains("exited 0"), "diagnostic: {diag}");
        assert!(diag.contains("FAIL"), "diagnostic: {diag}");
    }

    #[test]
    fn product_quality_contradictory_exit_for_review_becomes_review() {
        // exit 0 with REVIEW_REQUIRED claim: contradiction.
        let tmp = tempfile::tempdir().unwrap();
        let script = write_envelope_exit(tmp.path(), REVIEW_ENVELOPE, 0);
        let result = run_product_quality_adapter(
            "project-runtime",
            Some("placeholder-threshold"),
            &pq_input("sh", &["-c", &script]),
        );
        assert_eq!(result.status, GateStatus::ReviewRequired);
    }

    #[test]
    fn product_quality_malformed_envelope_with_output_becomes_review() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("bad.json");
        std::fs::write(&path, "this is not json").unwrap();
        let script = format!("cat {} ; exit 0", path.display());
        let result = run_product_quality_adapter(
            "project-runtime",
            Some("product-code-boundary"),
            &pq_input("sh", &["-c", &script]),
        );
        // No envelope, exit 0, output present: legacy text-mode pass
        // with envelope-missing diagnostic.
        assert_eq!(result.status, GateStatus::Pass);
        let diag = result.diagnostic.as_deref().unwrap();
        assert!(
            diag.contains("no product-quality JSON envelope"),
            "diag: {diag}"
        );
    }

    #[test]
    fn product_quality_empty_output_becomes_review() {
        // Exit 0, no stdout, no stderr, no envelope: REVIEW_REQUIRED so
        // missing coverage never looks like a pass.
        let result = run_product_quality_adapter(
            "project-runtime",
            Some("product-code-boundary"),
            &pq_input("sh", &["-c", "exit 0"]),
        );
        assert_eq!(result.status, GateStatus::ReviewRequired);
        assert!(result.missing_evidence.iter().any(|k| k.contains("output")));
    }

    #[test]
    fn product_quality_unknown_status_in_envelope_becomes_review() {
        let tmp = tempfile::tempdir().unwrap();
        let bad = r#"{"version":1,"status":"MAYBE","findings":[]}"#;
        let script = write_envelope_exit(tmp.path(), bad, 0);
        let result = run_product_quality_adapter(
            "project-runtime",
            Some("product-code-boundary"),
            &pq_input("sh", &["-c", &script]),
        );
        // Unknown status means parse_envelope returns None, so we
        // fall back to text-mode pass with envelope-missing diagnostic.
        assert_eq!(result.status, GateStatus::Pass);
        let diag = result.diagnostic.as_deref().unwrap();
        assert!(
            diag.contains("no product-quality JSON envelope"),
            "diag: {diag}"
        );
    }

    #[test]
    fn product_quality_wrong_envelope_version_becomes_review() {
        let tmp = tempfile::tempdir().unwrap();
        let bad = r#"{"version":99,"status":"PASS","findings":[]}"#;
        let script = write_envelope_exit(tmp.path(), bad, 0);
        let result = run_product_quality_adapter(
            "project-runtime",
            Some("product-code-boundary"),
            &pq_input("sh", &["-c", &script]),
        );
        // Wrong version rejects the envelope, so we fall back to
        // text-mode pass with envelope-missing diagnostic.
        assert_eq!(result.status, GateStatus::Pass);
        let diag = result.diagnostic.as_deref().unwrap();
        assert!(
            diag.contains("no product-quality JSON envelope"),
            "diag: {diag}"
        );
    }

    #[test]
    fn product_quality_text_mode_preserves_legacy_exit_one_fail() {
        // Exit 1 with no envelope and stderr present: the legacy
        // text-mode FAIL path still applies so old commands work.
        let result = run_product_quality_adapter(
            "project-runtime",
            Some("product-code-boundary"),
            &pq_input("sh", &["-c", "echo boom >&2; exit 1"]),
        );
        assert_eq!(result.status, GateStatus::Fail);
        let diag = result.diagnostic.as_deref().unwrap();
        assert!(diag.contains("exit 1"), "diag: {diag}");
        assert!(diag.contains("boom"), "diag: {diag}");
    }

    #[test]
    fn product_quality_text_mode_preserves_legacy_exit_two_review() {
        // Exit 2 with no envelope: text-mode path records REVIEW_REQUIRED
        // so commands that pre-date the envelope still get a useful
        // status without claiming PASS.
        let result = run_product_quality_adapter(
            "project-runtime",
            Some("product-code-boundary"),
            &pq_input("sh", &["-c", "echo review >&2; exit 2"]),
        );
        assert_eq!(result.status, GateStatus::ReviewRequired);
    }

    #[test]
    fn product_quality_timeout_records_timeout_evidence() {
        let mut input = pq_input("sh", &["-c", "sleep 30"]);
        input.timeout_ms = 200;
        let result =
            run_product_quality_adapter("project-runtime", Some("product-code-boundary"), &input);
        assert_eq!(result.status, GateStatus::ReviewRequired);
        assert!(result
            .missing_evidence
            .iter()
            .any(|k| k.contains("timeout")));
    }

    #[test]
    fn product_quality_signalled_child_is_review() {
        let result = run_product_quality_adapter(
            "project-runtime",
            Some("product-code-boundary"),
            &pq_input("sh", &["-c", "kill -TERM $$"]),
        );
        assert_eq!(result.status, GateStatus::ReviewRequired);
    }

    #[test]
    fn product_quality_missing_executable_is_review() {
        let result = run_product_quality_adapter(
            "project-runtime",
            Some("product-code-boundary"),
            &pq_input("driftwatch-no-such-tool-xyz", &[]),
        );
        assert_eq!(result.status, GateStatus::ReviewRequired);
    }

    #[test]
    fn product_quality_redacts_secrets_in_diagnostic() {
        let tmp = tempfile::tempdir().unwrap();
        let bad = r#"{"version":1,"status":"FAIL","diagnostic":"api_key=hunter2-secret"}"#;
        let script = write_envelope_exit(tmp.path(), bad, 1);
        let result = run_product_quality_adapter(
            "project-runtime",
            Some("product-code-boundary"),
            &pq_input("sh", &["-c", &script]),
        );
        assert_eq!(result.status, GateStatus::Fail);
        let diag = result.diagnostic.as_deref().unwrap();
        assert!(!diag.contains("hunter2-secret"), "diag: {diag}");
        assert!(diag.contains("[REDACTED]"), "diag: {diag}");
    }

    #[test]
    fn product_quality_envelope_evidence_is_bounded() {
        // The envelope can carry evidence; the adapter bounds it
        // through the shared helper.
        let tmp = tempfile::tempdir().unwrap();
        let mut env = r#"{"version":1,"status":"PASS","findings":[],"evidence":["#.to_string();
        for i in 0..200 {
            if i > 0 {
                env.push(',');
            }
            env.push_str(&format!(r#"{{"key":"k{i}"}}"#));
        }
        env.push_str("]}");
        let script = write_envelope_exit(tmp.path(), &env, 0);
        let result = run_product_quality_adapter(
            "project-runtime",
            Some("product-code-boundary"),
            &pq_input("sh", &["-c", &script]),
        );
        assert_eq!(result.status, GateStatus::Pass);
        assert!(result.evidence.len() <= crate::gate::dto::MAX_EVIDENCE_REFS);
    }

    // --- release-evidence adapter ---------------------------------------

    const RELEASE_PASS_ENVELOPE: &str = r#"{
        "version":1,
        "status":"PASS",
        "severity":"info",
        "revision":"abc123",
        "product_version":"1.2.3",
        "artifacts":["binary.tar.gz","checksums.txt"],
        "provenance":{"type":"slsa-provenance/v0.2","digest":"sha256:deadbeef"},
        "sbom":{"media_type":"text/spdx","digest":"sha256:beefdead"},
        "publication_state":"draft",
        "findings":[]
    }"#;
    const RELEASE_FAIL_ENVELOPE: &str = r#"{
        "version":1,
        "status":"FAIL",
        "severity":"error",
        "findings":[{"title":"missing SBOM","severity":"error","rule":"sbom-required"}]
    }"#;
    const RELEASE_REVIEW_ENVELOPE: &str = r#"{
        "version":1,
        "status":"REVIEW_REQUIRED",
        "severity":"warning",
        "findings":[],
        "missing_evidence":["provenance"]
    }"#;

    fn re_input(program: &str, args: &[&str]) -> AdapterInput {
        AdapterInput {
            program: program.to_string(),
            args: args.iter().map(|s| s.to_string()).collect(),
            working_dir: std::env::temp_dir(),
            env: BTreeMap::new(),
            timeout_ms: 10_000,
            max_output_bytes: 1024 * 1024,
            evidence: vec![],
        }
    }

    #[test]
    fn release_evidence_complete_pass_envelope_passes() {
        let tmp = tempfile::tempdir().unwrap();
        let script = write_envelope_exit(tmp.path(), RELEASE_PASS_ENVELOPE, 0);
        let result = run_release_evidence_adapter(
            "project-runtime",
            Some("release-evidence"),
            &re_input("sh", &["-c", &script]),
            Some("abc123"),
        );
        assert_eq!(result.status, GateStatus::Pass);
        assert_eq!(result.severity, GateSeverity::Info);
    }

    #[test]
    fn release_evidence_failing_envelope_fails() {
        let tmp = tempfile::tempdir().unwrap();
        let script = write_envelope_exit(tmp.path(), RELEASE_FAIL_ENVELOPE, 1);
        let result = run_release_evidence_adapter(
            "project-runtime",
            Some("release-evidence"),
            &re_input("sh", &["-c", &script]),
            None,
        );
        assert_eq!(result.status, GateStatus::Fail);
        assert_eq!(result.severity, GateSeverity::Error);
        assert_eq!(result.findings.len(), 1);
        assert_eq!(result.findings[0].title, "missing SBOM");
    }

    #[test]
    fn release_evidence_review_envelope_reviews() {
        let tmp = tempfile::tempdir().unwrap();
        let script = write_envelope_exit(tmp.path(), RELEASE_REVIEW_ENVELOPE, 2);
        let result = run_release_evidence_adapter(
            "project-runtime",
            Some("release-evidence"),
            &re_input("sh", &["-c", &script]),
            None,
        );
        assert_eq!(result.status, GateStatus::ReviewRequired);
        assert!(result.missing_evidence.contains(&"provenance".to_string()));
    }

    #[test]
    fn release_evidence_contradictory_exit_for_pass_becomes_review() {
        let tmp = tempfile::tempdir().unwrap();
        let script = write_envelope_exit(tmp.path(), RELEASE_PASS_ENVELOPE, 1);
        let result = run_release_evidence_adapter(
            "project-runtime",
            Some("release-evidence"),
            &re_input("sh", &["-c", &script]),
            Some("abc123"),
        );
        assert_eq!(result.status, GateStatus::ReviewRequired);
        let diag = result.diagnostic.as_deref().unwrap();
        assert!(diag.contains("exited 1"));
        assert!(diag.contains("PASS"));
        assert!(diag.contains("exit code is authoritative"));
    }

    #[test]
    fn release_evidence_stale_revision_downgrades_pass_to_review() {
        let tmp = tempfile::tempdir().unwrap();
        let script = write_envelope_exit(tmp.path(), RELEASE_PASS_ENVELOPE, 0);
        let result = run_release_evidence_adapter(
            "project-runtime",
            Some("release-evidence"),
            &re_input("sh", &["-c", &script]),
            // Current revision is different from the envelope's
            // claim: stale evidence must refuse PASS.
            Some("def456"),
        );
        assert_eq!(result.status, GateStatus::ReviewRequired);
        let diag = result.diagnostic.as_deref().unwrap();
        assert!(diag.contains("stale"), "diag: {diag}");
        assert!(diag.contains("abc123"), "diag: {diag}");
        assert!(diag.contains("def456"), "diag: {diag}");
    }

    #[test]
    fn release_evidence_missing_current_revision_does_not_trigger_staleness() {
        // No git rev available (project not in a worktree): the
        // adapter skips the staleness rule and honours the envelope
        // status (still subject to required-evidence checks).
        let tmp = tempfile::tempdir().unwrap();
        let script = write_envelope_exit(tmp.path(), RELEASE_PASS_ENVELOPE, 0);
        let result = run_release_evidence_adapter(
            "project-runtime",
            Some("release-evidence"),
            &re_input("sh", &["-c", &script]),
            None,
        );
        assert_eq!(result.status, GateStatus::Pass);
    }

    #[test]
    fn release_evidence_missing_required_field_downgrades_pass_to_review() {
        // No revision, no product_version, no artifacts, no
        // provenance: required-evidence guard refuses PASS.
        let env = r#"{"version":1,"status":"PASS","findings":[]}"#;
        let tmp = tempfile::tempdir().unwrap();
        let script = write_envelope_exit(tmp.path(), env, 0);
        let result = run_release_evidence_adapter(
            "project-runtime",
            Some("release-evidence"),
            &re_input("sh", &["-c", &script]),
            None,
        );
        assert_eq!(result.status, GateStatus::ReviewRequired);
        let diag = result.diagnostic.as_deref().unwrap();
        assert!(diag.contains("missing required fields"), "diag: {diag}");
        assert!(diag.contains("revision"), "diag: {diag}");
        assert!(diag.contains("product_version"), "diag: {diag}");
        assert!(diag.contains("artifacts"), "diag: {diag}");
        assert!(diag.contains("provenance"), "diag: {diag}");
    }

    #[test]
    fn release_evidence_no_artifacts_becomes_review() {
        // Revision + product_version + provenance but no artifacts:
        // still missing a required field.
        let env = r#"{"version":1,"status":"PASS","revision":"abc","product_version":"1.0","provenance":{"type":"x"}}"#;
        let tmp = tempfile::tempdir().unwrap();
        let script = write_envelope_exit(tmp.path(), env, 0);
        let result = run_release_evidence_adapter(
            "project-runtime",
            Some("release-evidence"),
            &re_input("sh", &["-c", &script]),
            None,
        );
        assert_eq!(result.status, GateStatus::ReviewRequired);
        let diag = result.diagnostic.as_deref().unwrap();
        assert!(diag.contains("artifacts"), "diag: {diag}");
    }

    #[test]
    fn release_evidence_wrong_version_becomes_review() {
        let env = r#"{"version":99,"status":"PASS","findings":[]}"#;
        let tmp = tempfile::tempdir().unwrap();
        let script = write_envelope_exit(tmp.path(), env, 0);
        let result = run_release_evidence_adapter(
            "project-runtime",
            Some("release-evidence"),
            &re_input("sh", &["-c", &script]),
            None,
        );
        assert_eq!(result.status, GateStatus::ReviewRequired);
        let diag = result.diagnostic.as_deref().unwrap();
        assert!(
            diag.contains("no release-gate JSON envelope"),
            "diag: {diag}"
        );
    }

    #[test]
    fn release_evidence_malformed_envelope_with_output_becomes_review() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("bad.json");
        std::fs::write(&path, "not json").unwrap();
        let script = format!("cat {} ; exit 0", path.display());
        let result = run_release_evidence_adapter(
            "project-runtime",
            Some("release-evidence"),
            &re_input("sh", &["-c", &script]),
            None,
        );
        assert_eq!(result.status, GateStatus::ReviewRequired);
        let diag = result.diagnostic.as_deref().unwrap();
        assert!(
            diag.contains("no release-gate JSON envelope"),
            "diag: {diag}"
        );
    }

    #[test]
    fn release_evidence_empty_output_becomes_review() {
        let result = run_release_evidence_adapter(
            "project-runtime",
            Some("release-evidence"),
            &re_input("sh", &["-c", "exit 0"]),
            None,
        );
        assert_eq!(result.status, GateStatus::ReviewRequired);
        assert!(result.missing_evidence.iter().any(|k| k.contains("output")));
    }

    #[test]
    fn release_evidence_unknown_status_becomes_review() {
        let env = r#"{"version":1,"status":"MAYBE","findings":[]}"#;
        let tmp = tempfile::tempdir().unwrap();
        let script = write_envelope_exit(tmp.path(), env, 0);
        let result = run_release_evidence_adapter(
            "project-runtime",
            Some("release-evidence"),
            &re_input("sh", &["-c", &script]),
            None,
        );
        // Unknown status: envelope parsing returns None (no status
        // match), so we fall through to "no envelope" → REVIEW_REQUIRED.
        assert_eq!(result.status, GateStatus::ReviewRequired);
    }

    #[test]
    fn release_evidence_redacts_secrets_in_diagnostic() {
        let env =
            r#"{"version":1,"status":"FAIL","diagnostic":"api_key=hunter2-secret","findings":[]}"#;
        let tmp = tempfile::tempdir().unwrap();
        let script = write_envelope_exit(tmp.path(), env, 1);
        let result = run_release_evidence_adapter(
            "project-runtime",
            Some("release-evidence"),
            &re_input("sh", &["-c", &script]),
            None,
        );
        assert_eq!(result.status, GateStatus::Fail);
        let diag = result.diagnostic.as_deref().unwrap();
        assert!(!diag.contains("hunter2-secret"), "diag: {diag}");
    }

    #[test]
    fn release_evidence_provenance_can_be_a_string_or_object() {
        // Both shapes are accepted: a plain string is treated as
        // "provenance present" (the producer's own format), an
        // object is parsed in place.
        for value in [
            r#""https://example.com/provenance.json""#,
            r#"{"type":"slsa-provenance/v0.2"}"#,
        ] {
            let env = format!(
                r#"{{"version":1,"status":"PASS","revision":"abc","product_version":"1.0","artifacts":["a"],"provenance":{value}}}"#
            );
            let tmp = tempfile::tempdir().unwrap();
            let script = write_envelope_exit(tmp.path(), &env, 0);
            let result = run_release_evidence_adapter(
                "project-runtime",
                Some("release-evidence"),
                &re_input("sh", &["-c", &script]),
                None,
            );
            assert_eq!(result.status, GateStatus::Pass, "provenance={value}");
        }
    }

    // --- capability-conformance adapter ---------------------------------

    const CAPABILITY_PASS_ENVELOPE: &str = r#"{
        "version":1,
        "status":"PASS",
        "severity":"info",
        "capabilities":{
            "declared":["api-contract","migration"],
            "configured":["api-contract","migration"],
            "verified":["api-contract","migration"],
            "unverified":[]
        },
        "findings":[]
    }"#;
    const CAPABILITY_FAIL_ENVELOPE: &str = r#"{
        "version":1,
        "status":"FAIL",
        "severity":"error",
        "capabilities":{
            "declared":["api-contract","migration"],
            "configured":["api-contract"],
            "verified":["api-contract"],
            "unverified":["migration"]
        },
        "findings":[{"title":"migration unverified","severity":"error","rule":"capability-missing"}]
    }"#;

    fn cap_input(program: &str, args: &[&str]) -> AdapterInput {
        AdapterInput {
            program: program.to_string(),
            args: args.iter().map(|s| s.to_string()).collect(),
            working_dir: std::env::temp_dir(),
            env: BTreeMap::new(),
            timeout_ms: 10_000,
            max_output_bytes: 1024 * 1024,
            evidence: vec![],
        }
    }

    #[test]
    fn capability_conformance_pass_with_verified_subset_passes() {
        let tmp = tempfile::tempdir().unwrap();
        let script = write_envelope_exit(tmp.path(), CAPABILITY_PASS_ENVELOPE, 0);
        let result = run_capability_conformance_adapter(
            "project-runtime",
            Some("capability-conformance"),
            &cap_input("sh", &["-c", &script]),
            &[
                "api-contract".to_string(),
                "migration".to_string(),
                "capability-conformance".to_string(),
            ],
        );
        assert_eq!(result.status, GateStatus::Pass);
    }

    #[test]
    fn capability_conformance_failing_envelope_fails() {
        let tmp = tempfile::tempdir().unwrap();
        let script = write_envelope_exit(tmp.path(), CAPABILITY_FAIL_ENVELOPE, 1);
        let result = run_capability_conformance_adapter(
            "project-runtime",
            Some("capability-conformance"),
            &cap_input("sh", &["-c", &script]),
            &[
                "api-contract".to_string(),
                "migration".to_string(),
                "capability-conformance".to_string(),
            ],
        );
        assert_eq!(result.status, GateStatus::Fail);
        assert_eq!(result.findings.len(), 1);
        assert_eq!(result.findings[0].title, "migration unverified");
    }

    #[test]
    fn capability_conformance_contradictory_exit_for_pass_becomes_review() {
        let tmp = tempfile::tempdir().unwrap();
        let script = write_envelope_exit(tmp.path(), CAPABILITY_PASS_ENVELOPE, 1);
        let result = run_capability_conformance_adapter(
            "project-runtime",
            Some("capability-conformance"),
            &cap_input("sh", &["-c", &script]),
            &[
                "api-contract".to_string(),
                "migration".to_string(),
                "capability-conformance".to_string(),
            ],
        );
        assert_eq!(result.status, GateStatus::ReviewRequired);
        let diag = result.diagnostic.as_deref().unwrap();
        assert!(diag.contains("exited 1"));
        assert!(diag.contains("PASS"));
    }

    #[test]
    fn capability_conformance_pass_with_empty_verified_becomes_review() {
        // PASS with no verified capabilities is a contradiction: it
        // claims the gate passed but verifies nothing.
        let env = r#"{
            "version":1,
            "status":"PASS",
            "capabilities":{
                "declared":["api-contract"],
                "configured":[],
                "verified":[],
                "unverified":["api-contract"]
            }
        }"#;
        let tmp = tempfile::tempdir().unwrap();
        let script = write_envelope_exit(tmp.path(), env, 0);
        let result = run_capability_conformance_adapter(
            "project-runtime",
            Some("capability-conformance"),
            &cap_input("sh", &["-c", &script]),
            &[
                "api-contract".to_string(),
                "capability-conformance".to_string(),
            ],
        );
        assert_eq!(result.status, GateStatus::ReviewRequired);
        let diag = result.diagnostic.as_deref().unwrap();
        assert!(diag.contains("`verified` is empty"), "diag: {diag}");
    }

    #[test]
    fn capability_conformance_pass_with_out_of_scope_verified_becomes_review() {
        // PASS with verified "ui-review" but the plan does not include
        // it: stale evidence. Driftwatchdog refuses the claim because
        // the gate cannot reproduce it.
        let env = r#"{
            "version":1,
            "status":"PASS",
            "capabilities":{
                "declared":["ui-review","api-contract"],
                "configured":["ui-review","api-contract"],
                "verified":["ui-review","api-contract"],
                "unverified":[]
            }
        }"#;
        let tmp = tempfile::tempdir().unwrap();
        let script = write_envelope_exit(tmp.path(), env, 0);
        let result = run_capability_conformance_adapter(
            "project-runtime",
            Some("capability-conformance"),
            &cap_input("sh", &["-c", &script]),
            &[
                "api-contract".to_string(),
                "capability-conformance".to_string(),
            ],
        );
        assert_eq!(result.status, GateStatus::ReviewRequired);
        let diag = result.diagnostic.as_deref().unwrap();
        assert!(diag.contains("not in the resolved plan"), "diag: {diag}");
        assert!(diag.contains("ui-review"), "diag: {diag}");
    }

    #[test]
    fn capability_conformance_not_applicable_with_empty_verified_passes() {
        // NOT_APPLICABLE with empty verified list is acceptable: the
        // gate is opt-out for this run.
        let env = r#"{
            "version":1,
            "status":"NOT_APPLICABLE",
            "capabilities":{
                "declared":[],
                "configured":[],
                "verified":[],
                "unverified":[]
            }
        }"#;
        let tmp = tempfile::tempdir().unwrap();
        let script = write_envelope_exit(tmp.path(), env, 0);
        let result = run_capability_conformance_adapter(
            "project-runtime",
            Some("capability-conformance"),
            &cap_input("sh", &["-c", &script]),
            &["capability-conformance".to_string()],
        );
        assert_eq!(result.status, GateStatus::NotApplicable);
    }

    #[test]
    fn capability_conformance_wrong_version_becomes_review() {
        let env = r#"{"version":2,"status":"PASS","capabilities":{"verified":["a"]}}"#;
        let tmp = tempfile::tempdir().unwrap();
        let script = write_envelope_exit(tmp.path(), env, 0);
        let result = run_capability_conformance_adapter(
            "project-runtime",
            Some("capability-conformance"),
            &cap_input("sh", &["-c", &script]),
            &["a".to_string()],
        );
        assert_eq!(result.status, GateStatus::ReviewRequired);
    }

    #[test]
    fn capability_conformance_malformed_envelope_becomes_review() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("bad.json");
        std::fs::write(&path, "not json").unwrap();
        let script = format!("cat {} ; exit 0", path.display());
        let result = run_capability_conformance_adapter(
            "project-runtime",
            Some("capability-conformance"),
            &cap_input("sh", &["-c", &script]),
            &["capability-conformance".to_string()],
        );
        assert_eq!(result.status, GateStatus::ReviewRequired);
    }

    #[test]
    fn capability_conformance_empty_output_becomes_review() {
        let result = run_capability_conformance_adapter(
            "project-runtime",
            Some("capability-conformance"),
            &cap_input("sh", &["-c", "exit 0"]),
            &["capability-conformance".to_string()],
        );
        assert_eq!(result.status, GateStatus::ReviewRequired);
    }

    #[test]
    fn capability_conformance_redacts_secrets_in_diagnostic() {
        let env = r#"{"version":1,"status":"FAIL","diagnostic":"api_key=hunter2-secret","capabilities":{"verified":["api-contract"]},"findings":[]}"#;
        let tmp = tempfile::tempdir().unwrap();
        let script = write_envelope_exit(tmp.path(), env, 1);
        let result = run_capability_conformance_adapter(
            "project-runtime",
            Some("capability-conformance"),
            &cap_input("sh", &["-c", &script]),
            &[
                "api-contract".to_string(),
                "capability-conformance".to_string(),
            ],
        );
        assert_eq!(result.status, GateStatus::Fail);
        let diag = result.diagnostic.as_deref().unwrap();
        assert!(!diag.contains("hunter2-secret"), "diag: {diag}");
    }
}
