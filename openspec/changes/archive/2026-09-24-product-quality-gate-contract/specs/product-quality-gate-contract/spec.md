## ADDED Requirements

### Requirement: Recognize product-quality concern IDs

The Gate contract MUST recognize `product-code-boundary` and
`placeholder-threshold` as stable concern IDs without embedding a language
scanner or provider-specific implementation.

#### Scenario: Selected project concern

- **WHEN** a project manifest selects either concern and binds a command
- **THEN** the resolved Gate plan includes the concern and its command executes
  through the generic project-runtime adapter

#### Scenario: Existing manifest

- **WHEN** a manifest does not select either concern
- **THEN** existing Gate and checker-only behavior remains unchanged

### Requirement: Normalize checker command outcomes honestly

The Gate MUST map product-quality command outcomes to PASS, FAIL,
REVIEW_REQUIRED, or NOT_APPLICABLE using exit-code authority and MUST NOT
convert missing required coverage into PASS.

#### Scenario: Passing report

- **WHEN** the command exits 0 and emits a valid PASS report
- **THEN** the Gate records PASS with bounded evidence

#### Scenario: Threshold or boundary failure

- **WHEN** the command exits 1 and emits a valid FAIL report
- **THEN** the Gate records FAIL and blocks a required concern

#### Scenario: Configuration review

- **WHEN** the command exits 2 or emits a valid REVIEW_REQUIRED report
- **THEN** the Gate records REVIEW_REQUIRED and applies the manifest blocking
  policy

#### Scenario: Missing required command

- **WHEN** a selected required concern has no command binding
- **THEN** the Gate records REVIEW_REQUIRED with missing-command evidence

#### Scenario: Contradictory output

- **WHEN** a non-zero command emits a report claiming PASS
- **THEN** the Gate refuses PASS and records REVIEW_REQUIRED or FAIL with the
  command exit as authoritative evidence

### Requirement: Preserve generic Gate boundaries

The product-quality contract MUST remain language-agnostic, local-first, and
compatible with existing manifest precedence, evidence bounds, redaction,
persistence, and checker-only commands.

#### Scenario: Optional unbound concern

- **WHEN** an optional product-quality concern has no command
- **THEN** the Gate records visible NOT_APPLICABLE status without blocking

#### Scenario: Dry run

- **WHEN** `driftwatch gate --dry-run` resolves a product-quality manifest
- **THEN** it renders the selected and unbound concerns without executing a
  command or persisting a run

#### Scenario: Foreign runtime

- **WHEN** `.ai-gate/gate.yaml` names a runtime other than Driftwatchdog
- **THEN** no product-quality command runs and no Gate run is persisted
