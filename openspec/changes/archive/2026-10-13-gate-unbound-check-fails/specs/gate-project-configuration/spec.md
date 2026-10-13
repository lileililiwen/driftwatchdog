## MODIFIED Requirements

### Requirement: Map YAML policy onto the existing Gate plan

The system MUST map `checks` selections, an optional `commands` binding,
the `blocking` status list, and optional `project_commands` and
`contexts` onto the existing plan: a selected concern with a declared
command runs through the project-runtime adapter, a required selected
concern without a command records the unbound status rather than
`NOT_APPLICABLE`, a concern executed in-process without a command still
runs, and the `blocking` list sets the review-required policy without
changing `FAIL` blocking. The presence of a check in `checks` is a
declaration of coverage and is never by itself evidence of coverage.

#### Scenario: Selected concern with command

- **WHEN** a selected check has a matching non-empty `commands` entry
- **THEN** the concern is scheduled with that command for execution and
  its result status is whatever the command reports

#### Scenario: Selected concern without command

- **WHEN** a selected check has no command binding
- **THEN** the concern is scheduled and records the unbound status rather than a
  silent pass **or** the non-failing `NOT_APPLICABLE` it used to record, at
  error severity when the check is required and warning severity when it is
  optional; the two cases below state how each status blocks

#### Scenario: Required concern without command

- **WHEN** a check selected `true` has no `commands` entry and no
  `not_applicable` entry
- **THEN** the concern is scheduled and records the unbound status with
  error severity and a remediation naming the missing `commands` key,
  rather than a non-failing `NOT_APPLICABLE`

#### Scenario: Optional concern without command

- **WHEN** a check selected `"optional"` has no `commands` entry
- **THEN** the concern is scheduled and records the unbound status with
  warning severity, is listed as unbound, and does not block the gate

#### Scenario: Concern executed in-process without a command

- **WHEN** a selected concern is executed by the runtime's own scanner
  rather than by a project command, such as the built-in source-size
  concern
- **THEN** the concern runs, reports its own result, and is never recorded
  as unbound

#### Scenario: Review-required blocking follows the list

- **WHEN** `blocking` includes `REVIEW_REQUIRED`
- **THEN** unresolved reviews block the gate; when it omits
  `REVIEW_REQUIRED`, reviews stay explicit but do not block

#### Scenario: Unknown blocking status

- **WHEN** `blocking` contains a status name the system does not define
- **THEN** resolution fails with the offending value and the accepted set

### Requirement: Validate a project Gate manifest

The system MUST load and validate a project-local Gate manifest with version,
profile, checks, blocking policy, project commands, and optional context
providers, and MUST fail that validation before any check executes on an
unknown field, an unsupported version, an empty enabled command, or a malformed
project command, and MUST additionally fail it, under the `reject` binding
policy, when a required enabled check binds no command and declares no
exemption.

#### Scenario: Valid manifest

- **WHEN** a project declares a supported profile and valid checks
- **THEN** the system produces a resolved plan without executing tools

#### Scenario: Unknown field

- **WHEN** the manifest contains an unknown field
- **THEN** loading fails with the field name and a remediation hint

#### Scenario: Invalid project command

- **WHEN** an enabled required command is empty or malformed
- **THEN** plan resolution fails before any check executes

#### Scenario: Empty command string is still rejected

- **WHEN** an enabled check binds an empty or whitespace-only command
  string
- **THEN** resolution fails with the existing empty-command error naming
  the check, and the failure is never reported as an unbound coverage gap

#### Scenario: Required check with no command is rejected under reject

- **WHEN** `binding_policy` is `reject` and a required enabled check binds
  no command and declares no exemption
- **THEN** resolution fails before any check executes and names every
  offending check id in a single refusal

#### Scenario: Rejected manifest records no run

- **WHEN** resolution is refused under the `reject` binding policy
- **THEN** no check executes, no gate run is persisted, and no project
  state is mutated

## ADDED Requirements

### Requirement: Fail closed on a required check with no bound command

The system SHALL report a required check that binds no command and declares
no exemption as a distinct unbound outcome that is separate from a
not-applicable verdict, carries error severity, names the missing command
binding, and blocks the gate independently of the project's declared
blocking list.

#### Scenario: Required unbound check blocks the gate

- **WHEN** a required check is unbound and the project declares
  `blocking: [FAIL, REVIEW_REQUIRED]`
- **THEN** the gate is blocked, the run status is a failure, the exit code
  is nonzero, and the result names the check, the missing `commands` key,
  and the exemption alternative

#### Scenario: Unbound check blocks even without review-required blocking

- **WHEN** a required check is unbound and the project's `blocking` list
  omits `REVIEW_REQUIRED`
