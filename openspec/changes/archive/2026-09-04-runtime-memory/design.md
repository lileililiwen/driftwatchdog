## Context

`driftwatch run cargo test`, `driftwatch run pytest`, `driftwatch run npm test`, and equivalent commands must share one execution contract. The runner must not reinterpret arguments or require a known language.

## Goals / Non-Goals

**Goals:**
- Run arbitrary executable plus arguments from a selected working directory.
- Record command, arguments, timestamps, duration, exit code, output, tags, and Git context.
- Persist successful runs lightly and failed runs with analysis-ready output.
- Provide recent-run and recurring-failure query surfaces.

**Non-Goals:**
- Failure normalization and fingerprint grouping, which is a later change; the runner may store raw output and a provisional failure reference.

## Decisions

Use an argument vector rather than shell interpolation to avoid changing quoting semantics and reduce injection risk. Stream child output to the user while capturing bounded data according to config. Preserve the child's exit code for the CLI process. A tag is optional and repeatable. `top` consumes grouped bug data once fingerprinting is present and shows a clear empty state before then.

## Risks / Trade-offs

Capturing output can increase memory use, so the runner must enforce a configurable limit and mark truncation. Exact terminal parity is platform-dependent; output should remain readable and exit behavior deterministic.

## ADDED Requirements

### Requirement: Run arbitrary commands
The CLI MUST provide `driftwatch run [--tag <tag>]... <program> [args...]`, execute the supplied program without language-specific assumptions, stream output where possible, persist a run record, and return the child exit code.

#### Scenario: Successful command
- **WHEN** a wrapped command exits with code 0
- **THEN** Driftwatch records a successful run with command, arguments, working directory, timing, Git context, and exit code 0, then exits 0

#### Scenario: Failed command
- **WHEN** a wrapped command exits nonzero
- **THEN** Driftwatch records failure status and captured stdout/stderr, reports failure to the terminal, and exits with the child's nonzero status

#### Scenario: Missing executable
- **WHEN** the requested program cannot be started
- **THEN** Driftwatch records a start failure with diagnostic context and exits nonzero without panicking

### Requirement: Query recent runs
`driftwatch list` MUST show recent runs with time, command, status, and associated bug identifier when available. It MUST support `--limit`, `--failed`, and `--tag` filters.

#### Scenario: Filter failed tagged runs
- **WHEN** a user runs `driftwatch list --failed --tag auth --limit 20`
- **THEN** only matching failed runs are shown, newest first, capped at 20

### Requirement: Show recurring failures
`driftwatch top` MUST show grouped recurring failures with count, first seen, and last seen, and MUST support `--limit`, `--days`, and `--tag`.

#### Scenario: No recurring failures
- **WHEN** no failed run has a fingerprint in the selected window
- **THEN** the command exits successfully with an explicit empty-state message
