# generic-gate-contract Specification (delta: declare-new-change-with-gate-debt-code)

## MODIFIED Requirements

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

## ADDED Requirements

### Requirement: A declared code states why it is declared

Every code in `GOVERNANCE_QUALITY_DECLARED_CODES` SHALL be accompanied by a
statement of the failure its absence would cause, so that a later reader can
tell an intentional exclusion from an oversight.

#### Scenario: A code is added

- **WHEN** a code is added to the declared set
- **THEN** the rationale for declaring it is recorded beside the constant