- **THEN** the gate is still blocked, the run is still not recorded as
  passing, and the outcome does not depend on any project-controlled
  policy toggle

#### Scenario: Unbound check blocks with no blocking list at all

- **WHEN** a required check is unbound and the manifest declares no
  `blocking` list
- **THEN** the gate is blocked on the unbound check alone

#### Scenario: Unbound is distinct from a not-applicable verdict

- **WHEN** a bound command runs and itself reports not-applicable
- **THEN** that result is recorded as not-applicable with the command's
  own evidence, and is never merged with, or substituted by, an unbound
  result that no command produced

#### Scenario: Disabled check is not an unbound check

- **WHEN** a check is selected `false` and binds no command
- **THEN** it is deselected, produces no result, and is never recorded as
  unbound

#### Scenario: An unbound optional check does not block

- **WHEN** a check selected `"optional"` is unbound
- **THEN** the result carries warning severity, the check id appears in
  the unbound list, and the gate outcome is not changed by it

#### Scenario: An invented check name cannot escape the rule

- **WHEN** a manifest declares a check id the runtime does not recognise
  and binds no command for it
- **THEN** the same unbound rule applies, so the id is reported as an
  unbound required check rather than ignored

### Requirement: Validate required check binding before execution

The system SHALL resolve a `binding_policy` from the manifest with the
closed value set `report` and `reject`, SHALL default to `report` until
the fleet is migrated, SHALL refuse an unrecognised value, and SHALL
report an unbound required check without refusing the manifest when the
policy is `report`.

#### Scenario: Default policy reports rather than refuses

- **WHEN** a manifest omits `binding_policy` and declares a required
  unbound check
- **THEN** the manifest is accepted, the gate still executes, a gate run
  is still persisted, and the unbound check is reported and blocks

#### Scenario: Reject policy refuses before execution

- **WHEN** `binding_policy` is `reject` and a required check is unbound
- **THEN** no check executes and the refusal names the check id and the
  `commands` key to add

#### Scenario: Unknown policy value is refused

- **WHEN** `binding_policy` carries a value outside the accepted set
- **THEN** resolution fails with the offending value and the accepted set

#### Scenario: Dry run stays read only

- **WHEN** the user requests a dry run on a manifest with unbound required
  checks
- **THEN** the plan names every unbound required check with its
  remediation, and no child process runs and no state is mutated

### Requirement: Declare a check that does not apply to the project stack

The system SHALL accept a `not_applicable` declaration mapping a selected
concern id to a non-empty reason, SHALL record that reason as an
acknowledged exemption, and SHALL bound and redact it before it is
persisted or rendered; a project with no such gate states that explicitly
rather than leaving the absence implicit.

#### Scenario: Exemption with a reason is accepted

- **WHEN** a project declares `not_applicable` for a selected check with a
  non-empty reason and binds no command
- **THEN** the check is recorded as an acknowledged exemption with that
  reason, is not reported as unbound, and does not block the gate

#### Scenario: Exemption without a reason is refused

- **WHEN** a `not_applicable` entry has an empty or whitespace-only reason
- **THEN** resolution fails naming the offending key and requiring a
  reason

#### Scenario: Exemption on an unselected check is refused

- **WHEN** a `not_applicable` entry names a check that is not selected
- **THEN** resolution fails naming the key

#### Scenario: Exemption conflicting with a bound command is refused

- **WHEN** the same check both binds a `commands` entry and appears in
  `not_applicable`
- **THEN** resolution fails naming the conflict, so a project cannot claim
  a command and an exemption at once

#### Scenario: Exemption reason is bounded and redacted

- **WHEN** a `not_applicable` reason exceeds the supported length or
  contains a credential-shaped string
- **THEN** the stored and rendered reason is truncated with a marker and
  credential-redacted, and is never persisted raw

#### Scenario: An unbound check may be neither bound nor exempted

- **WHEN** a project declares a check it has neither implemented nor
  exempted, including a low-frequency kind such as an accessibility,
  format, or lint concern
- **THEN** the check is reported as an unbound required check, so an
  unimplemented and unexplained check is never silently ignored

### Requirement: Cap the exemptions one project may declare

The system SHALL limit how many checks a single project may exempt, SHALL
compute that limit from that project's own manifest as
`min(floor(0.25 × required_checks), 5)` where `required_checks` is the number
of required checks the manifest selects, SHALL count every declared
`not_applicable` entry against the limit, SHALL never accept an exemption for
`build`, `tests`, `security`, or `repository` — nor for a spelling of one of
those checks that the workspace uses, such as `repository_integrity` for the
repository-integrity check — SHALL report the headroom as exhausted when the
declared exemptions reach the limit, and SHALL fail the gate when the project
declares more exemptions than the limit or exempts one of those checks. A
non-exempable concern is identified by what it is, not by how it is spelled,
so an alias is protected exactly as the canonical id is. The limit is a
ceiling on self-authorised absence, not a purchase of coverage: a project
whose stack genuinely needs more must bind a command or be reviewed, and the
exemption reason is required text in every case, never a free pass.

