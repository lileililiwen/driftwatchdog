## 1. BFS: Impact and Structure

- [x] 1.1 Map evaluator, evidence, configuration, doctor, redaction, storage, and report boundaries.
- [x] 1.2 Define provider-neutral request/response schema, status policy, metadata, and timeout budget.
- [x] 1.3 Define secret handling, artifact preview policy, and offline/no-provider behavior.

## 2. DFS: Requirement Implementation

- [x] 2.1 Implement AI evaluator contract and schema validation.
- [x] 2.2 Implement provider configuration and bounded invocation boundary without embedding an LLM.
- [x] 2.3 Implement redaction, evidence references, provider diagnostics, and result persistence.
- [x] 2.4 Add tests for valid, invalid, missing evidence, missing provider, timeout, and secret cases.

## 3. BFS: Regression and Completion

- [x] 3.1 Re-check deterministic evaluator precedence and blocking aggregation.
- [x] 3.2 Re-check no provider call occurs when AI is disabled or no applicable concern exists.
- [x] 3.3 Re-check persisted output does not contain secrets or unbounded prompts/artifacts.
- [x] 3.4 Run full Rust gates, strict OpenSpec validation, and local verification.
