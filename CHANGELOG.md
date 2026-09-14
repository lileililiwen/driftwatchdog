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

### Fixed
- Installer verification is mandatory (no skip flag); unsupported hosts
  print the target list plus the source-build fallback.
- Auto-tag skips with a notice when `REPO_PAT` is absent, avoids tag spam
  when the remote tag exists, and cold-starts at `0.1.0`.
