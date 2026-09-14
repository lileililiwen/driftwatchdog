# Proposal: Quality gates, release integrity, docs and UX

## Why
Audit found maturity gaps across quality/CI/CD/docs/UX: `Cargo.toml` has no `[lints]`, no `rust-version`/MSRV, loose reqs (`toml 0.8`, `rusqlite 0.31`, `thiserror 1`), no `cargo-deny`/`audit` or coverage, and no proptest/fuzz for the arbitrary-input normalizer; `ci.yml:41` runs clippy without `--all-features` (contradicts AGENTS.md/HANDOFF.md gate), single `ubuntu-22.04` job, no macOS/arm64 matrix, no `shellcheck`, no `npm audit`, `scripts/smoke.sh` not in CI; `release.yml` uses aging `macos-13`, `npm-publish` lacks `id-token: write` so `npm publish --provenance` fails, Cargo version stays `0.1.0` while npm pins to tag (drift), `auto-tag.yml` hard-fails without `REPO_PAT` + risks tag spam, `bump.sh` fails cold-start, no Dependabot, no signing/SBOM; docs drift: binary naming triple (`driftwatchdog` cargo bin vs clap `driftwatch` vs npm `driftwatch`) with README quickstart/npm section mismatch, `ROADMAP.md` omits archived `ci-test-gates`, `release-distribution/spec.md:3` still `Purpose: TBD`, HANDOFF 5-step vs AGENTS 6-step close workflow, no `driftwatch.toml.example` (placeholder + git-ignored), installer `skip_verify` dead flag + undocumented `--dry-run` + no `cargo install` fallback on unsupported hosts, npm `os/cpu` permits `darwin+arm64` install-then-fail, CLI `--help` thin with leaked internals and no examples/completions/man, double-wrapped terse errors with no remediation, `doctor` severity misuse (`checker.configured` PASS on "No external checkers", `remediation:` for info), `LICENSE` ok but no CHANGELOG, musl/Windows source-build compat unknown, stray `.driftwatch/` in repo root.

## What Changes
- Quality: `[lints]` + MSRV + pinned policy, `cargo-deny`, coverage, proptest/fuzz seeds for normalizer.
- CI: `--all-features` parity, macOS/arm64 matrix, shellcheck, npm audit, smoke in CI.
- Release: `id-token: write`, runner refresh, single version source (tag ↔ Cargo ↔ npm check), auto-tag hardening, Dependabot, SBOM/signing story, CHANGELOG.
- Docs/UX: single naming rule documented at install line, accurate README/ROADMAP/HANDOFF/spec purpose, `driftwatch.toml.example`, installer fallback + darwin-arm64 guard, rich `--help` + completions/man, actionable errors, honest doctor severities.

## Capabilities
### New Capabilities
- `quality-cicd-docs-ux`: lint/MSRV/audit/coverage policy, CI matrix parity, release integrity, doc accuracy, CLI UX.
### Modified Capabilities
- `ci-test-gates`: extend gates (features matrix, shellcheck, npm audit, smoke).
- `release-distribution`: provenance permissions, version-source rule, SBOM/signing note.

## Impact
Affects: `Cargo.toml`, `deny.toml` (new), `.github/workflows/ci.yml`, `.github/workflows/release.yml`, `.github/workflows/auto-tag.yml`, `.github/dependabot.yml` (new), `scripts/*.sh`, `scripts/lib/*`, `npm/driftwatchdog/package.json`, `src/cli.rs`, `src/main.rs`, `src/error.rs`, `src/doctor/mod.rs`, `scripts/install.sh`, `README.md`, `ROADMAP.md`, `HANDOFF.md`, `AGENTS.md`, `openspec/specs/release-distribution/spec.md`, `CHANGELOG.md` (new), `driftwatch.toml.example` (new).
