# Driftwatch Handoff

## Change completion workflow

After implementing a change and ticking every box in its `tasks.md`, follow the closing sequence in `AGENTS.md` ("Change completion workflow"):

1. Verification gates: `cargo fmt --check && cargo test && cargo clippy --all-targets --all-features -- -D warnings && openspec validate --changes --strict --no-interactive`.
2. `openspec archive <change> -y --skip-specs` (drop `--skip-specs` only if the change adds or modifies a `spec.md` capability).
3. `git add -A && git commit -m "Implement <change>"`.
4. Update this file (status, next action, module map) and commit: `git add HANDOFF.md && git commit -m "Update HANDOFF after <change>"`.
5. Re-run `openspec validate --changes --strict --no-interactive`.

## Current state

`project-foundation`, `runtime-memory`, `fingerprinting-and-retention`, `export-and-doctor`, and `checker-and-drift-alerts` are **implemented and archived**. The `driftwatch` Rust binary builds and tests cleanly: `driftwatch init`, `driftwatch run`, `driftwatch list`, `driftwatch top`, `driftwatch show`, `driftwatch report`, `driftwatch gc`, `driftwatch export json|jsonl|markdown`, `driftwatch doctor`, and `driftwatch check` are all functional. The SQLite schema (version 2) covers runs, fingerprints, occurrences, check_snapshots (with `git_commit`/`git_branch`), drift_alerts, correlations, and manual_links. Five capability specs are live under `openspec/specs/`. The remaining change package (`correlation-and-ai-context`) is spec-only.

## Start here

Read these in order:

1. README.md — product positioning and user-facing command surface.
2. ROADMAP.md — release sequence and dependency graph.
3. `openspec/changes/correlation-and-ai-context/` — **next change**: heuristic links, manual links, AI report.

Then continue with the remaining change in the order documented by the roadmap.

## Change inventory

| Change | Status | Purpose | Depends on |
| --- | --- | --- | --- |
| project-foundation | archived 2026-09-04 | Rust CLI, config, local directory, SQLite schema, Git metadata | none |
| runtime-memory | archived 2026-09-04 | Run arbitrary commands, persist/query runs, top-level empty state | foundation |
| fingerprinting-and-retention | archived 2026-09-04 | Normalize failures, aggregate bugs, report, GC | runtime memory |
| export-and-doctor | archived 2026-09-04 | Portable export (json/jsonl/markdown) and local doctor diagnostics | foundation |
| checker-and-drift-alerts | archived 2026-09-04 | External checker protocol, adapters, snapshots, alerts, `driftwatch check` | foundation |
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
    cargo test             # 217 tests pass: 156 lib unit + 61 integration
    cargo clippy --all-targets --all-features -- -D warnings
    openspec validate --changes --strict --no-interactive   # 2/2 pass
    ./target/debug/driftwatch run sh -c 'echo boom >&2; exit 1'   # bug attached
    ./target/debug/driftwatch show <hash8>                  # render fingerprint
    ./target/debug/driftwatch report                        # markdown report
    ./target/debug/driftwatch gc                            # pruned 0 runs (recent)
    ./target/debug/driftwatch export json                   # valid JSON document
    ./target/debug/driftwatch export jsonl                  # one record per line
    ./target/debug/driftwatch export markdown               # human-readable
    ./target/debug/driftwatch doctor                        # 6/6 checks pass (1 warn)
    ./target/debug/driftwatch check                         # runs configured checkers

## Module map

- `src/main.rs` — binary entrypoint, `anyhow` boundary, returns `ExitCode`; dispatches all 10 subcommands.
- `src/cli.rs` — `clap` derive types (`Cli`, `Command::{Init,Run,List,Top,Show,Report,Gc,Export,Doctor,Check}` and arg structs).
- `src/error.rs` — `thiserror` `Error` enum used by library code.
- `src/fingerprint/mod.rs` — module entry, re-exports `Rules`, `Canonical`, `fingerprint`.
- `src/fingerprint/normalizer.rs` — generic normalizer (13 ordered rules).
- `src/fingerprint/hash.rs` — `fingerprint(&str) -> String` (SHA-256).
- `src/project/root.rs` — `ProjectRoot::discover` with bounded walk-up.
- `src/project/config.rs` — `Config` + `CheckerEntry` with optional `working_dir`, `env`, `timeout_ms`, `max_output_bytes`.
- `src/project/git.rs` — `GitContext`, `capture(cwd)`, non-fatal failures.
- `src/project/init.rs` — `init`/`init_at` orchestration, idempotent.
- `src/storage/mod.rs` — `open(path)` with PRAGMAs (WAL, NORMAL, foreign_keys=ON).
- `src/storage/migrations.rs` — versioned migration runner (now applies 1 + 2).
- `src/storage/schema.rs` — `MIGRATION_0001_BASELINE` + `MIGRATION_0002_SNAPSHOT_GIT`.
- `src/repo/mod.rs` — `Db` wrapper, `open`, `open_in_memory`, `conn`, `conn_mut`.
- `src/repo/runs.rs` — `RunRecord`, `RunStatus`, `RunCompletion`, `ListFilter`; `Runs::{reserve,find,insert_full,list,all}`.
- `src/repo/bugs.rs` — `Fingerprint`, `TopRow`, `Occurrence`, `RecentCommit`, `Report`; `Bugs::{...}`.
- `src/repo/alerts.rs` — `Snapshot`, `Alert`, `NewSnapshot`, `NewAlert`; `Alerts::{snapshot_count,alert_count,list_snapshots,list_alerts,latest_snapshot_for,insert_snapshot,insert_alerts,record_run}`.
- `src/repo/correlations.rs` — `Correlations::{list_all,count}`; placeholder DTOs populated by correlation change.
- `src/repo/links.rs` — `Links::{list_all,count}`; placeholder DTOs populated by correlation change.
- `src/export/{dto,build,json,jsonl,markdown,mod}.rs` — versioned export DTOs (`SCHEMA_VERSION = 1`) and three serializers.
- `src/doctor/{check,mod}.rs` — `Check`, `Status`, and the `Report` aggregator. Includes a `checker.last_run` warn when the most recent check snapshot for a configured checker was a failure.
- `src/checker/mod.rs` — public module: `protocol`, `runner`, `report` re-exports.
- `src/checker/protocol.rs` — `DriftAlert`, `AlertsDocument`, `ProtocolError`, `parse_alerts_document`.
- `src/checker/runner.rs` — `CheckerSpec`, `run_checker`, `CheckerRun` with bounded capture and per-checker timeout.
- `src/checker/report.rs` — `Status`, `Severity`, `CheckerOutcome`, `label_for_status`.
- `src/runtime/runner.rs` — `CommandSpec`, `CapturedStream`, `RunOutcome`, `run`.
- `src/commands/{run,list,top,show,report,gc,export,doctor,check}.rs` — per-subcommand orchestration returning process exit code.

## Known environment note

OpenSpec Codex skill generation initially hit a read-only sandbox directory. The integration was regenerated successfully in the writable host checkout with `openspec init --tools codex --force` and `openspec update --force`. Restart the IDE if slash commands are not visible.

## Next action

Implement exactly one active change at a time, beginning with `correlation-and-ai-context`. The checkers surface is now stable: `driftwatch check` populates `check_snapshots` and `drift_alerts` (with `git_commit`/`git_branch`), `driftwatch export` reflects them with no further DTO work, and `driftwatch doctor` surfaces a Warn when a configured checker has been failing on its last run. Follow the "Change completion workflow" at the top of this file.
