## Context

Adapters are integration boundaries, not domain logic. They must isolate tool
failures, preserve raw artifacts safely, and normalize findings into the generic
Gate contract.

## Goals / Non-Goals

**Goals:**

- Define adapter lifecycle, capability declaration, timeout, and failure mapping.
- Parse JSON/SARIF where available and retain bounded text diagnostics otherwise.
- Keep evaluator logic independent from process invocation.

**Non-Goals:**

- Reimplementing Semgrep, Gitleaks, OSV, Playwright, or analyzers.
- Making AI decisions.

## Decisions

- An adapter declares tool id, supported execution modes, input needs, output
  formats, and required evidence.
- Adapter execution returns a normalized result plus evidence references; it does
  not decide aggregate blocking policy.
- Nonzero exit, timeout, signal, malformed output, and missing executable remain
  distinguishable diagnostics.
- Deterministic evaluators consume normalized findings and rules, not process
  objects or tool-specific APIs.
- Each adapter is isolated so one failure does not abort other checks.

## Risks / Trade-offs

- [Risk] Tool output formats drift → [Mitigation] fixture-based contract tests
  and tolerant parsing with explicit protocol errors.
- [Risk] Adapter sprawl → [Mitigation] capability declarations and small public
  adapter facade.
- [Risk] Tool exit code semantics differ → [Mitigation] normalize exit, findings,
  and evidence separately.
