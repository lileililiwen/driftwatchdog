# generic-gate-contract Specification (delta: bind-declared-verification-command)

## ADDED Requirements

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
