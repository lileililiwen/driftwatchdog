# Driftwatchdog Handoff

## Change completion workflow

After implementing a change and ticking every box in its `tasks.md`, follow the closing sequence in `AGENTS.md` ("Change completion workflow"):

1. Verification gates: `cargo fmt --check && cargo test && cargo clippy --all-targets --all-features -- -D warnings && openspec validate --changes --strict --no-interactive`.
2. Confirm `openspec/changes/<change>/tasks.md` has every box ticked.
3. `openspec archive <change> -y` (drop `--skip-specs` only if the change adds or modifies a `spec.md` capability).
4. `git add -A && git commit -m "Implement <change>"`.
5. Update this file (status, next action, module map) and commit: `git add HANDOFF.md && git commit -m "Update HANDOFF after <change>"`.
6. Re-run `openspec validate --changes --strict --no-interactive`.

## Current state

All v0.x change packages through v0.5 are **implemented and archived**: `project-foundation`, `runtime-memory`, `fingerprinting-and-retention`, `export-and-doctor`, `checker-and-drift-alerts`, `correlation-and-ai-context`, and `linux-macos-distribution`. The `crash-hardening` change is also **implemented and archived** (2026-09-14): char-boundary truncation helper, UTF-8-once capture, signal-aware statuses (`RunStatus::Signalled`/`Timeout`, checker `Status::Unknown`), opt-in `run --timeout-ms` with process-group kill, and capture diagnostics. The `quality-cicd-docs-ux` change is also **implemented and archived** (2026-09-14): `[lints.clippy] all = "deny"` + `rust-version = "1.74"` MSRV (with `is_none_or`/`repeat_n` lowered to 1.74-compatible APIs), `deny.toml` advisory/license policy, proptest seeds for the normalizer, CI breadth (`--all-features` clippy, MSRV check, cargo-deny, tarpaulin coverage, `macos-14` matrix, shellcheck + script-mode enforcement, `npm audit`, smoke), release integrity (npm provenance via `id-token: write`, `macos-14` runner, tag==Cargo version assertion, SPDX SBOM, CHANGELOG.md, cold-start `bump.sh` at 0.1.0, PAT-missing skip + no-tag-spam auto-tag, Dependabot), and docs/UX (binary-naming rule, `driftwatch completions`/`man`, per-command examples, error `hint:` lines, honest doctor INFO, mandatory-verify installer with early `--dry-run` + `cargo install` fallback, npm darwin-arm64 preinstall guard, `driftwatch.toml.example`, repo-hygiene packaging test). The `driftwatchdog` Rust binary builds and tests cleanly (396 tests pass). The full CLI surface is functional: `init`, `run`, `list`, `top`, `show`, `report` (with `--ai`), `gc`, `export json|jsonl|markdown`, `doctor`, `check`, `link`, `unlink`, `completions`, `man`, `mcp`. The MCP server speaks JSON-RPC 2.0 on stdio with four read-only tools (`top_bugs`, `show_bug`, `ai_report`, `doctor_status`) and opens the database read-only. The SQLite schema (version 3) covers runs, fingerprints, occurrences, check_snapshots (with `git_commit`/`git_branch`), drift_alerts, correlations (with per-component scores and `algorithm_version`), and manual_links. Seven capability specs are live under `openspec/specs/` (including `quality-cicd-docs-ux`; `ci-test-gates` and `release-distribution` Purposes filled). Native release archives plus an SHA-256 manifest are produced for Linux x86_64, Linux arm64, and macOS x86_64 by `.github/workflows/release.yml`; the shell installer (`scripts/install.sh`), the npm launcher (`npm/driftwatchdog/`), direct downloads, and `cargo install` are documented in README.md. The next change is the v0.6 integration work (MCP read tools + examples + GitHub Actions templates); see ROADMAP.md.

## Start here

Read these in order:

1. README.md — product positioning, user-facing command surface, and installation channels.
2. ROADMAP.md — release sequence, closed change inventory, and dependency graph.
3. `openspec/specs/` — the eight capability specifications the implementation satisfies.
4. The remaining v0.6 proposal changes (spec-only) under `openspec/changes/`: implement `agent-examples` next, then `github-actions-templates`, one at a time.

## Change inventory

