## MODIFIED Requirements

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
silently convert `REVIEW_REQUIRED` into `PASS`. A required result with status
`UNBOUND` MUST block the aggregate and MUST produce aggregate status `FAIL`
independently of the configured blocking policy, because a project must not be
able to reduce an attempted-nothing required check to a pass by editing its
declared blocking list. The aggregate MUST keep unbound checks in a collection
distinct from proven or acknowledged not-applicable checks, so a single run
distinguishes an absence that was evaluated from one that was never attempted.
A failing exemption-budget result MUST block the aggregate on the same terms as
any other required failure, and a warning-severity budget result MUST be
recorded without blocking.

#### Scenario: Blocking failure

- **WHEN** any required result is `FAIL`
- **THEN** the aggregate status is `FAIL`

#### Scenario: Blocking review

- **WHEN** `REVIEW_REQUIRED` is configured as blocking
- **THEN** an unresolved review blocks the aggregate result

#### Scenario: All optional checks not applicable

- **WHEN** optional checks are `NOT_APPLICABLE` and required checks pass
- **THEN** the aggregate status is `PASS` with explicit not-applicable records

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
