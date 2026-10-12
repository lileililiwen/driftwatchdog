# generic-gate-contract Specification

## Purpose

Driftwatchdog has a checker protocol for external alerts but no general contract for gate plans, evidence, findings, and `PASS`/`FAIL`/`REVIEW_REQUIRED` results. The contract must be language-agnostic and must not depend on OpenSpec.
## Requirements
### Requirement: Normalize gate results

The system MUST represent deterministic and semantic checks with one versioned
result model containing status, source, severity, findings, evidence references,
missing evidence, diagnostics, and remediation.

#### Scenario: Successful deterministic result

- **WHEN** an adapter returns no findings and all required evidence is present
- **THEN** the normalized result has status `PASS`

#### Scenario: Missing evidence

- **WHEN** a required evaluator input is unavailable
- **THEN** the normalized result has status `REVIEW_REQUIRED` and lists the
  missing evidence

#### Scenario: Tool finding

- **WHEN** an adapter reports a high-severity finding
- **THEN** the normalized result has status `FAIL`, preserves source and
  location evidence, and includes remediation

### Requirement: Aggregate gate results

The system MUST aggregate results using an explicit blocking policy and MUST NOT
silently convert `REVIEW_REQUIRED` into `PASS`.

#### Scenario: Blocking failure

- **WHEN** any required result is `FAIL`
- **THEN** the aggregate status is `FAIL`

#### Scenario: Blocking review

- **WHEN** `REVIEW_REQUIRED` is configured as blocking
- **THEN** an unresolved review blocks the aggregate result

#### Scenario: All optional checks not applicable

- **WHEN** optional checks are `NOT_APPLICABLE` and required checks pass
- **THEN** the aggregate status is `PASS` with explicit not-applicable records

### Requirement: Preserve checker compatibility

Existing checker alerts MUST remain consumable through an adapter without
changing the current `driftwatch check` protocol.

#### Scenario: Existing alert document

- **WHEN** a configured checker emits the current alerts JSON document
- **THEN** `driftwatch check` records the existing snapshot and can expose an
  equivalent normalized gate result

### Requirement: Governance quality is scheduled for every profile

Driftwatchdog SHALL schedule the `governance-quality` concern exactly once for
every supported built-in, workspace, and project-defined profile. It SHALL
preserve each profile's existing concern set and append governance quality
when absent. Workspace profile names without another built-in concern set
SHALL be enumerated so unknown profile typos remain errors.

#### Scenario: Every built-in profile receives governance quality

- **WHEN** a manifest selects any supported built-in profile
- **THEN** profile resolution schedules `governance-quality` exactly once
- **AND** it preserves the profile's other built-in concerns

#### Scenario: Workspace-specific profile receives governance quality

- **WHEN** a manifest selects one of the supported workspace profiles without
  its own explicit concern set
- **THEN** profile resolution schedules only `governance-quality`

#### Scenario: Project-defined profile keeps its declared checks

- **WHEN** a manifest declares a project-defined profile with its own concern
  list and selects it
- **THEN** profile resolution returns that declared concern list
- **AND** it adds `governance-quality` exactly once

#### Scenario: Fallback concern is unbound

- **WHEN** the fallback schedules `governance-quality`
- **AND** the manifest has no command binding for that concern
- **THEN** the gate reports the required concern as `UNBOUND`
- **AND** the aggregate gate fails under its existing blocking policy

#### Scenario: Unknown profile typo is rejected

- **WHEN** a manifest selects a profile name outside the built-in, workspace,
  and project-defined profile sets
- **THEN** manifest validation rejects it as `UnknownProfile`

#### Scenario: Explicit built-in profiles retain their defaults

- **WHEN** a manifest selects an explicit built-in profile
- **THEN** its existing concern set is unchanged by the fallback behavior

