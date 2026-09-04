## ADDED Requirements

### Requirement: Configure external checkers
The configuration MUST support repeatable `[[checkers]]` entries with at least `name` and `command`, and optional project working directory/environment settings.

#### Scenario: Multiple configured checkers
- **WHEN** a project configures architecture and OpenAPI checkers
- **THEN** `driftwatch check` discovers both in declaration order and executes each independently

### Requirement: Consume checker JSON protocol
A checker adapter MUST accept a JSON object containing an `alerts` array. Each alert MUST require severity, message, source, and symbol at the normalized boundary; optional fields and unknown JSON fields MUST not break parsing.

#### Scenario: Valid checker output
- **WHEN** a checker emits a valid alerts document
- **THEN** Driftwatch converts every alert to a `DriftAlert` with checker name, Git commit, and creation time

### Requirement: Persist check snapshots and alerts
`driftwatch check` MUST persist one snapshot per configured checker and its normalized alerts, allowing repeated snapshots to be distinguished and queried.

#### Scenario: Checker reports no alerts
- **WHEN** a checker exits successfully with an empty alerts array
- **THEN** a successful empty snapshot is persisted and the command reports zero alerts

### Requirement: Isolate checker failures
One checker failing to start, exiting nonzero, timing out, or emitting malformed JSON MUST produce a warning/failed snapshot and MUST NOT prevent remaining configured checkers from running.

#### Scenario: First checker fails
- **WHEN** the first configured checker fails and the second succeeds
- **THEN** both outcomes are persisted/reported and `driftwatch check` completes with the second checker's alerts available
