# source-file-size-gate Specification

## Purpose
TBD - created by archiving change source-file-size-gate. Update Purpose after archive.
## Requirements
### Requirement: Provide a built-in source-file-size concern

The Gate MUST recognize `source-file-size` as a built-in product-quality
concern and MUST select it by default in the `product` and `rust-product`
profiles. The concern MUST execute in Driftwatchdog without requiring a
project command.

#### Scenario: Product profile schedules the common checker

- **WHEN** a project resolves the `product` or `rust-product` profile
- **THEN** the resolved plan contains a required `source-file-size` concern
  and execution does not require `commands.source-file-size`

#### Scenario: Existing project command concerns remain compatible

- **WHEN** a project selects `product-code-boundary` or
  `placeholder-threshold` with a command binding
- **THEN** those concerns continue through the existing project-runtime and
  product-quality adapter path, while `source-file-size` uses the in-process
  scanner

### Requirement: Resolve a repository-owned source boundary

The scanner MUST count only candidate repository files selected by the source
boundary policy. It MUST use Git-listed paths and Git's standard exclusion
state when the project is a Git repository, MUST honor `.gitignore` for
untracked content, MUST exclude dependency/vendor/build/generated/test fixture
trees by default, and MUST support repository-relative configured `include` and
`exclude` patterns. Excludes MUST override includes. Symlinks MUST NOT be
followed and binary/invalid UTF-8 files MUST be skipped.

#### Scenario: Ignored dependency tree is excluded

- **GIVEN** a repository contains a 10,000-line file under `node_modules/` or
  `target/` and that tree is Git-ignored
- **WHEN** the source-size concern runs with default policy
- **THEN** the dependency file is not a candidate and does not produce a
  failure

#### Scenario: Conventional source root is discovered

- **GIVEN** a repository contains `src/feature.rs` and no explicit `include`
  patterns
- **WHEN** the source-size concern runs
- **THEN** `src/feature.rs` is a candidate and is counted

#### Scenario: Explicit boundary overrides discovery

- **GIVEN** `[source_size] include = ["packages/service/**"]` and
  `exclude = ["packages/service/generated/**"]`
- **WHEN** the source-size concern runs
- **THEN** only matching non-excluded repository files are candidates

#### Scenario: Symlink and binary boundaries are safe

- **GIVEN** an included path is a symlink or contains binary/NUL content
- **WHEN** the source-size concern runs
- **THEN** Driftwatchdog does not follow or count it as source content

### Requirement: Count raw physical lines and fail oversized files

The scanner MUST count each `0x0A` byte in each included regular UTF-8 file.
It MUST use a default maximum of 1,000 lines per file, allow a valid configured
positive maximum, and MUST return `FAIL` with bounded repository-relative
findings for every included file above the maximum. A required failing result
MUST block the Gate.

#### Scenario: Boundary at 1,000 lines passes

- **GIVEN** an included source file contains exactly 1,000 newline bytes
- **WHEN** the required source-size concern runs
- **THEN** the concern returns `PASS` and the Gate is not blocked by it

#### Scenario: 1,001 lines fail

- **GIVEN** an included source file contains 1,001 newline bytes
- **WHEN** the required source-size concern runs
- **THEN** it returns `FAIL`, identifies the relative path and actual/maximum
  counts in a bounded finding, and the Gate blocks

#### Scenario: Unterminated final line follows wc semantics

- **GIVEN** a file has 1,000 newline bytes plus an unterminated final line
- **WHEN** the source-size concern runs
- **THEN** it counts 1,000 lines and does not fail solely because of the final
  unterminated bytes

### Requirement: Fail closed on boundary or read uncertainty

The scanner MUST continue safe independent reads after an individual read
error, but any Git-boundary, ignore-evaluation, path, or candidate-read error
that prevents complete evaluation MUST return `REVIEW_REQUIRED` with bounded
diagnostic and missing-evidence fields. It MUST NOT convert incomplete
coverage into `PASS`.

#### Scenario: Candidate read fails

- **GIVEN** a selected candidate cannot be read
- **WHEN** the scanner runs
- **THEN** it records `REVIEW_REQUIRED`, names the bounded operation/path, and
  does not claim a complete PASS

#### Scenario: No candidate files exist

- **GIVEN** the source boundary resolves successfully but selects no files
- **WHEN** the required concern runs
- **THEN** it returns `PASS` with evidence that zero candidates were evaluated

### Requirement: Preserve Gate configuration and output contracts

The source-size policy MUST be represented by a strict `[source_size]` manifest
section with `max_lines`, `include`, and `exclude`; invalid, absolute, or
traversal patterns MUST fail configuration before execution. Policy changes
MUST affect the manifest digest. Human/JSON output and existing `gate_runs`
storage MUST use the existing bounded Gate contracts.

#### Scenario: Dry run is side-effect free

- **WHEN** `driftwatch gate --dry-run` resolves a profile containing
  `source-file-size`
- **THEN** it renders the plan and policy without scanning, writing files, or
  persisting a Gate run

#### Scenario: Disabled concern is explicit

- **WHEN** a manifest explicitly disables `source-file-size`
- **THEN** the concern is absent from the resolved plan and existing manifest
  opt-out behavior applies; no scan runs

#### Scenario: Foreign runtime remains untouched

- **WHEN** `.ai-gate/gate.yaml` names a runtime other than Driftwatchdog
- **THEN** no source-size scan runs and no Gate run is persisted

