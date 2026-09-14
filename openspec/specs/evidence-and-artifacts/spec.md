# evidence-and-artifacts Specification

## Purpose
TBD - created by archiving change evidence-and-artifacts. Update Purpose after archive.
## Requirements
### Requirement: Record traceable evidence

The system MUST record bounded evidence metadata with producer, kind, size,
digest, and a path confined to the project state directory.

#### Scenario: Command evidence

- **WHEN** a gate captures command output
- **THEN** the result references a bounded artifact with producer and digest

#### Scenario: Path escape

- **WHEN** an adapter supplies an artifact path outside the allowed state root
- **THEN** persistence rejects it and records a safe diagnostic

### Requirement: Redact sensitive evidence

The system MUST redact configured sensitive values and recognized secret
patterns before persisting previews or result JSON.

#### Scenario: Secret in stderr

- **WHEN** a child process prints a configured token
- **THEN** persisted evidence does not contain the token

#### Scenario: Redaction failure

- **WHEN** evidence cannot be safely redacted
- **THEN** the evidence is marked unavailable and the related result is not
  allowed to claim evidence-backed `PASS`

### Requirement: Retain summaries after artifact cleanup

Artifact cleanup MUST preserve gate result identity, status, producer, and
digest even when bulky content is removed.

#### Scenario: Retention cleanup

- **WHEN** an artifact exceeds retention policy
- **THEN** its reference remains auditable with an unavailable-content marker

