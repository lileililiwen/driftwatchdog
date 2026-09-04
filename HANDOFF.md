# Driftwatch Handoff

## Change completion workflow

After implementing a change and ticking every box in its `tasks.md`, follow the closing sequence in `AGENTS.md` ("Change completion workflow"):

1. Verification gates: `cargo fmt --check && cargo test && cargo clippy --all-targets --all-features -- -D warnings && openspec validate --changes --strict --no-interactive`.
2. `openspec archive <change> -y --skip-specs` (drop `--skip-specs` only if the change adds or modifies a `spec.md` capability).
3. `git add -A && git commit -m "Implement <change>"`.
4. Update this file (status, next action, module map) and commit: `git add HANDOFF.md && git commit -m "Update HANDOFF after <change>"`.
5. Re-run `openspec validate --changes --strict --no-interactive`.

## Current state

`project-foundation` and `runtime-memory` are **implemented and archived** (both 2026-09-04). The `driftwatch` Rust binary builds and tests cleanly: `driftwatch init`, `driftwatch run`, `driftwatch list`, and `driftwatch top` are all functional. The SQLite schema (version 1) covers runs, fingerprints, occurrences, check_snapshots, drift_alerts, correlations, and manual_links. Both capability specs are live under `openspec/specs/`. The remaining four change packages (`fingerprinting-and-retention`, `export-and-doctor`, `checker-and-drift-alerts`, `correlation-and-ai-context`) are spec-only.

## Start here

Read these in order:

1. README.md — product positioning and user-facing command surface.
2. ROADMAP.md — release sequence and dependency graph.
3. `openspec/changes/fingerprinting-and-retention/` — **next change**: generic failure normalization, bug grouping, retention/GC. Runtime is ready (`Runs::list` filters, exit codes, output excerpts, tags, Git context).
4. `openspec/changes/export-and-doctor/` — portable export and local diagnostics.

Then continue with checker and correlation changes in the order documented by the roadmap.

## Change inventory

| Change | Status | Purpose | Depends on |
| --- | --- | --- | --- |
| project-foundation | archived 2026-09-04 | Rust CLI, config, local directory, SQLite schema, Git metadata | none |
| runtime-memory | archived 2026-09-04 | Run arbitrary commands, persist/query runs, top-level empty state | foundation |
| fingerprinting-and-retention | spec-only | Normalize failures, aggregate bugs, report, GC | runtime memory |
| export-and-doctor | spec-only | Portable export and local diagnostics | foundation; integrates with later data |
| checker-and-drift-alerts | spec-only | External checker protocol, adapters, snapshots, alerts | foundation |
| correlation-and-ai-context | spec-only | Heuristic links, manual links, AI report | fingerprinting; checker alerts |

## Implementation constraints

- Rust is the implementation language; monitored projects are language-agnostic.
- The default normalizer must accept arbitrary text. Rust/.NET/Python/Node-specific profiles are optional enhancements only.
- Use a single local binary and SQLite. Do not introduce services, a plugin runtime, an embedded vector database, or an LLM dependency.
- Default behavior is offline and non-telemetric.
- Preserve child command arguments and exit codes.
- Checker failures must be isolated from other checkers.
- Correlation wording must say "possible relationship", "possible match", or "heuristic correlation"; never present a score as root-cause proof.
- Keep full logs disposable while preserving long-term bug identity and statistics.

## Verification gates (current)

Last run on this change:

    cargo fmt --check
    cargo test             # 60 tests pass: 32 lib unit + 28 integration
    cargo clippy --all-targets --all-features -- -D warnings
    openspec validate --changes --strict --no-interactive   # 4/4 pass
    ./target/debug/driftwatch run echo hi                   # exit 0, row written
    ./target/debug/driftwatch list                          # table renders
    ./target/debug/driftwatch top                           # empty-state message

## Module map

- `src/main.rs` — binary entrypoint, `anyhow` boundary, returns `ExitCode`.
- `src/cli.rs` — `clap` derive types (`Cli`, `Command::{Init,Run,List,Top}` and arg structs).
- `src/error.rs` — `thiserror` `Error` enum used by library code.
- `src/project/root.rs` — `ProjectRoot::discover` with bounded walk-up.
- `src/project/config.rs` — `Config` struct + TOML loader (preserves absent sections).
- `src/project/git.rs` — `GitContext`, `capture(cwd)`, non-fatal failures.
- `src/project/init.rs` — `init`/`init_at` orchestration, idempotent.
- `src/storage/mod.rs` — `open(path)` with PRAGMAs (WAL, NORMAL, foreign_keys=ON).
- `src/storage/migrations.rs` — versioned migration runner.
- `src/storage/schema.rs` — `MIGRATION_0001_BASELINE` SQL.
- `src/repo/mod.rs` — `Db` wrapper, `open`, `open_in_memory`.
- `src/repo/runs.rs` — `RunRecord`, `RunStatus`, `RunCompletion`, `ListFilter`; `Runs::{reserve,find,insert_full,list}`.
- `src/repo/bugs.rs` — `Fingerprint`, `TopRow`; `Bugs::{find_by_hash,top}`.
- `src/repo/alerts.rs` — `Alerts::snapshot_count` (placeholder for checker change).
- `src/repo/correlations.rs`, `src/repo/links.rs` — placeholders with constructors.
- `src/runtime/runner.rs` — `CommandSpec`, `CapturedStream`, `RunOutcome`, `run`; two-thread drain past capture limit.
- `src/commands/{run,list,top}.rs` — per-subcommand orchestration returning process exit code.

## Known environment note

OpenSpec Codex skill generation initially hit a read-only sandbox directory. The integration was regenerated successfully in the writable host checkout with `openspec init --tools codex --force` and `openspec update --force`. Restart the IDE if slash commands are not visible.

## Next action

Implement exactly one active change at a time, beginning with `fingerprinting-and-retention`. Reuse the runtime's `Runs::list` output and `RunRecord` fields (tags, output excerpts, exit_code); add a generic normalizer that produces `fingerprints` and `occurrences` rows, hook it into the `run` flow after `insert_full`, and add `driftwatch report` and `driftwatch gc` to round out v0.1. When the change is done, follow the "Change completion workflow" at the top of this file.
