## Why

Users and automation must be able to inspect Driftwatch without being locked into SQLite, and local failures need actionable diagnostics.

## What Changes

Add `driftwatch export json|jsonl|markdown` and `driftwatch doctor`, with stable serializable records and checks for SQLite, Git, configuration, checker installation, versions, and directory permissions.

## Capabilities

### New Capabilities
- `export-and-doctor`: portable data export and local environment diagnostics.

### Modified Capabilities
- `project-foundation`: expose configuration and environment checks.

## Impact

Adds versioned export schemas and diagnostics output suitable for humans and scripts. Export is local and read-only.