| Change | Status | Purpose | Depends on |
| --- | --- | --- | --- |
| project-foundation | archived 2026-09-04 | Rust CLI, config, local directory, SQLite schema, Git metadata | none |
| runtime-memory | archived 2026-09-04 | Run arbitrary commands, persist/query runs, top-level empty state | foundation |
| fingerprinting-and-retention | archived 2026-09-04 | Normalize failures, aggregate bugs, report, GC | runtime memory |
| export-and-doctor | archived 2026-09-04 | Portable export (json/jsonl/markdown) and local doctor diagnostics | foundation |
| checker-and-drift-alerts | archived 2026-09-04 | External checker protocol, adapters, snapshots, alerts, `driftwatch check` | foundation |
| correlation-and-ai-context | archived 2026-09-04 | Heuristic correlations, manual `link`/`unlink`, `driftwatch report --ai` | fingerprinting; checker alerts |
| linux-macos-distribution | archived 2026-09-04 | Shell installer, npm launcher, release workflow, SHA-256-verified native archives for Linux x86_64, Linux arm64, and macOS x86_64 | any prior archive |
| crash-hardening | archived 2026-09-14 | No-panic truncation, UTF-8-safe capture, signal-aware status, bounded run/checker execution | runtime-memory; checker-and-drift-alerts |
| mcp-read-tools | archived 2026-09-14 | stdio MCP server with read-only bug tools (`top_bugs`, `show_bug`, `ai_report`, `doctor_status`) | stable CLI surface |
| agent-examples | proposed (spec-only) | Tested Claude Code/OpenCode/Aider MCP + workflow examples | mcp-read-tools |
| github-actions-templates | proposed (spec-only) | Reusable CI check template publishing the AI report | mcp-read-tools (sequenced third; stable CLI only) |

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
    cargo test             # 396 tests pass: lib + integration (incl. packaging)
    cargo clippy --all-targets --all-features -- -D warnings
    openspec validate --changes --strict --no-interactive   # 0/0 pass (changes archived)
    ./target/debug/driftwatch run sh -c 'echo boom >&2; exit 1'   # bug attached
    ./target/debug/driftwatch show <hash8>                  # render fingerprint
    ./target/debug/driftwatch report                        # markdown report
    ./target/debug/driftwatch report --ai                   # AI-context report
    ./target/debug/driftwatch gc                            # pruned 0 runs (recent)
    ./target/debug/driftwatch export json                   # valid JSON document
    ./target/debug/driftwatch export jsonl                  # one record per line
    ./target/debug/driftwatch export markdown               # human-readable
    ./target/debug/driftwatch doctor                        # 6/6 checks pass (1 warn)
    ./target/debug/driftwatch check                         # runs configured checkers
    ./target/debug/driftwatch link bug:<hash8> spec:<id>    # persists manual link
    ./target/debug/driftwatch unlink <id>                   # removes targeted link
    sh scripts/smoke.sh                                     # release packaging end-to-end

## Module map

