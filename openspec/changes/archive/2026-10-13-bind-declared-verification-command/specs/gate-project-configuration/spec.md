# gate-project-configuration Specification (delta: bind-declared-verification-command)

## ADDED Requirements

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
