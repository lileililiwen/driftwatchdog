## Context

The current `checker` module runs a configured child process and parses alert
JSON. A comprehensive Gate layer needs richer but stable types while retaining
language and specification-system neutrality.

## Goals / Non-Goals

**Goals:**

- Define bounded, serializable gate plans and results.
- Distinguish evidence, findings, diagnostics, and remediation.
- Make `REVIEW_REQUIRED` fail closed when configured as blocking.
- Allow old checker results to be adapted without breaking storage.

**Non-Goals:**

- Tool lifecycle, adapters, AI, context providers, or UI behavior.

## Decisions

- `GateStatus` contains `Pass`, `Fail`, `ReviewRequired`, and `NotApplicable`.
- `GateResult` carries gate id, source, status, severity, findings, evidence
  references, missing evidence, diagnostics, and remediation.
- `Evidence` is bounded metadata plus an artifact reference; secrets and raw
  unbounded output are not embedded in the result contract.
- `GatePlan` is resolved before execution and lists selected checks and policy.
- Aggregation is deterministic: any blocking `Fail` blocks; configured blocking
  `ReviewRequired` blocks; `NotApplicable` does not pass a required check.
- JSON output is versioned and independent of OpenSpec.

## Risks / Trade-offs

- [Risk] Result model grows too quickly → [Mitigation] keep core fields small and
  put adapter-specific data in bounded extensions.
- [Risk] Existing alert meaning changes → [Mitigation] retain the current
  checker protocol adapter and snapshot semantics.
- [Risk] Evidence leaks secrets → [Mitigation] redact, cap, hash, and reference
  artifacts instead of embedding them.
