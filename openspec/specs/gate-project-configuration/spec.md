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

### Requirement: Every gate check may declare its own execution timeout

A Gate manifest SHALL accept an optional per-check `timeout_ms`, in
milliseconds, on each check declaration. The value SHALL govern the
deadline applied when that check's bound command is executed, and SHALL
be the deadline the resolved plan records for that check.

A check that declares no `timeout_ms` SHALL resolve to the existing
default of 60000 ms. The default SHALL NOT change as a consequence of
this requirement: an existing manifest that declares no timeout SHALL
keep its current execution behaviour, and its canonical form and
manifest digest SHALL be unchanged by the existence of the field.

A declared timeout SHALL be validated before any check executes. A value
of `0` SHALL be refused as not a usable deadline, and a value above the
documented cap `MAX_CHECK_TIMEOUT_MS` of 3600000 SHALL be refused. Both
refusals SHALL name the offending check and state an actionable
remediation, and the cap refusal SHALL name the cap.

#### Scenario: A check declares no timeout

- **WHEN** a manifest selects a check and declares no `timeout_ms`
- **THEN** the resolved plan records 60000 ms for that check, execution
  applies that deadline, and the check's canonical manifest form and
  digest are byte-identical to a manifest that predates the field

#### Scenario: A check declares a timeout

- **WHEN** a manifest declares a positive `timeout_ms` within the cap for
  a check
- **THEN** the resolved plan records that value for that check, and
  execution of that check's command is bounded by it instead of the
  default

#### Scenario: A check declares a zero timeout

- **WHEN** a manifest declares `timeout_ms = 0` for a check
- **THEN** plan resolution fails before any check executes, with a message
  naming the check and stating that a positive value or no key is required

#### Scenario: A check declares a timeout above the cap

- **WHEN** a manifest declares a `timeout_ms` greater than 3600000
- **THEN** plan resolution fails before any check executes, with a message
  naming the check and the cap

#### Scenario: The resolved timeout is visible

- **WHEN** a plan is rendered for dry-run
- **THEN** the rendering states the effective timeout of every check it
  scheduled, so the deadline is readable before anything executes

### Requirement: Both manifest forms declare a check timeout identically

The business `.ai-gate/gate.yaml` form and the native `gate.toml` form
SHALL accept a per-check timeout and SHALL convert it to the same check
field, so that a manifest written in either form resolves to the same
effective timeout for the same check. Validation of the declared value
SHALL run once, in the shared manifest validation step, for both forms;
the YAML form SHALL NOT carry a second copy of that rule.

A per-check timeout naming a check the manifest does not select SHALL be
refused, because a deadline bound to no scheduled check is a value that
can never take effect.

#### Scenario: The two forms agree

- **WHEN** the same check, command and timeout are declared in a
  `.ai-gate/gate.yaml` manifest and in the equivalent native `gate.toml`
- **THEN** both resolve to the same check timeout, and the same validation
  outcome

#### Scenario: A timeout names an unselected check

- **WHEN** a manifest declares a per-check timeout for a check it does not
  select
- **THEN** parsing fails with a message naming the check, because a
  timeout for a check that never runs is silently inert

#### Scenario: An invalid timeout in the business YAML form

- **WHEN** the business YAML form declares a per-check timeout of `0` or
  one above the cap
- **THEN** the same refusal the native form produces is reported, from the
  shared validation step

### Requirement: The declared verification command is a first-class, non-exemptable concern

The Gate SHALL recognise `declared-verification` as a stable concern id whose
binding is the repository's own declared verification command, and that id SHALL
be non-exemptable. It SHALL be pure data: it SHALL belong to no closed concern
vocabulary, so the existing project-runtime text adapter serves it and no
envelope adapter can capture it. A repository SHALL NOT be able to exclude the
ruling that a declared command must run by writing a `not_applicable` reason,
because an exempable binding is a binding that ran nothing.

#### Scenario: A repository selects and binds the concern

- **WHEN** a manifest selects `declared-verification` in `checks` and binds a
  command for it
- **THEN** the resolved plan includes the concern as required, bound to that
  command, and the Gate executes it through the project-runtime text adapter

#### Scenario: A repository tries to exempt the concern

- **WHEN** a manifest declares `not_applicable` for `declared-verification`
- **THEN** resolution records the exemption as refused and the concern fails with
  the `non-exempable-concern` rule, rather than reporting `NOT_APPLICABLE`

#### Scenario: A repository does not select the concern

- **WHEN** a manifest neither selects nor binds `declared-verification`
- **THEN** the resolved plan is unchanged by this concern's existence, because
  the id is inert data until a repository selects it

#### Scenario: A profile must not invent the binding

- **WHEN** a repository selects a built-in profile that includes no
  `declared-verification` entry
- **THEN** the profile contributes no command for it, because profile defaults
  select concern ids only and never invent a command

### Requirement: A generated binding is written from the declaring repository

Where a binding for `declared-verification` is generated rather than
hand-written, it SHALL be produced from that same repository's own declared
`verification.command`, SHALL be written to both `checks` and `commands`
because a binding with no selection is a parse error, and SHALL be refused
rather than invented when the declaration is absent or unparseable. Generating
the same binding twice SHALL be a no-op, and a differing hand-written binding
SHALL NOT be overwritten.

#### Scenario: The declaration is present

- **WHEN** a repository's `.project.json` declares a non-empty
  `verification.command`
- **THEN** the generated manifest binds exactly that string verbatim, including
  its shell operators, under both `checks` and `commands`

#### Scenario: The declaration is absent or unparseable

- **WHEN** a repository's `.project.json` is missing, unparseable, or declares no
  non-empty string `verification.command`
- **THEN** the generator refuses for that repository, names the condition, and
  writes no command, because inventing one would assert a verification the
  repository never declared

#### Scenario: The generator runs twice

- **WHEN** the generator is run a second time over an already-bound repository
- **THEN** the manifest is byte-for-byte unchanged, because the declared command
  is already the bound command

#### Scenario: A differing binding already exists

- **WHEN** `commands.declared-verification` already holds a value that is not the
  declared command
- **THEN** the generator reports the conflict and leaves the file untouched,
  because a hand-written command is the repository's decision

