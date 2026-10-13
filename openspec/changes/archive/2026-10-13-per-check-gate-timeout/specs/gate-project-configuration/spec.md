# gate-project-configuration Specification (delta: per-check-gate-timeout)

## ADDED Requirements

### Requirement: Every gate check may declare its own execution timeout

A Gate manifest SHALL accept an optional per-check `timeout_ms`, in
milliseconds, on each check declaration. The value SHALL govern the
deadline applied when that check's bound command is executed, and SHALL
be the deadline the resolved plan records for that check.

A check that declares no `timeout_ms` SHALL resolve to the existing
default of 60000 ms. The default SHALL NOT change as a consequence of
this requirement: an existing manifest that declares no timeout SHALL
keep its current execution behaviour, and its canonical form and
manifest digest SHALL be unchanged by the existence of the field.

A declared timeout SHALL be validated before any check executes. A value
of `0` SHALL be refused as not a usable deadline, and a value above the
documented cap `MAX_CHECK_TIMEOUT_MS` of 3600000 SHALL be refused. Both
refusals SHALL name the offending check and state an actionable
remediation, and the cap refusal SHALL name the cap.

#### Scenario: A check declares no timeout

- **WHEN** a manifest selects a check and declares no `timeout_ms`
- **THEN** the resolved plan records 60000 ms for that check, execution
  applies that deadline, and the check's canonical manifest form and
  digest are byte-identical to a manifest that predates the field

#### Scenario: A check declares a timeout

- **WHEN** a manifest declares a positive `timeout_ms` within the cap for
  a check
- **THEN** the resolved plan records that value for that check, and
  execution of that check's command is bounded by it instead of the
  default

#### Scenario: A check declares a zero timeout

- **WHEN** a manifest declares `timeout_ms = 0` for a check
- **THEN** plan resolution fails before any check executes, with a message
  naming the check and stating that a positive value or no key is required

#### Scenario: A check declares a timeout above the cap

- **WHEN** a manifest declares a `timeout_ms` greater than 3600000
- **THEN** plan resolution fails before any check executes, with a message
  naming the check and the cap

#### Scenario: The resolved timeout is visible

- **WHEN** a plan is rendered for dry-run
- **THEN** the rendering states the effective timeout of every check it
  scheduled, so the deadline is readable before anything executes

### Requirement: Both manifest forms declare a check timeout identically

The business `.ai-gate/gate.yaml` form and the native `gate.toml` form
SHALL accept a per-check timeout and SHALL convert it to the same check
field, so that a manifest written in either form resolves to the same
effective timeout for the same check. Validation of the declared value
SHALL run once, in the shared manifest validation step, for both forms;
the YAML form SHALL NOT carry a second copy of that rule.

A per-check timeout naming a check the manifest does not select SHALL be
refused, because a deadline bound to no scheduled check is a value that
can never take effect.

#### Scenario: The two forms agree

- **WHEN** the same check, command and timeout are declared in a
  `.ai-gate/gate.yaml` manifest and in the equivalent native `gate.toml`
- **THEN** both resolve to the same check timeout, and the same validation
  outcome

#### Scenario: A timeout names an unselected check

- **WHEN** a manifest declares a per-check timeout for a check it does not
  select
- **THEN** parsing fails with a message naming the check, because a
  timeout for a check that never runs is silently inert

#### Scenario: An invalid timeout in the business YAML form

- **WHEN** the business YAML form declares a per-check timeout of `0` or
  one above the cap
- **THEN** the same refusal the native form produces is reported, from the
  shared validation step
