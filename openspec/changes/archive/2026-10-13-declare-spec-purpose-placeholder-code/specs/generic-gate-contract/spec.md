# generic-gate-contract Specification (delta: declare-spec-purpose-placeholder-code)

## ADDED Requirements

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
