# Proposal: Evidence and artifact model

## Why

Gate decisions need traceable evidence such as source locations, command output,
screenshots, traces, and raw tool artifacts. The current checker path stores
bounded output but has no general evidence lifecycle or provenance model.

## What Changes

- Add evidence bundles, artifact metadata, provenance, redaction, and retention.
- Persist references and hashes rather than unbounded payloads in results.
- Make evidence available to deterministic and optional AI evaluators.

## Capabilities

### New Capabilities

- `evidence-and-artifacts`: bounded, traceable Gate evidence.

### Modified Capabilities

## Impact

Affects storage schema, export DTOs, cleanup, result reporting, and process
capture. Depends on `generic-gate-contract`.

## Non-Goals

- No browser automation implementation.
- No AI provider implementation.
