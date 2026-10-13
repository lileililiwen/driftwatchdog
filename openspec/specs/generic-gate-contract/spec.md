# generic-gate-contract Specification

## Purpose

Driftwatchdog has a checker protocol for external alerts but no general contract for gate plans, evidence, findings, and `PASS`/`FAIL`/`REVIEW_REQUIRED` results. The contract must be language-agnostic and must not depend on OpenSpec.
## Requirements
### Requirement: Normalize gate results

The system MUST represent deterministic and semantic checks with one versioned
result model containing status, source, severity, findings, evidence references,
missing evidence, diagnostics, and remediation. The status vocabulary MUST keep
`UNBOUND` distinct from `NOT_APPLICABLE`: `NOT_APPLICABLE` is a verdict that
something was evaluated and found inapplicable, while `UNBOUND` records that a
check asserted coverage and no command ran at all. A check whose result is
`UNBOUND` MUST have an empty evidence list, MUST record the absent command as
missing evidence, and MUST carry a finding naming the check and the absent
command. The exemption budget of one project is normalized as its own result:
reaching the project's computed limit is a `REVIEW_REQUIRED` result with
`Warning` severity, while exceeding the limit or exempting `build`, `tests`,
`security`, or `repository` — or a workspace alias of one of them, such as
`repository_integrity` — is a `FAIL` result with `Error` severity.

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

#### Scenario: Required check that binds no command

- **WHEN** a required check is asserted and no command is bound to it
- **THEN** the normalized result has status `UNBOUND` and `Error` severity, its
  evidence list is empty, its missing evidence records the absent command, and
  its findings name the check and the absent command — never a
  `NOT_APPLICABLE` result, which would claim the check was evaluated

#### Scenario: Optional check that binds no command

- **WHEN** an optional check is asserted and no command is bound to it
- **THEN** the normalized result has status `UNBOUND` and `Warning` severity,
  the absence is recorded, and the result never blocks the aggregate

#### Scenario: Acknowledged exemption

- **WHEN** a check declares an exemption naming the check and the reason it does
  not apply, and no command is bound
- **THEN** the normalized result has status `NOT_APPLICABLE` with the reason
  recorded, is never recorded as `UNBOUND`, and does not block the aggregate

#### Scenario: Exemption budget at the limit

- **WHEN** a project's declared exemptions reach its computed limit
- **THEN** the normalized budget result has status `REVIEW_REQUIRED` and
  `Warning` severity, its diagnostic names the declared count, the limit, and
  the exhausted headroom, and it is never a failure

#### Scenario: Exemption budget over the limit or a non-exempable check

- **WHEN** a project's declared exemptions exceed its computed limit, or one of
  them names `build`, `tests`, `security`, `repository`, or a workspace alias
  of one of those checks
- **THEN** the normalized budget result has status `FAIL` and `Error` severity,
  and the exempted non-exempable check is never normalized as
  `NOT_APPLICABLE`

### Requirement: Aggregate gate results

The system MUST aggregate results using an explicit blocking policy and MUST NOT
silently convert `REVIEW_REQUIRED` into `PASS`.

A run's status SHALL be derived from the results of the concerns it actually
bound. A concern that is enabled but unbound SHALL contribute an `UNBOUND`
result and SHALL NOT be reported as a pass. A run containing a `FAIL` or a
`REVIEW_REQUIRED` result SHALL be gated according to the project's declared
`blocking` list rather than reported as satisfied; **a required `UNBOUND`
supersedes that list**, because the project would otherwise be able to reduce an
attempted-nothing required check to a pass by editing the list it declares. A
required result with status `UNBOUND` MUST block the aggregate and MUST produce
aggregate status `FAIL` independently of the configured blocking policy. The
aggregate MUST keep unbound checks in a collection distinct from proven or
acknowledged not-applicable checks, so a single run distinguishes an absence
that was evaluated from one that was never attempted. A failing
exemption-budget result MUST block the aggregate on the same terms as any other
required failure, and a warning-severity budget result MUST be recorded without
blocking.

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

