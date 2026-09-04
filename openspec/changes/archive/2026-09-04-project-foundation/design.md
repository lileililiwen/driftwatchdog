## Context

The repository is initially empty. All later changes depend on a predictable project root, configuration, durable identifiers, and tables for runs, bugs, occurrences, check snapshots, alerts, correlations, and manual links.

## Goals / Non-Goals

**Goals:**
- Provide `driftwatch init` and a usable Rust CLI skeleton.
- Store state locally in `.driftwatch/state.db`; create `driftwatch.toml` and recommend ignoring state in Git.
- Define migrations and repository interfaces that later features can use without coupling to SQL details.
- Capture Git commit, branch, and dirty status when available.

**Non-Goals:**
- Running commands, fingerprinting output, checker execution, reports, or network services.

## Decisions

Use SQLite with explicit migrations and UTC timestamps. Initialization is idempotent and must not overwrite an existing config or database. Git inspection is best-effort: a non-Git directory is a valid project, with missing metadata represented explicitly. The monitored project language is never inferred as a restriction; the runner and storage contracts accept arbitrary commands.

## Risks / Trade-offs

SQLite keeps the single-binary local-first model simple but requires migration discipline. Git commands may be unavailable or fail; callers receive optional metadata and a diagnostic rather than initialization failure.

## ADDED Requirements

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
