# Proposal: Gate adapters and evaluation

## Why

Different tools emit JSON, SARIF, text, screenshots, and traces. Driftwatchdog
needs stable adapter and evaluator contracts so tools can be replaced without
changing Gate Core.

## What Changes

- Define adapter input/output contracts and isolated execution.
- Add deterministic evaluator and finding normalization.
- Add initial adapters for existing checkers plus Gitleaks, OSV, Semgrep, and
  project-runtime commands as separately verifiable integrations.

## Capabilities

### New Capabilities

- `gate-adapter-evaluation`: tool adapters and deterministic evaluation.

### Modified Capabilities

## Impact

Affects new adapter modules, process runner integration, evidence artifacts,
configuration, result persistence, and tests. Depends on gate contract,
configuration, evidence, and toolchain execution.

## Non-Goals

- No copied scanner implementation.
- No AI evaluator in this change.
