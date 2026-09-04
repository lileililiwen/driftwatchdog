# Driftwatch Handoff

## Change completion workflow

After implementing a change and ticking every box in its `tasks.md`, follow the closing sequence in `AGENTS.md` ("Change completion workflow"):

1. Verification gates: `cargo fmt --check && cargo test && cargo clippy --all-targets --all-features -- -D warnings && openspec validate --changes --strict --no-interactive`.
2. `openspec archive <change> -y --skip-specs` (drop `--skip-specs` only if the change adds or modifies a `spec.md` capability).
3. `git add -A && git commit -m "Implement <change>"`.
4. Update this file (status, next action, module map) and commit: `git add HANDOFF.md && git commit -m "Update HANDOFF after <change>"`.
5. Re-run `openspec validate --changes --strict --no-interactive`.

## Current state

The repository is past the spec/bootstrap stage. `project-foundation` is implemented: the `driftwatch` Rust binary builds and tests cleanly, `driftwatch init` is functional, the SQLite schema (version 1) is in place, and all OpenSpec validation gates pass. The remaining five change packages (`runtime-memory`, `fingerprinting-and-retention`, `export-and-doctor`, `checker-and-drift-alerts`, `correlation-and-ai-context`) are spec-only.

## Start here

Read these in order:

1. README.md — product positioning and user-facing command surface.
2. ROADMAP.md — release sequence and dependency graph.
3. `openspec/changes/runtime-memory/` — next change: arbitrary command runner and run queries. Foundation is ready (`Db`, `Runs::reserve`, schema fields for output/tags/git, project-root discovery, config loader).
4. `openspec/changes/fingerprinting-and-retention/` — generic normalization and bug memory.

Then continue with export/diagnostics, checker, and correlation changes in the order documented by the roadmap.

## Change inventory

| Change | Status | Purpose | Depends on |
| --- | --- | --- | --- |
| project-foundation | implemented | Rust CLI, config, local directory, SQLite schema, Git metadata | none |
| runtime-memory | spec-only | Run arbitrary commands and persist/query runs | foundation |
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
    cargo test             # 31 tests pass: 19 unit + 12 integration
    cargo clippy --all-targets --all-features -- -D warnings
    openspec validate --changes --strict --no-interactive   # 6/6 pass
    ./target/debug/driftwatch init --help                   # works

## Foundation module map

- `src/main.rs` — binary entrypoint, `anyhow` boundary.
- `src/cli.rs` — `clap` derive types (`Cli`, `Command::Init`, `InitArgs`).
- `src/error.rs` — `thiserror` `Error` enum used by library code.
- `src/project/root.rs` — `ProjectRoot::discover` with bounded walk-up.
- `src/project/config.rs` — `Config` struct + TOML loader (preserves absent sections).
- `src/project/git.rs` — `GitContext`, `capture(cwd)`, non-fatal failures.
- `src/project/init.rs` — `init`/`init_at` orchestration, idempotent.
- `src/storage/mod.rs` — `open(path)` with PRAGMAs (WAL, NORMAL, foreign_keys=ON).
- `src/storage/migrations.rs` — versioned migration runner.
- `src/storage/schema.rs` — `MIGRATION_0001_BASELINE` SQL.
- `src/repo/mod.rs` — `Db` wrapper, `open`, `open_in_memory`.
- `src/repo/runs.rs` — `RunRecord`, `RunStatus`, `Runs::reserve/find`.
- `src/repo/bugs.rs` — `Fingerprint` row, `Bugs::find_by_hash`.
- `src/repo/alerts.rs` — `Alerts::snapshot_count` (placeholder for checker change).
- `src/repo/correlations.rs`, `src/repo/links.rs` — placeholders with constructors.

## Known environment note

OpenSpec Codex skill generation initially hit a read-only sandbox directory. The integration was regenerated successfully in the writable host checkout with `openspec init --tools codex --force` and `openspec update --force`. Restart the IDE if slash commands are not visible.

## Next action

Implement exactly one active change at a time, beginning with `runtime-memory`. Reuse the foundation's `Db` + `Runs::reserve`; extend the runner schema, add bounded stream capture, and add the `list`/`top` query surfaces.
