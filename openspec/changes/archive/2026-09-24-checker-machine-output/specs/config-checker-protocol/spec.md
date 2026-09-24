# config-checker-protocol Specification

## ADDED Requirements

### Requirement: Machine-readable check report

`driftwatch check` MUST support `--format json`, emitting one versioned
checker-report document on stdout — contract id, tool, version, ordered
per-checker rows with statuses and alerts in the existing alert
vocabulary, and summary counts — while leaving checker selection,
isolation, persistence and exit-status semantics identical to human mode.

#### Scenario: Mixed outcomes

- **WHEN** configured checkers include one clean, one alerting and one
  broken checker
- **THEN** the document lists all three in declaration order with their
  statuses and the alerting one's alerts in the standard wire shape

#### Scenario: Dry run with JSON

- **WHEN** `driftwatch check --dry-run --format json` runs
- **THEN** the same document prints and no rows are persisted

#### Scenario: Empty alerts is valid

- **WHEN** every checker passes
- **THEN** the document contains `"alerts": []` for each checker (never a
  missing `alerts` key) and exits like human mode

#### Scenario: Isolated failure stays visible

- **WHEN** one checker times out or emits malformed output
- **THEN** other checkers still appear with their own rows and the failed
  one carries its protocol-error/timeout status, not a crash
