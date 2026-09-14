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

All v0.x change packages through v0.5 are **implemented and archived**: `project-foundation`, `runtime-memory`, `fingerprinting-and-retention`, `export-and-doctor`, `checker-and-drift-alerts`, `correlation-and-ai-context`, and `linux-macos-distribution`. The `crash-hardening` change is also **implemented and archived** (2026-09-14): char-boundary truncation helper, UTF-8-once capture, signal-aware statuses (`RunStatus::Signalled`/`Timeout`, checker `Status::Unknown`), opt-in `run --timeout-ms` with process-group kill, and capture diagnostics. The `quality-cicd-docs-ux` change is also **implemented and archived** (2026-09-14): `[lints.clippy] all = "deny"` + `rust-version = "1.74"` MSRV (with `is_none_or`/`repeat_n` lowered to 1.74-compatible APIs), `deny.toml` advisory/license policy, proptest seeds for the normalizer, CI breadth (`--all-features` clippy, MSRV check, cargo-deny, tarpaulin coverage, `macos-14` matrix, shellcheck + script-mode enforcement, `npm audit`, smoke), release integrity (npm provenance via `id-token: write`, `macos-14` runner, tag==Cargo version assertion, SPDX SBOM, CHANGELOG.md, cold-start `bump.sh` at 0.1.0, PAT-missing skip + no-tag-spam auto-tag, Dependabot), and docs/UX (binary-naming rule, `driftwatch completions`/`man`, per-command examples, error `hint:` lines, honest doctor INFO, mandatory-verify installer with early `--dry-run` + `cargo install` fallback, npm darwin-arm64 preinstall guard, `driftwatch.toml.example`, repo-hygiene packaging test). The `mcp-read-tools` change is also **implemented and archived** (2026-09-14): stdio JSON-RPC 2.0 MCP server with four read-only tools (`top_bugs`/`show_bug`/`ai_report`/`doctor_status`), closed input schemas, `SQLITE_OPEN_READ_ONLY` DB open, and `driftwatch mcp` subcommand. The `agent-examples` change is also **implemented and archived** (2026-09-14): tested copy-paste MCP + workflow examples for Claude Code, OpenCode, and Aider, the `report --ai > drift.md` fallback for Aider, `examples/README.md` with the harness→destination map, and a packaging consistency test that asserts every `mcp.json` parses, every `driftwatch <sub>` token in the workflows exists in `--help`, and no `run`/`check` is ever presented as an agent step. The `github-actions-templates` change is also **implemented and archived** (2026-09-14): reusable GitHub Actions workflow at `templates/github-actions/driftwatch-check.yml` (workflow_call + workflow_dispatch) that pins a `driftwatchdog` install, always renders `report --ai > drift.md`, uploads only `drift.md` (never `state.db` or command logs), summaries `driftwatch top` into `$GITHUB_STEP_SUMMARY`, and ships with `contents: read` plus `fail_on_drift`/`upload_report` inputs. The `driftwatchdog` Rust binary builds and tests cleanly (396 tests pass). The full CLI surface is functional: `init`, `run`, `list`, `top`, `show`, `report` (with `--ai`), `gc`, `export json|jsonl|markdown`, `doctor`, `check`, `link`, `unlink`, `completions`, `man`, `mcp`. The MCP server speaks JSON-RPC 2.0 on stdio with four read-only tools (`top_bugs`, `show_bug`, `ai_report`, `doctor_status`) and opens the database read-only. The SQLite schema (version 3) covers runs, fingerprints, occurrences, check_snapshots (with `git_commit`/`git_branch`), drift_alerts, correlations (with per-component scores and `algorithm_version`), and manual_links. Nine capability specs are live under `openspec/specs/` (including `quality-cicd-docs-ux`, `agent-examples`, and `github-actions-templates`; `ci-test-gates` and `release-distribution` Purposes filled). Native release archives plus an SHA-256 manifest are produced for Linux x86_64, Linux arm64, and macOS x86_64 by `.github/workflows/release.yml`; the shell installer (`scripts/install.sh`), the npm launcher (`npm/driftwatchdog/`), direct downloads, and `cargo install` are documented in README.md. v0.6 (Integrate) is complete. v1.0 (Stable) is the next planning milestone; the first v1.0 hardening change is not yet proposed in `openspec/changes/`.

## Start here

Read these in order:

