## ADDED Requirements

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
