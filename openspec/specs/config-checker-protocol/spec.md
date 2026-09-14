# config-checker-protocol Specification

## Purpose
TBD - created by archiving change config-checker-protocol. Update Purpose after archive.
## Requirements
### Requirement: Validated checker config
Invalid checker configuration MUST fail fast with an actionable error naming the checker and the fix.

#### Scenario: Duplicate checker name
- **WHEN** `driftwatch.toml` declares two checkers named `architecture`
- **THEN** `driftwatch check` and `doctor` fail with `duplicate checker name "architecture"` before running anything

#### Scenario: Empty command
- **WHEN** a checker has `command = ""`
- **THEN** config load errors with `checker "x" has an empty command`

#### Scenario: Zero timeout
- **WHEN** a checker sets `timeout_ms = 0`
- **THEN** config load errors suggesting a positive value or omitting the key

#### Scenario: Unknown field typo
- **WHEN** config contains `commad = "..."`
- **THEN** the error says `unknown field "commad", did you mean "command"?`

#### Scenario: Working-dir escape
- **WHEN** `working_dir = "../../etc"`
- **THEN** config load errors with `working_dir escapes the project root`; `working_dir = "tools/spec"` resolves inside the root regardless of process cwd

### Requirement: Forward-compatible alerts protocol
The protocol MUST ignore unknown top-level fields and MUST require `alerts`; empty-object documents MUST NOT count as success.

#### Scenario: New checker version adds a field
- **WHEN** a checker emits `{"alerts":[…], "newField": 1}`
- **THEN** parsing succeeds and the alerts are recorded

#### Scenario: Missing alerts key
- **WHEN** a checker emits `{}`
- **THEN** `driftwatch check` records a protocol-error outcome (not Empty/success) with `missing "alerts"` diagnostic

#### Scenario: Dry-run smoke test
- **WHEN** the user runs `driftwatch check --dry-run`
- **THEN** each checker runs, parsed counts print, and nothing is persisted

