# Driftwatch Handoff

## Change completion workflow

After implementing a change and ticking every box in its `tasks.md`, follow the closing sequence in `AGENTS.md` ("Change completion workflow"):

1. Verification gates: `cargo fmt --check && cargo test && cargo clippy --all-targets --all-features -- -D warnings && openspec validate --changes --strict --no-interactive`.
2. `openspec archive <change> -y --skip-specs` (drop `--skip-specs` only if the change adds or modifies a `spec.md` capability).
3. `git add -A && git commit -m "Implement <change>"`.
4. Update this file (status, next action, module map) and commit: `git add HANDOFF.md && git commit -m "Update HANDOFF after <change>"`.
5. Re-run `openspec validate --changes --strict --no-interactive`.

## Current state

`project-foundation`, `runtime-memory`, and `fingerprinting-and-retention` are **implemented and archived** (all 2026-09-04). The `driftwatch` Rust binary builds and tests cleanly: `driftwatch init`, `driftwatch run`, `driftwatch list`, `driftwatch top`, `driftwatch show`, `driftwatch report`, and `driftwatch gc` are all functional. The SQLite schema (version 1) covers runs, fingerprints, occurrences, check_snapshots, drift_alerts, correlations, and manual_links. Three capability specs are live under `openspec/specs/`. The remaining three change packages (`export-and-doctor`, `checker-and-drift-alerts`, `correlation-and-ai-context`) are spec-only.

## Start here

Read these in order:

1. README.md — product positioning and user-facing command surface.
2. ROADMAP.md — release sequence and dependency graph.
3. `openspec/changes/export-and-doctor/` — **next change**: portable export and local diagnostics.
4. `openspec/changes/checker-and-drift-alerts/` — external checker protocol and drift snapshots.

Then continue with the correlation change in the order documented by the roadmap.

## Change inventory

| Change | Status | Purpose | Depends on |
| --- | --- | --- | --- |
| project-foundation | archived 2026-09-04 | Rust CLI, config, local directory, SQLite schema, Git metadata | none |
| runtime-memory | archived 2026-09-04 | Run arbitrary commands, persist/query runs, top-level empty state | foundation |
| fingerprinting-and-retention | archived 2026-09-04 | Normalize failures, aggregate bugs, report, GC | runtime memory |
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
    cargo test             # 136 tests pass: 88 lib unit + 48 integration
    cargo clippy --all-targets --all-features -- -D warnings
    openspec validate --changes --strict --no-interactive   # 3/3 pass
    ./target/debug/driftwatch run sh -c 'echo boom >&2; exit 1'   # bug attached
    ./target/debug/driftwatch show <hash8>                  # render fingerprint
    ./target/debug/driftwatch report                        # markdown report
    ./target/debug/driftwatch gc                            # pruned 0 runs (recent)

## Module map

- `src/main.rs` — binary entrypoint, `anyhow` boundary, returns `ExitCode`.
- `src/cli.rs` — `clap` derive types (`Cli`, `Command::{Init,Run,List,Top,Show,Report,Gc}` and arg structs).
- `src/error.rs` — `thiserror` `Error` enum used by library code, with `BugNotFound { id }`.
- `src/fingerprint/mod.rs` — module entry, re-exports `Rules`, `Canonical`, `fingerprint`.
- `src/fingerprint/normalizer.rs` — `Rule`, `Rules::generic()`, `Canonical`, `summary_of`, `bounded_excerpt`; 13 ordered rules (ANSI, OSC, temp_path, absolute_path, port, line/column, uuid, iso_timestamp, pid, duration_ms, duration_s, long_hex, long_decimal).
- `src/fingerprint/hash.rs` — `fingerprint(&str) -> String` (64 hex SHA-256).
- `src/project/root.rs` — `ProjectRoot::discover` with bounded walk-up.
- `src/project/config.rs` — `Config` struct + TOML loader (preserves absent sections).
- `src/project/git.rs` — `GitContext`, `capture(cwd)`, non-fatal failures.
- `src/project/init.rs` — `init`/`init_at` orchestration, idempotent.
- `src/storage/mod.rs` — `open(path)` with PRAGMAs (WAL, NORMAL, foreign_keys=ON).
- `src/storage/migrations.rs` — versioned migration runner.
- `src/storage/schema.rs` — `MIGRATION_0001_BASELINE` SQL.
- `src/repo/mod.rs` — `Db` wrapper, `open`, `open_in_memory`, `conn`, `conn_mut`.
- `src/repo/runs.rs` — `RunRecord`, `RunStatus`, `RunCompletion`, `ListFilter`; `Runs::{reserve,find,insert_full,list}`.
- `src/repo/bugs.rs` — `Fingerprint`, `TopRow`, `Occurrence`, `RecentCommit`, `Report`; `Bugs::{find_by_hash,find_by_id,find_by_hash_prefix,hash_for_run,hash_for_runs,upsert_for_occurrence,insert_occurrence,occurrences_for,recent_commits,report_for,top,prune_streams}`.
- `src/repo/alerts.rs` — `Alerts::snapshot_count` (placeholder for checker change).
- `src/repo/correlations.rs`, `src/repo/links.rs` — placeholders with constructors.
- `src/runtime/runner.rs` — `CommandSpec`, `CapturedStream`, `RunOutcome`, `run`; two-thread drain past capture limit.
- `src/commands/{run,list,top,show,report,gc}.rs` — per-subcommand orchestration returning process exit code.

## Known environment note

OpenSpec Codex skill generation initially hit a read-only sandbox directory. The integration was regenerated successfully in the writable host checkout with `openspec init --tools codex --force` and `openspec update --force`. Restart the IDE if slash commands are not visible.

## Next action

Implement exactly one active change at a time, beginning with `export-and-doctor`. The runtime + fingerprinting + retention foundation is now stable: `Runs::list` filters, `Bugs::top/report_for/prune_streams`, and the `Driftwatch.toml` config surface are all ready to be consumed. The new change should add `driftwatch export json|jsonl|markdown` and `driftwatch doctor` without touching the checker or correlation surface. When the change is done, follow the "Change completion workflow" at the top of this file.