#### Scenario: Required unbound check blocks independently of the blocking policy

- **WHEN** a required result is `UNBOUND` and `REVIEW_REQUIRED` is not configured
  as blocking
- **THEN** the aggregate status is `FAIL`, the outcome is blocked, the check is
  listed as a failure and as unbound, and the run is never `PASS`

#### Scenario: Unbound and not-applicable stay separate

- **WHEN** one run contains both a not-applicable check and an unbound check
- **THEN** each check appears only in its own collection, and the persisted run
  record carries the unbound collection together with the per-check unbound
  results

#### Scenario: An exemption-budget failure blocks independently of the blocking policy

- **WHEN** the exemption-budget result is `FAIL` and `REVIEW_REQUIRED` is not
  configured as blocking
- **THEN** the aggregate status is `FAIL`, the outcome is blocked, and the
  budget check is listed as a failure

#### Scenario: An at-the-limit budget warning is recorded and does not block

- **WHEN** the exemption-budget result is `REVIEW_REQUIRED` with `Warning`
  severity and every other required result passes
- **THEN** the aggregate status is `PASS`, the budget check is listed as an
  optional pending review, and the warning is never converted into a failure

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

The declared set SHALL cover every code the bound checkers can emit at ERROR.
A code that exists in a producing checker but not in the declared set is
invisible to the gate, and a gate that cannot see a failing check is worse than
a check that does not exist, because it reports a clean result.

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

#### Scenario: A checker emits an ERROR code the concern does not declare

- **WHEN** the producing checker emits `QUALITY_NEW_CHANGE_WITH_GATE_DEBT` at
  ERROR and exits 1, and that code is absent from the declared set
- **THEN** the concern reports `PASS` with "0 of N governance findings matched
  this concern's scope" and exits 0, which is a false green
- **AND** this is a defect in the declared set, not a permitted configuration,
  so the code is added rather than the finding being allowed to pass

#### Scenario: The code is declared

- **WHEN** the producing checker emits `QUALITY_NEW_CHANGE_WITH_GATE_DEBT` at
  ERROR and exits 1, and that code is in the declared set
- **THEN** the concern reports `FAIL`, counts 1, names the rule in the finding,
  and maps to exit 1

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

### Requirement: A declared code states why it is declared

Every code in `GOVERNANCE_QUALITY_DECLARED_CODES` SHALL be accompanied by a
statement of the failure its absence would cause, so that a later reader can
tell an intentional exclusion from an oversight.

#### Scenario: A code is added

- **WHEN** a code is added to the declared set
- **THEN** the rationale for declaring it is recorded beside the constant

### Requirement: A declared code is owned before its producer emits it

A code-scoped concern SHALL own every code in its declared set regardless of
whether the producing checker has shipped that code yet. Declaring a code ahead
of emission SHALL make the concern able to report the finding, and SHALL NOT by
itself alter the concern's status, severity or exit code. The status SHALL be
derived only from findings actually observed in the report, and a declared code
for which the report carries no finding SHALL NOT be named in the result.

#### Scenario: The checker emits a newly declared code

- **WHEN** the producing checker reports a finding whose code is in the concern's
  declared set at level `ERROR`
- **THEN** the concern reports it as a finding, its status is `FAIL` with exit 1,
  and the finding's rule names the code

#### Scenario: The checker has not emitted a declared code

- **WHEN** a report carries no finding for a declared code
- **THEN** the concern's status is the mapping of the findings it did observe, the
  declared code contributes no finding, and it is not named in the diagnostic,
  because a code with no observed finding is not a result

#### Scenario: A project narrows its scope to the newly declared code

- **WHEN** a project declares a code scope consisting only of a code that the
  concern declares
- **THEN** the scope is accepted, and a matching finding for that code is reported,
  rather than refused as an undeclared widening

#### Scenario: A declared code and an undeclared code are not conflated

- **WHEN** the concern's declared set and the codes observed in a report do not
  correspond one to one, because a declared code produced no finding
