# quality-cicd-docs-ux Specification

## Purpose

Audit found maturity gaps across quality/CI/CD/docs/UX: `Cargo.toml` has no `[lints]`, no `rust-version`/MSRV, loose reqs (`toml 0.8`, `rusqlite 0.31`, `thiserror 1`), no `cargo-deny`/`audit` or coverage, and no proptest/fuzz for the arbitrary-input normalizer; `ci.yml:41` runs clippy without `--all-features` (contradicts AGENTS.md/HANDOFF.md gate), single `ubuntu-22.04` job, no macOS/arm64 matrix, no `shellcheck`, no `npm audit`, `scripts/smoke.sh` not in CI; `release.yml` uses aging `macos-13`, `npm-publish` lacks `id-token: write` so `npm publish --provenance` fails, Cargo version stays `0.1.0` while npm pins to tag (drift), `auto-tag.yml` hard-fails without `REPO_PAT` + risks tag spam, `bump.sh` fails cold-start, no Dependabot, no signing/SBOM; docs drift: binary naming triple (`driftwatchdog` cargo bin vs clap `driftwatch` vs npm `driftwatch`) with README quickstart/npm section mismatch, `ROADMAP.md` omits archived `ci-test-gates`, `release-distribution/spec.md:3` still `Purpose: TBD`, HANDOFF 5-step vs AGENTS 6-step close workflow, no `driftwatch.toml.example` (placeholder + git-ignored), installer `skip_verify` dead flag + undocumented `--dry-run` + no `cargo install` fallback on unsupported hosts, npm `os/cpu` permits `darwin+arm64` install-then-fail, CLI `--help` thin with leaked internals and no examples/completions/man, double-wrapped terse errors with no remediation, `doctor` severity misuse (`checker.configured` PASS on "No external checkers", `remediation:` for info), `LICENSE` ok but no CHANGELOG, musl/Windows source-build compat unknown, stray `.driftwatch/` in repo root.
## Requirements
### Requirement: Lint and supply-chain policy
The repo MUST declare lints/MSRV and MUST gate advisories, licenses, and coverage.

#### Scenario: New clippy warning
- **WHEN** a PR introduces a clippy warning under `--all-targets --all-features`
- **THEN** CI fails with `-D warnings`

#### Scenario: Known advisory
- **WHEN** a dependency has a published RustSec advisory
- **THEN** `cargo deny check advisories` fails in CI

#### Scenario: Normalizer fuzz
- **WHEN** proptest feeds 10k arbitrary unicode strings
- **THEN** normalization never panics and golden fixtures stay stable

### Requirement: CI parity and breadth
CI MUST match the HANDOFF gate flags and MUST cover macOS, shell, npm, and smoke.

#### Scenario: Feature-gated code
- **WHEN** code is behind a feature flag
- **THEN** clippy still lints it (`--all-features`)

#### Scenario: Shell change
- **WHEN** `scripts/install.sh` has a bashism
- **THEN** shellcheck fails the PR

#### Scenario: Smoke regression
- **WHEN** packaging breaks end-to-end install
- **THEN** the smoke job fails before release

### Requirement: Release integrity
Releases MUST publish with provenance, consistent versions, SBOM, and a changelog.

#### Scenario: npm publish
- **WHEN** a tag `v1.2.3` builds
- **THEN** `npm publish --provenance` succeeds (OIDC present), `Cargo.toml`, tag, and npm version all equal `1.2.3`, and SBOM + CHANGELOG entry ship

#### Scenario: Cold-start bump
- **WHEN** no prior tag exists
- **THEN** bump creates the initial tag instead of exiting nonzero

#### Scenario: Missing PAT
- **WHEN** `REPO_PAT` is absent
- **THEN** auto-tag skips with a notice instead of hard-failing the workflow

### Requirement: Accurate docs and naming
README/ROADMAP/HANDOFF/specs MUST agree on binary names, inventory, and workflow; the checker story MUST be copy-paste runnable.

#### Scenario: npm install
- **WHEN** a user reads the install section and runs `npm i -g driftwatchdog`
- **THEN** the next line tells them the command is `driftwatch` (not `driftwatchdog`) with a working example

#### Scenario: Checker onboarding
- **WHEN** a user copies the README checker example + `driftwatch.toml.example`
- **THEN** `driftwatch check` succeeds against a stub checker without edits

#### Scenario: Spec purpose
- **WHEN** reading `release-distribution/spec.md`
- **THEN** Purpose is a real paragraph, not `TBD`, and ROADMAP lists `ci-test-gates` in the closed inventory

### Requirement: Humane CLI UX
Help MUST include examples; errors MUST include remediation hints; doctor severities MUST be honest; installer/npm MUST fail with a next step.

#### Scenario: Bad show id
- **WHEN** the user runs `driftwatch show bad$$$`
- **THEN** the error is single-wrapped with `hint: try 'driftwatch list' … (prefix needs 8+ hex chars)`

#### Scenario: No checkers configured
- **WHEN** `doctor` runs with zero checkers
- **THEN** it reports INFO (not PASS) with `hint: add [[checkers]] or run with --dry-run`

#### Scenario: darwin-arm64 via npm
- **WHEN** installing on darwin-arm64
- **THEN** npm fails fast naming supported targets + shell-installer and `cargo install` alternatives

#### Scenario: Unsupported host via shell
- **WHEN** the installer runs on musl/Windows
- **THEN** it prints the target list plus the `cargo install --locked` fallback

#### Scenario: Shell completions
- **WHEN** the user installs completions/man
- **THEN** generated `complete`/`man` artifacts exist from `clap_complete`/`clap_mangen`

