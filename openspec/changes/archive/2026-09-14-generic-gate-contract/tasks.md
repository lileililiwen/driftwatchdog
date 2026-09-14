## 1. BFS: Impact and Structure

- [x] 1.1 Map current checker statuses, alert persistence, exports, reports, and CLI callers.
- [x] 1.2 Define GatePlan, GateResult, Finding, Evidence, severity, and aggregation contracts.
- [x] 1.3 Define JSON versioning, redaction, size limits, and compatibility with existing checker snapshots.
- [x] 1.4 Identify migration and export impact without coupling to OpenSpec.

## 2. DFS: Requirement Implementation

- [x] 2.1 Add generic gate domain types and versioned DTOs.
- [x] 2.2 Add deterministic aggregation and blocking policy evaluation.
- [x] 2.3 Add bounded evidence references and secret-safe diagnostics.
- [x] 2.4 Adapt existing checker outcomes to normalized results.
- [x] 2.5 Add unit and contract tests for success, failure, review, not-applicable, and compatibility scenarios.

## 3. BFS: Regression and Completion

- [x] 3.1 Re-check all checker callers, persistence, exports, reports, and MCP read paths.
- [x] 3.2 Re-run existing checker protocol tests and new result-contract tests.
- [x] 3.3 Test malformed, oversized, secret-bearing, and unknown-version result input.
- [x] 3.4 Run format, tests, clippy, strict OpenSpec validation, and local verification.