1. README.md — product positioning, user-facing command surface, and installation channels.
2. ROADMAP.md — release sequence, closed change inventory, and dependency graph.
3. `openspec/specs/` — the eight capability specifications the implementation satisfies.
4. `openspec/changes/` is empty (all v0.6 changes are archived); the next step is to scope the first v1.0 hardening change (likely a schema/config freeze plus an extended real-project validation sweep) and add a `proposal.md` / `tasks.md` for it.

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
| agent-examples | archived 2026-09-14 | Tested copy-paste MCP + workflow examples for Claude Code, OpenCode, and Aider; Aider includes the `report --ai > drift.md` fallback; consistency test in `tests/packaging/test_agent_examples.sh` | mcp-read-tools |
| github-actions-templates | archived 2026-09-14 | Reusable `templates/github-actions/driftwatch-check.yml` (`workflow_call` + `workflow_dispatch`) that pins a `driftwatchdog` install, always renders the AI report, uploads only `drift.md`, summarises `top` into the step summary, with `contents: read` and the `fail_on_drift`/`upload_report` inputs; shape test in `tests/packaging/test_gha_templates.sh` | mcp-read-tools (stable CLI only) |

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
    sh tests/packaging.sh   # 6/6 pass: target_mapping, artifact_naming, checksum_manifest, installer, repo_hygiene, agent_examples
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
- `tests/packaging.sh` + `tests/packaging/{test_*.sh,fixture.sh}` — bash packaging tests (target mapping, artifact naming, checksum manifest, installer, repo hygiene, agent examples) and shared fixture.
- `tests/packaging/run_all.sh` + `tests/packaging.rs` — combined bash+node runner and a Rust integration test that invokes it from `cargo test`.
- `tests/packaging/test_agent_examples.sh` — `agent-examples` consistency test: every `examples/*/mcp.json` parses, declares a stdio server with `driftwatch` + `mcp`, and every `driftwatch <sub>` token in `workflow.md` exists in `driftwatchdog --help`; no `run`/`check` is allowed as an agent step. Auto-picked by `tests/packaging.sh`.
- `tests/packaging/test_gha_templates.sh` — `github-actions-templates` shape test: triggers (`workflow_call` + `workflow_dispatch`), installer base URL matches the release repository in `scripts/lib/release.sh`, `driftwatch check` + `report --ai` + step summary + upload-artifact anchors present, upload `path:` is `drift.md` (never `state.db` or anything under `.driftwatch/`), `fail_on_drift` documented in both files, `contents: read` set; embedded `run:` blocks pass `shellcheck -S error` when the tool is available (soft-skip otherwise). Auto-picked by `tests/packaging.sh`.
- `examples/{claude-code,opencode,aider}/` — per-harness MCP client-config fragment (`mcp.json`) and investigation workflow (`workflow.md`); Aider also documents the `report --ai > drift.md` fallback.
- `examples/README.md` — harness→destination map, stdio-only + read-only caveat, fallback pointer.
- `templates/github-actions/driftwatch-check.yml` — reusable workflow (`workflow_call` + `workflow_dispatch`); pinned `driftwatchdog` install via the same release base `scripts/lib/release.sh` ships, `driftwatch check`, always-render `report --ai > drift.md`, upload `drift.md` only, `driftwatch top --limit 5` into `$GITHUB_STEP_SUMMARY`, `contents: read`, `fail_on_drift` and `upload_report` inputs.
- `templates/github-actions/README.md` — input table, copy-vs-`uses:` decision guide, exit-code contract, and the never-upload-`state.db` warning.
- `.github/workflows/ci.yml` — fmt, clippy (`--all-targets --all-features`, `-D warnings`), workspace tests, MSRV (1.74) check, cargo-deny, tarpaulin coverage, `macos-14` tests, shellcheck + script-mode enforcement, npm tests + audit, smoke.
- `.github/workflows/release.yml` — tag-triggered matrix build (linux-x86_64, linux-arm64, darwin-x86_64 on `macos-14`), tag==Cargo version assertion, checksum manifest + SPDX SBOM generation, and `softprops/action-gh-release` upload; the publish job depends on every matrix build, so partial matrices fail before any asset ships. npm-publish carries `id-token: write` for `--provenance`.
- `.github/workflows/auto-tag.yml` — PAT-missing skip with notice, `[no-release]` skip, remote tag-spam guard, cold-start bump.
- `.github/dependabot.yml` — weekly cargo/npm/actions updates.

## Known environment note

OpenSpec Codex skill generation initially hit a read-only sandbox directory. The integration was regenerated successfully in the writable host checkout with `openspec init --tools codex --force` and `openspec update --force`. Restart the IDE if slash commands are not visible.

## Next action

**v0.6 (Integrate) is complete.** All three v0.6 change packages
(`mcp-read-tools`, `agent-examples`, `github-actions-templates`) are
archived as of 2026-09-14, the packaging suite runs 7/7 green, and
the Rust test suite still passes (396 tests). The next planning
milestone is **v1.0 (Stable)**, which under ROADMAP.md groups the
"future hardening changes" that stabilize schema, config, checker
protocol, CLI, cross-platform behavior, documentation, and
real-project validation. No v1.0 change package has been proposed
yet; the next step is to scope the first v1.0 change (likely a
schema/config freeze + extended real-project validation sweep) and
add a `proposal.md` / `tasks.md` under `openspec/changes/`.

Follow the "Change completion workflow" at the top of this file
whenever the next change is ready to archive.
