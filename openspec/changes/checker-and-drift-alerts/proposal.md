## Why

Spec validation belongs to existing tools, but their output needs a stable local shape before it can be retained or correlated with runtime failures.

## What Changes

Add configured external checkers, a versioned JSON input protocol, adapters into `DriftAlert`, `driftwatch check`, check snapshots, and resilient failure handling.

## Capabilities

### New Capabilities
- `checker-and-drift-alerts`: execute configured checkers and persist normalized spec drift alerts.

### Modified Capabilities
- `export-and-doctor`: include snapshots and alerts in export and checker diagnostics.

## Impact

Adds TOML checker configuration, subprocess execution, JSON parsing, adapter boundaries, and new SQLite writes. It does not define a spec format or implement a checker.
