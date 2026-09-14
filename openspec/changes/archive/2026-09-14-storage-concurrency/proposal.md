# Proposal: Storage concurrency and query correctness

## Why
Audit found races and silent data errors: migration tracking-table creation + `MAX(version)` read outside a transaction allows concurrent double-apply, and `ALTER TABLE ADD COLUMN` (`src/storage/schema.rs:100-119`) is not `IF NOT EXISTS` so a crash mid-migration is not rerunnable; `ProjectRoot::discover` (`src/project/root.rs:54`) has check-then-use `is_dir()/is_file` races, no canonicalization (symlink aliasing), and silent fallback when `MAX_WALK_DEPTH` overflows; `GitContext::capture` (`src/project/git.rs`) reports `dirty=Some(false)` when git is missing and spawns 3 sequential gits with no timeout; `doctor` mutates the DB (`Db::open` runs migrations, `doctor/mod.rs:120-147`), its writability probe (`:227-230`) races and can leave files, and `check_one_checker` (`:264,276`) whitespace-splits and runs `program --version` with no timeout; tag filters use unescaped `%"tag"%` LIKE (`src/repo/runs.rs:210`, `src/repo/bugs.rs:392-401`, `top.rs:16`, `report.rs:30`) so `auth` matches `oauth` and `%_,"` act as wildcards; `upsert_for_occurrence` (`src/repo/bugs.rs:267-274`) mis-handles backdated `seen_at`; `report trend_cells` (`src/commands/report.rs:108-117`) swallows DB errors, does N+1 queries, and uses UTC-day string ranges missing non-`Z` offsets; `doctor:322-325` and `check:102-104` swallow correlation/DB errors; `export build.rs:29` silently caps at 100k rows.

## What Changes
- Transactional, rerunnable migrations; canonicalized project-root discovery; bounded non-fatal git capture with unknown (not clean) on absence; read-only doctor with atomic probe and bounded version checks; escaped LIKE tag filters; correct backdated occurrence accounting; strict error propagation + efficient trend queries; explicit export cap warning.
- No schema version bump unless required; prefer idempotent DDL.

## Capabilities
### New Capabilities
- `storage-concurrency`: race-free migrations, root discovery, git capture, read-only doctor, correct tag/occurrence/trend/export semantics.
### Modified Capabilities
- `project-foundation`: init/root/git semantics.
- `runtime-memory`: list/top/report filtering.
- `export-and-doctor`: export cap + doctor diagnostics.

## Impact
Affects: `src/storage/migrations.rs`, `src/storage/schema.rs`, `src/project/root.rs`, `src/project/git.rs`, `src/doctor/mod.rs`, `src/repo/runs.rs`, `src/repo/bugs.rs`, `src/commands/top.rs`, `src/commands/report.rs`, `src/commands/check.rs`, `src/export/build.rs`, `tests/storage.rs`, `tests/git.rs`, `tests/doctor.rs`, `tests/export.rs`.
