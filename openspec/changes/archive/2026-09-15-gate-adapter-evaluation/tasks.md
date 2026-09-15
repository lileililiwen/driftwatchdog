## 1. BFS: Impact and Structure

- [x] 1.1 Map existing checker protocol/runner/report types and identify reusable process-hardening behavior.
- [x] 1.2 Define adapter capability, input, output, timeout, artifact, and failure contracts.
- [x] 1.3 Define fixtures and supported output formats for each initial tool.

## 2. DFS: Requirement Implementation

- [x] 2.1 Add adapter registry and capability validation.
- [x] 2.2 Implement existing checker compatibility adapter.
- [x] 2.3 Implement Gitleaks, OSV, Semgrep, and project-runtime adapters through CLI/container boundaries.
- [x] 2.4 Implement JSON/SARIF normalization and deterministic evaluator rules.
- [x] 2.5 Add isolated success, malformed, timeout, nonzero, signal, and finding tests.

## 3. BFS: Regression and Completion

- [x] 3.1 Re-check checker failure isolation, output caps, process-group kill, and exit-code semantics.
- [x] 3.2 Re-check tool versions, fixture drift, evidence references, and result aggregation.
- [x] 3.3 Verify no scanner implementation or SDK coupling entered the core.
- [x] 3.4 Run adapter fixtures, full Rust gates, strict OpenSpec validation, and local verification.
