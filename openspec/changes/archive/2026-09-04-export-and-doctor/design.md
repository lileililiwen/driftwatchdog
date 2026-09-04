## Context

SQLite is an implementation detail. Export enables issue creation, dashboards, AI agents, and debugging. `doctor` should identify setup problems without making external writes or requiring every optional tool.

## Goals / Non-Goals

**Goals:**
- Export runs, bugs, occurrences, snapshots, alerts, correlations, and links as JSON, JSONL, or Markdown.
- Provide pass/warn/fail checks with remediation text for local dependencies and permissions.
- Keep export schema explicit and forward-compatible.

**Non-Goals:**
- Upload, sync, remote dashboards, or automatically installing missing tools.

## Decisions

Use one top-level JSON document for `json`, one record per line for `jsonl`, and a human-readable Markdown projection. Include a schema version. Doctor treats optional checkers as warnings when not configured, and reports checker versions only when executable discovery succeeds.

## Risks / Trade-offs

Exporting raw logs may expose secrets; users explicitly invoke export, and output should identify truncation. Diagnostics must avoid leaking environment secrets and should report paths only when useful.

## ADDED Requirements

### Requirement: Export portable data
`driftwatch export json|jsonl|markdown` MUST export persisted Driftwatch data without requiring direct SQLite access, include a schema version, and preserve stable IDs and timestamps.

#### Scenario: Export JSON
- **WHEN** a user runs `driftwatch export json`
- **THEN** valid versioned JSON is written to stdout and contains the available project, run, bug, alert, correlation, and link records

#### Scenario: Export JSONL
- **WHEN** a user runs `driftwatch export jsonl`
- **THEN** each output line is a valid self-contained record with a type discriminator and stable identifier

### Requirement: Diagnose local installation
`driftwatch doctor` MUST check database accessibility/schema, config validity, Git availability, configured checker executables and versions, and read/write access to required directories.

#### Scenario: Missing optional checker
- **WHEN** a configured checker executable is unavailable
- **THEN** doctor reports a failed checker check with command and remediation while completing all other checks

#### Scenario: Healthy installation
- **WHEN** all required local checks pass
- **THEN** doctor exits 0 and prints a concise pass summary
