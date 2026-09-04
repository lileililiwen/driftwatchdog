# project-foundation Specification

## Purpose
TBD - created by archiving change project-foundation. Update Purpose after archive.
## Requirements
### Requirement: Initialize local project state
The CLI MUST provide `driftwatch init`, create `.driftwatch/`, create the SQLite database and required schema, create `driftwatch.toml` when absent, and report the project root. Re-running init MUST preserve existing user files and schema data.

#### Scenario: Initialize a repository
- **WHEN** a user runs `driftwatch init` in a writable project directory
- **THEN** the command creates `.driftwatch/state.db`, applies all migrations, writes a default config only if absent, and exits successfully

#### Scenario: Initialize outside Git
- **WHEN** the current directory is not a Git worktree
- **THEN** initialization succeeds and later run records represent Git metadata as unavailable

### Requirement: Provide foundational persistence
The schema MUST define durable entities for runs, bug fingerprints, bug occurrences, check snapshots, drift alerts, correlations, and manual links, with foreign-key relationships and migration versioning.

#### Scenario: Open an initialized database
- **WHEN** a later command opens the state database
- **THEN** all foundation tables exist at the current migration version and opening does not recreate or delete records

### Requirement: Capture optional Git context
The Git abstraction MUST expose commit, branch, and dirty status as optional values and MUST not make command execution dependent on Git availability.

#### Scenario: Git inspection fails
- **WHEN** Git is missing or a Git query fails
- **THEN** the caller receives unavailable metadata plus a diagnostic suitable for a warning, not a fatal initialization error

