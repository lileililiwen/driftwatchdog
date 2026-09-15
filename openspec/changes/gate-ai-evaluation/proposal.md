# Proposal: Provider-neutral AI evaluation

## Why

Some concerns such as UX, architecture, privacy interpretation, and responsive
layout cannot be proven mechanically. Driftwatchdog needs an evidence-based AI
evaluator contract without making an LLM provider mandatory or embedding one in
the core.

## What Changes

- Define an AI evaluator request/response contract.
- Require evidence, rule ids, confidence, missing evidence, and remediation.
- Add provider configuration and fail-closed review semantics.

## Capabilities

### New Capabilities

- `gate-ai-evaluation`: optional provider-neutral semantic evaluation.

### Modified Capabilities

## Impact

Affects evaluators, evidence, configuration, doctor, redaction, reporting, and
optional external provider adapters. Depends on gate contract, evidence,
configuration, and context providers.

## Non-Goals

- No built-in LLM, API key storage, or mandatory network call.
- No free-form AI PASS decision.
