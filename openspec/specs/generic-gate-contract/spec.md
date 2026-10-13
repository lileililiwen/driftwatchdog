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

A run's status SHALL be derived from the results of the concerns it actually
bound. A concern that is enabled but unbound SHALL contribute an `UNBOUND`
result and SHALL NOT be reported as a pass, and a run containing any `FAIL`,
`REVIEW_REQUIRED` or `UNBOUND` result SHALL be gated according to the project's
declared `blocking` list rather than reported as satisfied.

#### Scenario: Blocking failure

- **WHEN** any required result is `FAIL`
- **THEN** the aggregate status is `FAIL`

#### Scenario: Blocking review

- **WHEN** `REVIEW_REQUIRED` is configured as blocking
- **THEN** an unresolved review blocks the aggregate result

#### Scenario: All optional checks not applicable

- **WHEN** optional checks are `NOT_APPLICABLE` and required checks pass
- **THEN** the aggregate status is `PASS` with explicit not-applicable records

#### Scenario: One concern fails among many that pass

- **WHEN** a run binds several concerns, all but one of which report `PASS`, and
  the remaining one reports `FAIL`
- **THEN** the run is gated as failing, and the report names the failing concern

#### Scenario: A concern is unbound in a project that binds others

- **WHEN** a project binds two of three enabled concerns
- **THEN** the third is reported as `UNBOUND` for that project, and the run does
  not present the project as fully covered

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

### Requirement: A bound concern reports only the codes it owns

A Gate concern SHALL declare the set of finding codes it reports on, and SHALL
report only findings whose code is in that set. It SHALL report the filtered
finding count, never the producing checker's total, so that a caller reading the
evidence can distinguish a scoped result from an unscoped backlog.

#### Scenario: A scoped run over a large backlog

- **WHEN** a producing checker reports 215 findings and the concern's code set
  contains 4 of them
- **THEN** the concern reports 4 findings and its evidence line states 4, not 215

#### Scenario: A finding outside the set

- **WHEN** a producing checker reports a `LICENSE_MISSING` finding and
  `LICENSE_MISSING` is not in the concern's code set
- **THEN** the concern does not report it, and does not count it

#### Scenario: A project narrows the set

- **WHEN** a project declares a code set that is a subset of the concern's
  declared codes
- **THEN** the concern reports only findings matching that subset

#### Scenario: A project tries to widen the set

- **WHEN** a project declares a code that the concern does not declare
- **THEN** the configuration is refused and reported as `REVIEW_REQUIRED` naming
  the unsupported code

#### Scenario: A project empties the set

- **WHEN** a concern is enabled and its effective code set is empty
- **THEN** the result is `REVIEW_REQUIRED` naming the empty scope, and is never
  `PASS`, because a scope that reports nothing is indistinguishable from a
  silently disabled check

### Requirement: A cannot-run checker is never a pass

A bound concern SHALL classify an outcome where the producing checker could not
complete, and SHALL NOT report `PASS` for any such outcome. A checker that did
not start at all SHALL be `UNBOUND`; a checker that started and could not finish
— an unexpected exit status, unparseable output, a timeout, or a result carrying
a level the concern does not recognise — SHALL be `REVIEW_REQUIRED`.

#### Scenario: The producing checker is absent

- **WHEN** the command bound to the concern does not exist
- **THEN** the result is `UNBOUND` with exit 1, and the reason names the missing
  path

#### Scenario: The producing checker misbehaves

- **WHEN** the command exits with a status the concern does not expect, or its
  output cannot be parsed, or it times out
- **THEN** the result is `REVIEW_REQUIRED` with exit 2 and the exit status and
  stderr as evidence

#### Scenario: The contract moves underneath the concern

- **WHEN** a finding carries a level the concern's mapping does not define
- **THEN** the result is `REVIEW_REQUIRED` naming the unrecognised level, and is
  never silently treated as a warning

#### Scenario: A checker that started is not reported as missing

- **WHEN** a checker runs and cannot finish, while a different project has no
  checker bound at all
- **THEN** the first is `REVIEW_REQUIRED` and the second is `UNBOUND`, and the
  two are not collapsed into one status

### Requirement: A concern is reported as unbound where no binding exists

A concern that is enabled in a profile but has no bound command in a given
project SHALL be reported as `UNBOUND` for that project. Enabling a concern in a
profile SHALL NOT be reported as coverage of a project that does not bind it.

#### Scenario: A concern enabled but not bound in a project

- **WHEN** a profile enables a concern and a project declares no command for it
- **THEN** that project records `UNBOUND` for the concern, and the run's summary
  distinguishes it from a project that bound the concern and passed it

### Requirement: A code-scoped quality concern is not exempable

A concern that owns an ERROR-severity finding code SHALL NOT be removable
through the exemption mechanism, and the exemption cap and the non-exemptable
alias list SHALL be unchanged by this requirement.

#### Scenario: An exemption targets the concern

- **WHEN** a project attempts to exempt a concern that owns an ERROR-severity
  code
- **THEN** the attempt is refused and the concern still runs

#### Scenario: The existing cap is unaffected

- **WHEN** exemptions are granted for concerns that are exempable
- **THEN** the cap of `min(floor(0.25 x required), 5)` and the non-exemptable
  alias list behave exactly as before this capability

