# deployable-compose-ci-contract Specification

## Purpose
TBD - created by archiving change deployable-compose-ci-gate. Update Purpose after archive.
## Requirements
### Requirement: Provide a deployable profile

The Gate MUST provide a built-in `deployable` profile selecting required
`compose-contract` and `ci-contract` concerns.

#### Scenario: Resolve deployable profile

- **WHEN** a project manifest selects `profile = "deployable"`
- **THEN** the resolved plan contains both concerns as required checks in
  deterministic order

#### Scenario: Preserve non-deployable profiles

- **WHEN** a project selects `backend`, `frontend`, `full`, `minimal`,
  `product`, `rust-product`, or `release`
- **THEN** the plan is unchanged by the deployable profile addition

### Requirement: Fail closed when mandatory bindings are absent

The Gate MUST represent a missing command binding for either deployable
concern as explicit missing evidence and MUST block the required check under
the default review policy.

#### Scenario: Missing Compose binding

- **WHEN** a deployable manifest omits the `compose-contract` command
- **THEN** the Gate records `compose-contract:command` as missing evidence and
  blocks with `REVIEW_REQUIRED`

#### Scenario: Missing CI binding

- **WHEN** a deployable manifest omits the `ci-contract` command
- **THEN** the Gate records `ci-contract:command` as missing evidence and
  blocks with `REVIEW_REQUIRED`

### Requirement: Execute project-owned contract commands

The Gate MUST execute the declared Compose and CI contract commands through
the existing bounded project-runtime adapter and MUST preserve their exit
status and diagnostics in the normal Gate result model.

#### Scenario: Both contracts pass

- **WHEN** both declared commands exit zero
- **THEN** both concerns are `PASS` and the Gate is unblocked

#### Scenario: A contract fails

- **WHEN** either declared command exits non-zero
- **THEN** its required concern is `FAIL` and the Gate is blocked

### Requirement: Provide bootstrap and CI templates

The repository MUST provide repository-relative starter templates for a local
CI command, Compose validation, a deployable Gate manifest, and a CI workflow
that invokes the local contract and Gate.

#### Scenario: Bootstrap copies templates

- **WHEN** a bootstrap integration copies the deployable templates into a
  project
- **THEN** the generated commands use relative paths and do not contain a
  machine-specific workspace prefix

#### Scenario: CI repeats local verification

- **WHEN** the template workflow runs on a push or pull request
- **THEN** it invokes the repository's local CI command and
  `driftwatch gate`, failing the job on either failure
