## ADDED Requirements

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
