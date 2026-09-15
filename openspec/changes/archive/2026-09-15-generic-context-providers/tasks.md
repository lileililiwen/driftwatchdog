## 1. BFS: Impact and Structure

- [x] 1.1 Map Git capture, project root confinement, checker context, reports, and future evaluator inputs.
- [x] 1.2 Define provider, context document, digest, truncation, and unavailable-context contracts.
- [x] 1.3 Define OpenSpec adapter scope and prove core modules do not depend on OpenSpec types.

## 2. DFS: Requirement Implementation

- [x] 2.1 Implement generic context provider registry and bounded document model.
- [x] 2.2 Implement Git and project-file providers.
- [x] 2.3 Implement optional OpenSpec provider using generic output only.
- [x] 2.4 Add configuration/CLI selection and context diagnostics.
- [x] 2.5 Add tests for Git-only, OpenSpec, missing, escape, truncation, and mutation cases.

## 3. BFS: Regression and Completion

- [x] 3.1 Re-check no OpenSpec dependency enters generic gate/core modules.
- [x] 3.2 Re-check path confinement, size bounds, redaction, and read-only behavior.
- [x] 3.3 Re-check reports and exports remain valid when no context provider is configured.
- [x] 3.4 Run full Rust gates, strict OpenSpec validation, and local verification.
