# gate-cli-and-memory-integration Specification

## Purpose
TBD - created by archiving change gate-cli-and-memory-integration. Update Purpose after archive.
## Requirements
### Requirement: Run Gate locally

The system MUST provide a `gate` command that resolves the project Gate plan,
executes applicable checks, persists the result, and returns a nonzero exit code
for blocking failures or unresolved blocking reviews.

#### Scenario: Local passing change

- **WHEN** all required checks pass before archive
- **THEN** `driftwatch gate` persists a passing snapshot and exits zero

#### Scenario: Blocking local failure

- **WHEN** a required gate fails
- **THEN** the command persists the failure, prints remediation, and exits nonzero

#### Scenario: Dry run

- **WHEN** `driftwatch gate --dry-run` is invoked
- **THEN** the resolved plan is shown without child execution or persistence

### Requirement: Persist Gate history

Gate snapshots MUST retain change/revision identity, manifest/rule digests, tool
versions, aggregate status, per-gate results, and evidence references.

#### Scenario: Repeated change

- **WHEN** the same change is gated at two revisions
- **THEN** both snapshots remain queryable and distinguishable

### Requirement: Preserve legacy checker behavior

Existing `driftwatch check` behavior MUST remain available for projects that have
not adopted the Gate manifest.

#### Scenario: Legacy project

- **WHEN** a project only has `driftwatch.toml` checkers
- **THEN** `driftwatch check` continues to run and persist checker snapshots

### Requirement: Local verification precedes remote CI

Documentation and templates MUST describe local Gate execution before archive or
completion, with CI repeating rather than replacing it.

#### Scenario: CI-only attempt

- **WHEN** a change has not passed its local Gate
- **THEN** it MUST NOT be represented as complete merely because CI is configured