#### Scenario: Exemptions below the limit are acknowledged

- **WHEN** a project declares fewer exemptions than its computed limit
- **THEN** each exemption is recorded as an acknowledged exemption with its
  reason, no exemption-budget result is produced, and the gate outcome is
  unchanged by the declarations

#### Scenario: Exemptions exactly at the limit exhaust the headroom

- **WHEN** a project declares exactly `min(floor(0.25 × required_checks), 5)`
  exemptions and none names a non-exempable check
- **THEN** the run records a warning-severity result whose diagnostic names
  the declared count, the limit, and the exhausted headroom, the run is not
  blocked, and every declaration remains an acknowledged exemption

#### Scenario: Exemptions over the limit fail the gate

- **WHEN** a project declares more exemptions than
  `min(floor(0.25 × required_checks), 5)`
- **THEN** the run status is a failure, the run is blocked, and the failure
  names the declared count, the limit, and the project's required-check count,
  so the project must bind a command or be reviewed instead of exempting its
  way to green

#### Scenario: A non-exempable check fails the gate regardless of the count

- **WHEN** a project declares `not_applicable` for `build`, `tests`,
  `security`, or `repository`
- **THEN** the run status is a failure and the run is blocked whatever the
  numeric limit is and however many other exemptions the project declares, and
  the exempted check is never recorded as an acknowledged exemption

#### Scenario: A workspace alias of a non-exempable check fails the same way

- **WHEN** a project declares `not_applicable` for a spelling of a
  non-exempable check that the workspace uses, such as `repository_integrity`
  for the repository-integrity check
- **THEN** that check is protected exactly as its canonical id is: the run
  status is a failure, the run is blocked, the diagnostic names the offending
  id as the project wrote it, and the exemption is never acknowledged, so
  spelling the concern differently cannot buy the hole the spine forbids

#### Scenario: A small manifest may exempt nothing

- **WHEN** a project selects fewer than four required checks, so that
  `floor(0.25 × required_checks)` is zero
- **THEN** its limit is zero, every declared exemption is over the limit, and
  the run fails rather than exempting a check from a manifest too small to
  carry any hole

#### Scenario: A large manifest buys no wider hole

- **WHEN** a project selects twenty or more required checks
- **THEN** its limit is five, so a larger manifest earns no additional
  exemptions, and the limit is derived from that project rather than shared
  with the workspace

#### Scenario: A project with no exemptions is unchanged

- **WHEN** a project declares no `not_applicable` entry
- **THEN** no exemption-budget result is produced and the run's outcome is
  what the same manifest produced before this rule existed

#### Scenario: The budget is auditable before anything executes

- **WHEN** a user requests a dry run on a manifest whose exemptions are at or
  over the limit, or that exempts a non-exempable check
- **THEN** the plan names the declared count, the computed limit, the
  required-check count, and the non-exempable ids, and no child process runs
  and no state is mutated

### Requirement: Record unbound coverage in the gate run

The system SHALL persist unbound coverage with the gate run and SHALL keep
unbound checks in a distinct collection from not-applicable checks, so
coverage debt is queryable from a stored run without re-executing the
gate.

#### Scenario: Unbound ids survive persistence

- **WHEN** a gate run contains unbound required checks
- **THEN** the stored run record carries the unbound collection and the
  per-check unbound results, with the diagnostic that no execution was
  attempted

#### Scenario: Unbound and not-applicable stay separate

- **WHEN** a single run contains both a proven not-applicable check and an
  unbound check
- **THEN** each appears only in its own collection and a consumer reading
  either one cannot mistake it for the other

#### Scenario: Every result surface shows the unbound state

- **WHEN** a run with unbound checks is rendered as human output, as JSON,
  exported as evidence, or read through a read-only tool
- **THEN** each surface reports the unbound checks with their remediation
  and does not drop them or count them as passing

### Requirement: State the measured workspace Gate inventory

The system SHALL document the workspace Gate inventory from the measured
manifest count rather than an estimate, and SHALL record how many of those
manifests currently bind no commands, so the documented position and the
real position do not diverge.

#### Scenario: Documented count matches the fleet

- **WHEN** the workspace Gate inventory is documented
- **THEN** the stated number of workspace Gate manifests equals the number
  present in the workspace, and the number that declare no command binding
  is stated alongside it

#### Scenario: A stale count is a defect

- **WHEN** the documented manifest count no longer matches the workspace
- **THEN** the documentation is corrected as part of this requirement
  rather than left to drift
