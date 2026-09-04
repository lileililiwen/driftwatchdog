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
