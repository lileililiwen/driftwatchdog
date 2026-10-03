# gate-project-configuration Specification

## Purpose

Business projects need to declare profiles, checks, project-runtime commands, blocking policy, and local rule overrides without embedding tool implementations or language-specific logic in Driftwatchdog.
## Requirements
### Requirement: Validate a project Gate manifest

The system MUST load and validate a project-local Gate manifest with version,
profile, checks, blocking policy, project commands, and optional context
providers.

#### Scenario: Valid manifest

- **WHEN** a project declares a supported profile and valid checks
- **THEN** the system produces a resolved plan without executing tools

#### Scenario: Unknown field

- **WHEN** the manifest contains an unknown field
- **THEN** loading fails with the field name and a remediation hint

#### Scenario: Invalid project command

- **WHEN** an enabled required command is empty or malformed
- **THEN** plan resolution fails before any check executes

### Requirement: Resolve conditional concerns

The system MUST resolve explicit checks and optional changed-surface triggers
without treating absent UI or privacy concerns as failures.

#### Scenario: Backend-only project

- **WHEN** a backend profile does not enable responsive checks
- **THEN** no browser check is scheduled and the result is explicit

#### Scenario: UI change trigger

- **WHEN** a configured trigger detects a frontend change
- **THEN** the matching concern is included in the resolved plan

### Requirement: Expose the resolved plan

The system MUST expose a dry-run plan containing selected checks, execution
mode, project commands, rule-pack version, and context providers.

#### Scenario: Dry-run

- **WHEN** the user requests a Gate dry run
- **THEN** the plan is printed and no child process or state mutation occurs

### Requirement: Resolve a business Gate manifest at .ai-gate/gate.yaml

The system MUST resolve a project Gate manifest from `gate.toml`,
`.driftwatch/gate.toml`, or `.ai-gate/gate.yaml` (in that precedence),
convert the YAML form into the same resolved plan as the TOML form, and
fail resolution before any check executes on unknown fields, an
unsupported version, an empty enabled command, or a malformed project
command.

#### Scenario: YAML manifest resolves

- **WHEN** a project declares a valid `.ai-gate/gate.yaml` naming runtime
  `driftwatchdog` with a profile, selected checks, and a blocking list
- **THEN** the system produces a resolved plan carrying that profile and
  rule-pack identity without executing any tool

#### Scenario: Native manifest takes precedence

- **WHEN** a project has both `gate.toml` and `.ai-gate/gate.yaml`
- **THEN** the `gate.toml` plan is resolved and the YAML file is ignored

#### Scenario: Foreign runtime policy is not executed

- **WHEN** a `.ai-gate/gate.yaml` names a runtime other than
  `driftwatchdog`
- **THEN** the system reports the policy is for another runtime, executes
  nothing, persists no gate run, and exits zero

#### Scenario: Unknown YAML field

- **WHEN** the YAML manifest contains a field outside the declared schema
- **THEN** resolution fails with the field name and a remediation hint

### Requirement: Map YAML policy onto the existing Gate plan

The system MUST map `checks` selections, an optional `commands` binding,
the `blocking` status list, and optional `project_commands` and
`contexts` onto the existing plan: a selected concern with a declared
command runs through the project-runtime adapter, a selected concern
without a command records explicit `NOT_APPLICABLE`, and the `blocking`
list sets the review-required policy without changing `FAIL` blocking.

#### Scenario: Selected concern with command

- **WHEN** a selected check has a matching non-empty `commands` entry
- **THEN** the concern is scheduled with that command for execution

#### Scenario: Selected concern without command

- **WHEN** a selected check has no command binding
- **THEN** the concern is scheduled and records explicit `NOT_APPLICABLE`
  rather than a silent pass

#### Scenario: Review-required blocking follows the list

- **WHEN** `blocking` includes `REVIEW_REQUIRED`
- **THEN** unresolved reviews block the gate; when it omits
  `REVIEW_REQUIRED`, reviews stay explicit but do not block

#### Scenario: Unknown blocking status

- **WHEN** `blocking` contains a status name the system does not define
- **THEN** resolution fails with the offending value and the accepted set

### Requirement: Accept project-defined domain profiles

The system MUST accept a `profile` that is a built-in (`backend`,
`frontend`, `full`, `minimal`) or a project-defined name whose selected
concerns are fully declared by the manifest, and MUST record the profile
string and rule-pack version as plan identity in the dry-run render and
the persisted run.

#### Scenario: Domain profile resolves

- **WHEN** a manifest declares profile `browser-extension` with its own
  selected checks
- **THEN** the plan resolves with that profile name and rule-pack version
  and schedules exactly the declared concerns

#### Scenario: Unknown profile without declaration

- **WHEN** a `gate.toml` names a profile that is neither built-in nor
  defined in a `[profiles.<name>]` table
- **THEN** resolution fails with the unsupported-profile error as before

