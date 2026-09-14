## ADDED Requirements
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
