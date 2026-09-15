## ADDED Requirements

### Requirement: Evaluate evidence through a typed contract

An AI evaluator MUST receive selected rules and evidence and MUST return typed
status, confidence, rules checked, violations, evidence, missing evidence,
reason, and recommended actions.

#### Scenario: Evidence-backed pass

- **WHEN** the provider returns valid schema output with sufficient evidence
- **THEN** the evaluation may be `PASS` and records provider and rule metadata

#### Scenario: Missing evidence

- **WHEN** required evidence is absent
- **THEN** the evaluation is `REVIEW_REQUIRED` and names missing evidence

#### Scenario: Invalid provider output

- **WHEN** provider output is malformed or outside the allowed status set
- **THEN** the evaluation is `REVIEW_REQUIRED`, never `PASS`

### Requirement: Keep AI optional and safe

AI evaluation MUST be opt-in, redacted, bounded, and independently diagnosable.

#### Scenario: No provider configured

- **WHEN** an AI concern is enabled but no provider is configured
- **THEN** the result is `REVIEW_REQUIRED` or `NOT_APPLICABLE` according to
  explicit policy and explains the missing provider

#### Scenario: Provider failure

- **WHEN** the provider times out or rejects a request
- **THEN** the result records provider unavailability and no code change is made
