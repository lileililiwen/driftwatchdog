## 1. BFS: Impact and Structure

- [x] 1.1 Map current CLI dispatch, checker command, SQLite schema, reports, exports, doctor, MCP, and CI template paths.
- [x] 1.2 Define `gate` command lifecycle, exit codes, dry-run, persistence, output formats, and legacy `check` compatibility.
- [x] 1.3 Define change/revision identity, manifest/rule/tool provenance, evidence references, migration, and report boundaries.
- [x] 1.4 Define local-before-archive documentation and CI-repeat semantics.

## 2. DFS: Requirement Implementation

- [x] 2.1 Add `gate` CLI arguments, dispatch, dry-run, and machine-readable output.
- [x] 2.2 Implement Gate plan execution using the shared builders and adapters.
- [x] 2.3 Add transactional Gate snapshot/result persistence and migrations.
- [x] 2.4 Extend doctor, reports, exports, and safe MCP read tools with Gate status.
- [x] 2.5 Update README, ROADMAP, HANDOFF, examples, and CI templates.
- [x] 2.6 Add local success/failure/review/dry-run/legacy integration tests.

## 3. BFS: Regression and Completion

- [x] 3.1 Re-check every existing `check`, report, export, MCP, and CI behavior.
- [x] 3.2 Re-check exit codes, partial failures, persistence transactions, redaction, and artifact retention.
- [x] 3.3 Re-check local-before-archive wording and verify CI does not become the only Gate invocation.
- [x] 3.4 Run full Rust, packaging, export, MCP, strict OpenSpec, and local Gate verification.
