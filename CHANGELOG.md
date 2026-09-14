# Changelog

All notable changes to Driftwatchdog are documented here. Format follows
[Keep a Changelog](https://keepachangelog.com/en/1.0.0/).

## [Unreleased]

### Added
- `driftwatch mcp`: stdio Model Context Protocol server with four
  read-only tools (`top_bugs`, `show_bug`, `ai_report`,
  `doctor_status`). JSON-RPC 2.0 / `protocolVersion 2024-11-05`,
  closed input schemas (`additionalProperties: false`), and a
  read-only SQLite connection. No new runtime dependencies.
- Agent example collateral under `examples/{claude-code,opencode,aider}/`:
  copy-paste `mcp.json` (stdio `driftwatch` server) plus an
  investigation `workflow.md` that mirrors the `report --ai`
  instructions. Aider's directory documents the `report --ai >
  drift.md` fallback for every Aider version. `examples/README.md`
  maps harnesses to their copy destination and restates the
  stdio-only, read-only caveat.
- GitHub Actions template at `templates/github-actions/driftwatch-check.yml`:
  reusable workflow (`workflow_call` + `workflow_dispatch`) that
  pins a `driftwatchdog` install, always renders
  `report --ai > drift.md`, uploads only `drift.md` (never
  `state.db` or command logs), summarises `top` into
  `$GITHUB_STEP_SUMMARY`, and exposes `version` /
  `fail_on_drift` / `upload_report` inputs under `contents: read`.
  `templates/github-actions/README.md` documents the inputs and
  the copy-vs-`uses:` decision.
- Quality gates: `[lints.clippy] all = "deny"`, `rust-version = "1.74"` MSRV,
  `cargo-deny` advisories/licenses policy, tarpaulin coverage, proptest
  seeds for the failure normalizer.
- CI breadth: `--all-features` clippy parity, macOS (`macos-14`) matrix,
  shellcheck, `npm audit`, and `scripts/smoke.sh` jobs.
- Release integrity: npm provenance (`id-token: write`), `macos-14`
  runner, tag==Cargo version assertion, SBOM (SPDX) attached to releases.
- UX: `driftwatch completions <shell>`, `driftwatch man`, per-command
  examples in `--help`, single-wrap errors with `hint: …`, honest
  `doctor` INFO severities, installer `--dry-run` docs + `cargo install`
  fallback, `driftwatch.toml.example` copy-paste checker.

### Packaging tests
- `tests/packaging/test_agent_examples.sh` — every `mcp.json`
  parses, declares a stdio server with `driftwatch` + `mcp`, and
  every `driftwatch <sub>` token in the workflows exists in
  `driftwatchdog --help`; no `run`/`check` is allowed as an
  agent step.
- `tests/packaging/test_gha_templates.sh` — triggers, install
  base URL, `driftwatch check` + `report --ai` + step summary +
  upload-artifact anchors, upload `path: drift.md` only, and
  `fail_on_drift` documented; embedded `run:` blocks pass
  `shellcheck -S error` when the tool is available.

### Fixed
- Installer verification is mandatory (no skip flag); unsupported hosts
  print the target list plus the source-build fallback.
- Auto-tag skips with a notice when `REPO_PAT` is absent, avoids tag spam
  when the remote tag exists, and cold-starts at `0.1.0`.
