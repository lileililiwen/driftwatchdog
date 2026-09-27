# gate-evidence-export Specification

## Purpose

Export a completed Gate run as an evidence record in Workspace Governance's
`release_evidence` vocabulary, so a `verified` state means a check actually ran
against the current revision rather than a hand-written declaration.

## ADDED Requirements

### Requirement: Export a completed run in the governance vocabulary

A completed Gate run MUST be exportable as a versioned document identifying the
project, revision and toolchain, with one entry per Workspace Governance
`release_evidence` field carrying a state and an evidence reference. The field
names MUST be exactly the governance set; an unknown field MUST be a
construction error.

#### Scenario: Complete run

- **WHEN** a run completes for a project with all required checks scheduled
- **THEN** the export carries an entry for every governance field

#### Scenario: Unknown field requested

- **WHEN** an entry would use a field name outside the governance set
- **THEN** construction fails and nothing is emitted

### Requirement: Only executed passing checks are verified

A field MUST be `verified` only when a scheduled check actually executed and
passed and the run's revision matches; a check that failed, was not scheduled,
or could not execute MUST NOT contribute `verified`.

#### Scenario: Passing check

- **WHEN** a scheduled check runs and passes on the current revision
- **THEN** the field it supplies is `verified`

#### Scenario: Check could not execute

- **WHEN** a check cannot run because tooling or the environment is unavailable
- **THEN** the field is `blocked`, never `verified`

#### Scenario: Check not scheduled

- **WHEN** a governance field has no scheduled check
- **THEN** the field is `unverified`

### Requirement: Stale runs never verify

When the completed run's revision differs from the project's current revision,
every exported field MUST be `unverified`, and the export MUST record a
diagnostic naming both revisions.

#### Scenario: Stale run

- **WHEN** the run revision differs from the project revision
- **THEN** the export contains no `verified` state

### Requirement: Export refuses absent evidence safely

The export MUST refuse when no completed run exists for the project, and MUST
NOT invent fields or defaults.

#### Scenario: No run

- **WHEN** a project has no completed Gate run
- **THEN** the export exits non-zero naming the project and emits no document

### Requirement: Export stays an executor

The export MUST NOT publish, sign, scan, or deploy, and MUST NOT mutate the run,
the project, or an external registry.

#### Scenario: Export is read-only

- **WHEN** an export is produced
- **THEN** the run, project files and any registry are unchanged
