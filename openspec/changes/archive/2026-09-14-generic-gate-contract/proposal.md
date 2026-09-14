# Proposal: Generic Gate Contract

## Why

Driftwatchdog has a checker protocol for external alerts but no general contract
for gate plans, evidence, findings, and `PASS`/`FAIL`/`REVIEW_REQUIRED` results.
The contract must be language-agnostic and must not depend on OpenSpec.

## What Changes

- Add generic gate context, plan, evidence, finding, result, and status models.
- Define deterministic aggregation and blocking semantics.
- Preserve the existing checker protocol as a compatibility surface.

## Capabilities

### New Capabilities

- `generic-gate-contract`: stable internal and serialized gate protocol.

### Modified Capabilities

## Impact

Affects new `src/gate/` and `src/evidence/` modules, JSON DTOs, CLI-facing
serialization, tests, and future adapters. No tool installation or AI provider
is introduced.

## Non-Goals

- No OpenSpec dependency.
- No scanner implementation.
- No network calls, tool downloads, or LLM calls.
