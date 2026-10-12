# generic-gate-contract Specification (delta: mandatory-governance-quality-all-profiles)

## ADDED Requirements

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
