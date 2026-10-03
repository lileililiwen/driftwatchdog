# storage-concurrency Specification

## Purpose

Audit found races and silent data errors: migration tracking-table creation + `MAX(version)` read outside a transaction allows concurrent double-apply, and `ALTER TABLE ADD COLUMN` (`src/storage/schema.rs:100-119`) is not `IF NOT EXISTS` so a crash mid-migration is not rerunnable; `ProjectRoot::discover` (`src/project/root.rs:54`) has check-then-use `is_dir()/is_file` races, no canonicalization (symlink aliasing), and silent fallback when `MAX_WALK_DEPTH` overflows; `GitContext::capture` (`src/project/git.rs`) reports `dirty=Some(false)` when git is missing and spawns 3 sequential gits with no timeout; `doctor` mutates the DB (`Db::open` runs migrations, `doctor/mod.rs:120-147`), its writability probe (`:227-230`) races and can leave files, and `check_one_checker` (`:264,276`) whitespace-splits and runs `program --version` with no timeout; tag filters use unescaped `%"tag"%` LIKE (`src/repo/runs.rs:210`, `src/repo/bugs.rs:392-401`, `top.rs:16`, `report.rs:30`) so `auth` matches `oauth` and `%_,"` act as wildcards; `upsert_for_occurrence` (`src/repo/bugs.rs:267-274`) mis-handles backdated `seen_at`; `report trend_cells` (`src/commands/report.rs:108-117`) swallows DB errors, does N+1 queries, and uses UTC-day string ranges missing non-`Z` offsets; `doctor:322-325` and `check:102-104` swallow correlation/DB errors; `export build.rs:29` silently caps at 100k rows.
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

