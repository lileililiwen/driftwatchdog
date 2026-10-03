# readiness Specification

## Purpose

Driftwatchdog v0.1–v0.6 and the v1.1–v1.3 Gate releases are shipped and the packaging suite is green. The README is usable but has three documentation gaps that publication readiness should close: 1. No screenshot or checked-in terminal capture. The README shows fenced command text, but nothing shows an actual `driftwatch run` failure (§"Example"), a detected recurring bug, or a Gate run, so a reader cannot see the product working. 2. Rule catalog depth. The built-in profiles (`product`, `rust-product`, `release`) and stable concern IDs (`product-code-boundary`, `placeholder-threshold`, `release-evidence`, `capability-conformance`) are documented in separate prose sections; there is no single catalog tying each profile to its concerns, blocking default, and text-mode fallback behaviour. 3. Integration recipe. The README explains `.ai-gate/gate.yaml` and `gate.toml` in pieces, but a project owner has no ordered recipe from "no manifest" to a first `driftwatch gate` run. Driftwatchdog owns the central workspace Gate: 75 `.ai-gate/gate.yaml` declarations name `runtime: driftwatchdog`, so this README is a shared reference for the portfolio. The change is documentation-only and does not expand the Gate.
## Requirements
### Requirement: README embeds a redacted terminal capture

The README SHALL embed at least one checked-in terminal capture showing a
`driftwatch run` failure and the recurring-bug report (and MAY include a Gate
run), stored under `docs/assets/`. The capture SHALL come from a synthetic local
project and SHALL be redacted of secrets and real repository data.

#### Scenario: Reader sees the tool work

- **WHEN** a reader opens the README example/capture section
- **THEN** a checked-in terminal capture shows a recorded failure and the
  recurring-bug output

#### Scenario: Capture is unredacted or external (negative)

- **WHEN** the capture contains secrets or real repository data, or is hosted on
  an external image host
- **THEN** it MUST NOT be committed

### Requirement: Built-in profile and concern catalog

The README SHALL include a consolidated catalog mapping each built-in profile to
its stable concern IDs, its required/optional default, the exit-code authority
rule, and whether a text-mode fallback applies. The catalog SHALL match
`src/gate/concerns.rs` and the canonical specs, and SHALL identify Driftwatchdog
as the shared workspace Gate runtime.

#### Scenario: Reader looks up a concern

- **WHEN** a reader looks up a profile or concern
- **THEN** the catalog names its profile, default, exit-code mapping, and
  text-mode fallback behavior

#### Scenario: Catalog names an unknown concern (negative)

- **WHEN** the catalog names a concern or profile absent from
  `src/gate/concerns.rs`
- **THEN** the change is rejected; the code is authoritative

#### Scenario: Release concern given a text fallback (negative)

- **WHEN** the catalog claims a text-mode fallback for `release-evidence` or
  `capability-conformance`
- **THEN** the change is rejected, because those concerns deliberately have none

### Requirement: Ordered integration recipe

The README SHALL include an ordered recipe from "no manifest" to a first
`driftwatch gate` run, covering `gate.toml`, `.ai-gate/gate.yaml`, blocking
policy, and optional context providers. The recipe SHALL describe Driftwatchdog
as an executor and aggregator only.

#### Scenario: Project owner integrates the Gate

- **WHEN** a project owner follows the recipe
- **THEN** they reach a first `driftwatch gate` run and see the resolved plan and
  result

#### Scenario: Recipe implies publishing (negative)

- **WHEN** the recipe implies Driftwatchdog publishes, signs, generates an SBOM,
  or deploys
- **THEN** the change is rejected

### Requirement: Readiness does not expand the Gate

The change SHALL NOT modify `src/**`, concern IDs, envelope wire versions, the
exit-code authority rule, or any CLI surface, and SHALL NOT add a Gate profile,
concern, rule, or checker.

#### Scenario: Boundary review

- **WHEN** the change diff is reviewed
- **THEN** only `README.md`, `docs/**`, and the change folder are touched, and
  no Gate behavior changed

### Requirement: Existing packaging and workflow checks stay green

The change SHALL keep the packaging suite, including the BFS/DFS/BFS change
workflow check and repo-hygiene check, and strict OpenSpec validation green.

#### Scenario: Packaging suite runs (negative)

- **WHEN** this active change lacks a BFS/DFS/BFS task shape or a Scenario
- **THEN** `tests/packaging/test_change_workflow.sh` fails

