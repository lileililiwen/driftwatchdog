## 1. BFS: Impact and Structure

- [x] 1.1 Map current config loading, root discovery, doctor, checker selection, and CLI error paths.
- [x] 1.2 Define manifest schema, precedence with `driftwatch.toml`, profile resolution, and trigger semantics.
- [x] 1.3 Define allowed local overrides, rule-pack identity, and dry-run plan output.

## 2. DFS: Requirement Implementation

- [x] 2.1 Implement manifest parsing and structural validation.
- [x] 2.2 Implement profile and explicit-check resolution.
- [x] 2.3 Implement project command and context-provider declarations.
- [x] 2.4 Implement dry-run plan rendering and actionable diagnostics.
- [x] 2.5 Add unit/integration tests for valid, invalid, conditional, and dry-run behavior.

## 3. BFS: Regression and Completion

- [x] 3.1 Re-check legacy `driftwatch.toml` behavior and checker compatibility.
- [x] 3.2 Re-check root confinement, unknown fields, empty state, and no-execution dry run.
- [x] 3.3 Verify resolved plans are deterministic and include version/policy identity.
- [x] 3.4 Run full Rust gates, strict OpenSpec validation, and local verification.
