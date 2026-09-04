## 1. Protocol and configuration

- [x] 1.1 Define checker config types, command execution limits, and forward-compatible input/output DTOs.
- [x] 1.2 Write parser tests for valid alerts, empty alerts, missing required fields, optional fields, and unknown fields.
- [x] 1.3 Implement the normalized `DriftAlert` adapter boundary independent of checker names or tool internals.

## 2. Execution and persistence

- [x] 2.1 Write failure-isolation tests for missing executable, nonzero exit, timeout, malformed JSON, and subsequent checker execution.
- [x] 2.2 Implement configured checker execution with bounded output and per-checker diagnostics.
- [x] 2.3 Persist snapshots and alerts transactionally, including checker, commit, timestamps, status, and raw diagnostic excerpt.
- [x] 2.4 Implement `driftwatch check` output and nonzero semantics that make failures visible without suppressing successful results.

## 3. Verification

- [x] 3.1 Test multiple checkers in declaration order and verify one failure does not cancel later checkers.
- [x] 3.2 Test export and doctor integration for snapshots, alerts, executable discovery, and versions.
- [x] 3.3 Verify no checker command causes network access unless the user explicitly configures a command that does so.
