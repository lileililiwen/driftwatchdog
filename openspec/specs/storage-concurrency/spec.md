# storage-concurrency Specification

## Purpose
TBD - created by archiving change storage-concurrency. Update Purpose after archive.
## Requirements
### Requirement: Transactional rerunnable migrations
Migrations MUST apply atomically and MUST be safe to rerun after a crash or under concurrent process startup.

#### Scenario: Concurrent first-run
- **WHEN** two `driftwatch run` processes open a fresh DB simultaneously
- **THEN** exactly one migration set applies and neither process errors on duplicate columns

#### Scenario: Crash mid-migration
- **WHEN** the process dies after `MIGRATION_0002` partially applied
- **THEN** the next open completes without `duplicate column` errors

### Requirement: Canonical project root
Root discovery MUST canonicalize symlinks and MUST NOT silently fall back on depth overflow or TOCTOU races.

#### Scenario: Symlinked workdir
- **WHEN** cwd is reached via a symlink
- **THEN** state resolves to the canonical project root consistently

### Requirement: Honest git context
Git capture MUST be bounded and MUST report unknown (not clean) when git is missing or times out.

#### Scenario: Git missing
- **WHEN** `git` is not on PATH
- **THEN** runs record `dirty=None` (unknown), never `Some(false)`

#### Scenario: Slow git
- **WHEN** git hangs beyond the budget
- **THEN** the CLI still proceeds within ~2s with unknown git context

### Requirement: Read-only doctor
`driftwatch doctor` MUST NOT migrate or modify the DB; its probes MUST be race-free and bounded.

#### Scenario: Doctor on prod DB
- **WHEN** `driftwatch doctor` runs
- **THEN** the DB schema version and mtime are unchanged afterwards

#### Scenario: Version probe hang
- **WHEN** a checker's `--version` hangs
- **THEN** doctor reports timeout instead of hanging

### Requirement: Exact tag filtering
Tag filters MUST match whole tags, with LIKE wildcards escaped.

#### Scenario: auth vs oauth
- **WHEN** runs are tagged `oauth` and the user filters `--tag auth`
- **THEN** no `oauth` rows are returned; `--tag oauth` returns them; tags containing `%_\"` match literally

### Requirement: Correct occurrence accounting
Backdated occurrences MUST increment the count and move `first_seen` earlier without freezing `last_seen`.

#### Scenario: Backdated seen_at
- **WHEN** an occurrence arrives with an older `seen_at`
- **THEN** `occurrence_count` increments, `first_seen` moves earlier, `last_seen` stays at the max

### Requirement: Strict trend and export semantics
Report trend cells MUST propagate DB errors and use one aggregate query with timezone-correct buckets; export MUST warn when the 100k cap truncates.

#### Scenario: Export cap hit
- **WHEN** the DB holds >100k exportable rows
- **THEN** `driftwatch export json` prints a `capped at 100000` warning to stderr and documents the cap

#### Scenario: Non-Z offset runs
- **WHEN** runs carry non-UTC offsets
- **THEN** the 7-day trend attributes them to the correct local day

