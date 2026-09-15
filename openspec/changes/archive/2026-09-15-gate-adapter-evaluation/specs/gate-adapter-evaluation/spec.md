## ADDED Requirements

### Requirement: Isolate and normalize adapters

Each adapter MUST declare its tool identity and execution needs and MUST return a
normalized result without aborting unrelated adapters.

#### Scenario: Adapter success

- **WHEN** a tool emits supported JSON or SARIF with no blocking finding
- **THEN** the adapter returns normalized findings and evidence with `PASS`

#### Scenario: Malformed output

- **WHEN** a tool exits successfully but emits invalid output
- **THEN** the adapter returns a protocol failure and other adapters continue

#### Scenario: Timeout

- **WHEN** an adapter exceeds its configured timeout
- **THEN** its result records timeout evidence and no other adapter is skipped

### Requirement: Normalize tool findings

The system MUST normalize severity, rule id, message, source location, and
remediation while preserving producer and raw-artifact references.

#### Scenario: SARIF finding

- **WHEN** a tool emits a SARIF result with a source location
- **THEN** the normalized finding preserves rule, severity, location, and artifact

### Requirement: Support deterministic evaluation

Deterministic evaluators MUST evaluate normalized evidence against selected rules
without invoking an AI provider.

#### Scenario: No evidence-backed pass

- **WHEN** a required deterministic rule lacks required evidence
- **THEN** evaluation returns `REVIEW_REQUIRED`, not `PASS`
