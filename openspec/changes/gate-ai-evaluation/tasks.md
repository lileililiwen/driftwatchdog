## 1. BFS: Impact and Structure

- [ ] 1.1 Map evaluator, evidence, configuration, doctor, redaction, storage, and report boundaries.
- [ ] 1.2 Define provider-neutral request/response schema, status policy, metadata, and timeout budget.
- [ ] 1.3 Define secret handling, artifact preview policy, and offline/no-provider behavior.

## 2. DFS: Requirement Implementation

- [ ] 2.1 Implement AI evaluator contract and schema validation.
- [ ] 2.2 Implement provider configuration and bounded invocation boundary without embedding an LLM.
- [ ] 2.3 Implement redaction, evidence references, provider diagnostics, and result persistence.
- [ ] 2.4 Add tests for valid, invalid, missing evidence, missing provider, timeout, and secret cases.

## 3. BFS: Regression and Completion

- [ ] 3.1 Re-check deterministic evaluator precedence and blocking aggregation.
- [ ] 3.2 Re-check no provider call occurs when AI is disabled or no applicable concern exists.
- [ ] 3.3 Re-check persisted output does not contain secrets or unbounded prompts/artifacts.
- [ ] 3.4 Run full Rust gates, strict OpenSpec validation, and local verification.