- **THEN** the concern reports the observed match count and the checker's total, and
  SHALL NOT infer, assert, or report any divergence between the declared set and
  the producing checker's full set of emittable codes, because the producing
  checker's report does not carry that set

### Requirement: A closed declared-code list is maintained against its producer

The set of finding codes a code-scoped concern owns SHALL be a closed
allow-list, kept in sorted order, and a project SHALL be refused for naming a code
outside it. Adding a code the producing checker does not yet emit SHALL NOT change
the canonical form of a manifest that does not declare a code scope, and therefore
SHALL NOT change that manifest's digest.

#### Scenario: The default set grows

- **WHEN** a code is added to the concern's declared set
- **THEN** a project that configures no code scope inherits the new default, and a
  project that configured a scope keeps the scope it declared

#### Scenario: The list stays a ceiling

- **WHEN** a project declares a code the concern does not declare
- **THEN** the configuration is refused before any command runs, exactly as before
  the list grew

#### Scenario: A default manifest's digest is stable

- **WHEN** a code is added to the concern's declared set
- **THEN** a manifest that omits the code scope reports the same digest as before,
  because the scope is serialized only when it differs from the default

### Requirement: The verification-binding drift code is owned before its producer emits it

The `governance-quality` concern's declared code set SHALL contain
`QUALITY_VERIFICATION_COMMAND_UNBOUND`, so that a repository declaring a
`verification.command` whose Gate binds no command is a gate result rather than
a report line no gate reads. The declaration SHALL land in the same change as
the emitting rule, because the concern filters report findings through this
closed set and treats the producing checker's non-zero exit as an *expected*
exit: while the code is outside the set, a checker emitting only this code would
exit non-zero with a report the concern filters to nothing, and the concern
would report `PASS` while the checker fails.

#### Scenario: The checker emits the verification-binding code

- **WHEN** the producing checker reports a finding whose code is
  `QUALITY_VERIFICATION_COMMAND_UNBOUND` at level `ERROR`
- **THEN** the concern reports it as a finding, its status is `FAIL` with exit 1,
  and the finding's rule names the code

#### Scenario: A project narrows its scope to the verification-binding code

- **WHEN** a project declares a code scope consisting only of
  `QUALITY_VERIFICATION_COMMAND_UNBOUND`
- **THEN** the scope is accepted as a subset of the declared set, and a matching
  finding for that code is reported, rather than refused as an undeclared
  widening

#### Scenario: The code is declared but not yet emitted

- **WHEN** a report carries no finding for `QUALITY_VERIFICATION_COMMAND_UNBOUND`
- **THEN** the concern's status is the mapping of the findings it did observe, the
  declared code contributes no finding, and it is not named in the diagnostic,
  because a declared code with no observed finding is not a result

### Requirement: A bound declared command reports its own failure

A concern bound to a repository's declared `verification.command` SHALL derive
its status from the command's exit code through the project-runtime text
adapter: exit `0` is `PASS`, a non-zero exit is `FAIL`, and a timeout or a
failed spawn is `REVIEW_REQUIRED` with missing evidence. A failing declared
command SHALL NOT be suppressed, skipped, softened, or reported as a pass to
keep a gate green.

#### Scenario: The declared command passes

- **WHEN** the bound command exits `0`
- **THEN** the concern's status is `PASS` with no findings

#### Scenario: The declared command fails

- **WHEN** the bound command exits non-zero
- **THEN** the concern's status is `FAIL` with severity `error`, and the
  command's own output is preserved in the diagnostic behind the exit code

#### Scenario: The declared command cannot finish

- **WHEN** the bound command exceeds the check timeout
- **THEN** the concern's status is `REVIEW_REQUIRED` and the result lists the
  timeout as missing evidence, because nothing was concluded

#### Scenario: The declared command is bound to a concern that is not selected

- **WHEN** a manifest binds `commands.declared-verification` without selecting
  `declared-verification` in `checks`
- **THEN** manifest parsing fails naming the unselected concern, because a
  binding with no selection would never run