- `src/main.rs` — binary entrypoint, `anyhow` boundary, returns `ExitCode`; dispatches all 13 subcommands.
- `src/cli.rs` — `clap` derive types (`Cli`, `Command::{Init,Run,List,Top,Show,Report,Gc,Export,Doctor,Check,Link,Unlink,Completions,Man,Mcp}`) and arg structs; `RunArgs` carries opt-in `--timeout-ms`.
- `src/error.rs` — `thiserror` `Error` enum used by library code; includes `LinkTarget` and `ManualLinkNotFound` variants.
- `src/fingerprint/mod.rs` — module entry, re-exports `Rules`, `Canonical`, `fingerprint`.
- `src/fingerprint/normalizer.rs` — generic normalizer (13 ordered rules).
- `src/fingerprint/hash.rs` — `fingerprint(&str) -> String` (SHA-256).
- `src/project/root.rs` — `ProjectRoot::discover` with bounded walk-up.
- `src/project/config.rs` — `Config` + `CheckerEntry` with optional `working_dir`, `env`, `timeout_ms`, `max_output_bytes`.
- `src/project/git.rs` — `GitContext`, `capture(cwd)`, non-fatal failures.
- `src/project/init.rs` — `init`/`init_at` orchestration, idempotent.
- `src/storage/mod.rs` — `open(path)` with PRAGMAs (WAL, NORMAL, foreign_keys=ON).
- `src/storage/migrations.rs` — versioned migration runner (applies 1 + 2 + 3).
- `src/storage/schema.rs` — `MIGRATION_0001_BASELINE`, `MIGRATION_0002_SNAPSHOT_GIT`, `MIGRATION_0003_CORRELATION_DETAIL`.
- `src/repo/mod.rs` — `Db` wrapper, `open`, `open_in_memory`, `conn`, `conn_mut`.
- `src/repo/runs.rs` — `RunRecord`, `RunStatus` (`Running`/`Success`/`Failed`/`StartFailed`/`Signalled`/`Timeout`), `RunCompletion`, `ListFilter`; `Runs::{reserve,find,insert_full,list,all}`.
- `src/repo/bugs.rs` — `Fingerprint`, `TopRow`, `Occurrence`, `RecentCommit`, `Report`; `Bugs::{...}` plus `current_fingerprints` and `tags_for`.
- `src/repo/alerts.rs` — `Snapshot`, `Alert`, `NewSnapshot`, `NewAlert`; `Alerts::{snapshot_count,alert_count,list_snapshots,list_alerts,latest_snapshot_for,insert_snapshot,insert_alerts,record_run}` plus `current_alerts`.
- `src/repo/correlations.rs` — `Correlation` with per-component score fields and `algorithm_version`; `Correlations::{list_all,count,upsert}` plus `replace_for_fingerprint` (static, takes `&mut Db`).
- `src/repo/links.rs` — `ManualLink`; `Links::{list_all,count,find_by_id,create,delete,list_for_fingerprint,list_for_alert}`.
- `src/similarity/{mod,tokenize,score,candidates}.rs` — heuristic engine: tokenization, Jaccard, weighted `score_pair`, N×M candidate generation with 5,000-pair cap.
- `src/correlate.rs` — `run_after_check` orchestration: loads fingerprints + alerts, runs the candidate generator, persists passing pairs.
- `src/export/{dto,build,json,jsonl,markdown,mod}.rs` — versioned export DTOs (`SCHEMA_VERSION = 2`) and three serializers.
- `src/doctor/{check,mod}.rs` — `Check`, `Status`, and the `Report` aggregator. Includes a `checker.last_run` warn when the most recent check snapshot for a configured checker was a failure.
- `src/checker/mod.rs` — public module: `protocol`, `runner`, `report` re-exports.
- `src/checker/protocol.rs` — `DriftAlert`, `AlertsDocument`, `ProtocolError`, `parse_alerts_document`.
- `src/checker/runner.rs` — `CheckerSpec`, `run_checker`, `CheckerRun` (incl. `signalled` + `capture_error`) with bounded capture and per-checker timeout plus group kill.
- `src/checker/report.rs` — `Status`, `Severity`, `CheckerOutcome`, `label_for_status`.
- `src/runtime/runner.rs` — `CommandSpec` (incl. opt-in `timeout_ms`), `CapturedStream`, `RunOutcome` (incl. `timed_out` + `diagnostic`), `run`; byte-accumulating UTF-8-once drain, signal-aware status, process-group kill on timeout.
- `src/util.rs` — `truncate_char_boundary` shared helper (byte limit, char-boundary cut, ellipsis).
- `src/checker/runner.rs` — `CheckerSpec`, `run_checker`, `CheckerRun` (incl. `signalled` + `capture_error`) with bounded capture and per-checker timeout plus group kill.
- `src/commands/{run,list,top,show,report,gc,export,doctor,check,link,unlink,report_ai}.rs` — per-subcommand orchestration returning process exit code. `top.rs` and `show.rs` expose `render` functions that return the same bytes the CLI prints; `report_ai.rs` exposes `render_ai_for_mcp` for the read-only MCP path.
- `src/commands/meta.rs` — `completions` (all five shells via `clap_complete`) and `man` (via `clap_mangen`) generators.
- `src/mcp/{mod,server,tools}.rs` — Model Context Protocol server over stdio. `server.rs` is the JSON-RPC 2.0 dispatch loop (newline-delimited, `PROTOCOL_VERSION = "2024-11-05"`, `-32700`/`-32601`/`-32602` error codes, EOF exits 0). `tools.rs` defines four read-only tools (`top_bugs`, `show_bug`, `ai_report`, `doctor_status`) that reuse the existing CLI builders; every input schema is closed (`additionalProperties: false`). The DB is opened with `SQLITE_OPEN_READ_ONLY` via the existing `Db::open_read_only` so a write attempt fails at the driver level.
- `src/cli.rs` — `clap` derive types plus `Completions`/`Man` subcommands, `long_about` with examples, per-command `after_help` examples, and the binary-naming rule doc comment.
- `src/error.rs` — `thiserror` `Error` enum plus `hint()` single-wrap remediation for every user-facing variant.
- `src/doctor/check.rs` — `Status::{Pass,Info,Warn,Fail}` with `Check::info` constructor (unconfigured-but-ok states).
- `deny.toml` — cargo-deny policy (advisories, licenses, bans, sources) enforced in CI.
- `CHANGELOG.md` — Keep-a-Changelog history (Unreleased section started).
- `scripts/lib/{config,version,platform,release}.sh` — shared packaging helpers: target matrix, version source (Cargo.toml), host detection, and URL construction.
- `scripts/package.sh` — reproducible per-target release builder (`cargo build --release --locked`, then archive).
- `scripts/checksum.sh` — deterministic SHA-256 manifest generator from the final archives.
- `scripts/install.sh` — POSIX shell installer: strict mode, `--version`/`--dest`/`--allow-root`/`--dry-run` (early exit, no network), mandatory SHA-256 verification, HTTPS download, `cargo install` fallback on unsupported hosts, atomic install into a user-writable default.
- `scripts/bump.sh` — conventional-commit bump heuristic with cold-start at `0.1.0`.
- `npm/driftwatchdog/scripts/install-guard.js` — preinstall preflight failing fast on unsupported hosts (incl. darwin-arm64).
- `scripts/smoke.sh` — opt-in local release smoke test that drives the installer against a hand-built fixture.
- `npm/driftwatchdog/package.json` — npm package metadata (`bin` exposes `driftwatch`).
- `npm/driftwatchdog/bin/driftwatch.js` — launcher: host detection, versioned cache, manifest + archive fetch, verification, exec with forwarded args and exit status.
- `npm/driftwatchdog/lib/platform.js` — npm-side `targetFor` / `detectTarget`, normalized to the shell matrix.
- `npm/driftwatchdog/lib/release.js` — npm-side `releaseUrls`, mirrors `scripts/lib/release.sh`.
- `npm/driftwatchdog/lib/verify.js` — dependency-free HTTP, SHA-256, manifest parser, and tar.gz extractor (with path-traversal safety).
- `npm/driftwatchdog/test/launcher.test.js` — `node:test` suite covering supported/unsupported hosts, verification, round-trip exec, cache reuse, and traversal rejection.
- `tests/packaging.sh` + `tests/packaging/{test_*.sh,fixture.sh}` — bash packaging tests (target mapping, artifact naming, checksum manifest, installer, repo hygiene) and shared fixture.
- `tests/packaging/run_all.sh` + `tests/packaging.rs` — combined bash+node runner and a Rust integration test that invokes it from `cargo test`.
- `.github/workflows/ci.yml` — fmt, clippy (`--all-targets --all-features`, `-D warnings`), workspace tests, MSRV (1.74) check, cargo-deny, tarpaulin coverage, `macos-14` tests, shellcheck + script-mode enforcement, npm tests + audit, smoke.
- `.github/workflows/release.yml` — tag-triggered matrix build (linux-x86_64, linux-arm64, darwin-x86_64 on `macos-14`), tag==Cargo version assertion, checksum manifest + SPDX SBOM generation, and `softprops/action-gh-release` upload; the publish job depends on every matrix build, so partial matrices fail before any asset ships. npm-publish carries `id-token: write` for `--provenance`.
- `.github/workflows/auto-tag.yml` — PAT-missing skip with notice, `[no-release]` skip, remote tag-spam guard, cold-start bump.
- `.github/dependabot.yml` — weekly cargo/npm/actions updates.

## Known environment note

OpenSpec Codex skill generation initially hit a read-only sandbox directory. The integration was regenerated successfully in the writable host checkout with `openspec init --tools codex --force` and `openspec update --force`. Restart the IDE if slash commands are not visible.

## Next action

**Recommended next spec: `agent-examples`** (`openspec/changes/agent-examples/`). It is a documentation + example-only change that attaches to the now-archived `mcp-read-tools` server. `github-actions-templates` follows once the example shape is established. Implement in this order:

1. `agent-examples` — `examples/{claude-code,opencode,aider}/` collateral + consistency test (depends on the server from `mcp-read-tools`, which is now archived).
2. `github-actions-templates` — `templates/github-actions/driftwatch-check.yml` + shape test (stable CLI only).

`mcp-read-tools` is closed (archived 2026-09-14). v0.6 (MCP read tools + examples + GitHub Actions templates) now needs only the two remaining example/template changes per ROADMAP.md. Follow the "Change completion workflow" at the top of this file when the next change is ready to archive.
