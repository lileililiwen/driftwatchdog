## 1. BFS: Impact and Structure

- [x] 1.1 Map existing output caps, SQLite tables, exports, GC, fingerprints, and report consumers.
- [x] 1.2 Define artifact classes, metadata, digest, redaction, retention, and state-root confinement.
- [x] 1.3 Define migrations and backward-compatible export behavior.

## 2. DFS: Requirement Implementation

- [x] 2.1 Add evidence/artifact domain types and storage tables.
- [x] 2.2 Implement bounded artifact writes, hashing, path confinement, and redaction.
- [x] 2.3 Link gate results to evidence and expose safe previews.
- [x] 2.4 Extend GC/export/report behavior for retained evidence metadata.
- [x] 2.5 Add tests for secrets, escapes, caps, cleanup, and unavailable artifacts.

## 3. BFS: Regression and Completion

- [x] 3.1 Re-check schema migrations, existing run/check snapshots, exports, and MCP read paths.
- [x] 3.2 Re-check UTF-8 boundaries, permission failures, disk failures, and concurrent writes.
- [x] 3.3 Verify cleanup never removes required identity or leaves state outside `.driftwatch/`.
- [x] 3.4 Run full Rust gates, strict OpenSpec validation, and local verification.
