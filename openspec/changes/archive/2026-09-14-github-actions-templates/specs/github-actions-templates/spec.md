## ADDED Requirements
### Requirement: Reusable check workflow
`templates/github-actions/driftwatch-check.yml` MUST be callable via `workflow_call`, install a pinned driftwatch, run checkers, and always publish the AI report.

#### Scenario: Caller reuses the template
- **WHEN** a repo calls the workflow with `version: v0.6.0`
- **THEN** the installer runs with `--version v0.6.0`, `driftwatch check` executes, and `drift.md` uploads as an artifact even when checkers report drift

#### Scenario: Drift findings stay informational by default
- **WHEN** `fail_on_drift` is unset and checkers report alerts
- **THEN** the job still succeeds and the summary lists the top bugs

#### Scenario: Strict mode opted in
- **WHEN** `fail_on_drift: true` and checkers report alerts
- **THEN** the job fails after still uploading `drift.md`

### Requirement: No raw state leaves the runner
The template MUST NOT upload `.driftwatch/state.db` or full command logs.

#### Scenario: Artifact audit
- **WHEN** the shape test scans the template's upload steps
- **THEN** only `drift.md` (and the step summary) are published; no `state.db` or `.driftwatch/` path appears in any upload

### Requirement: Template documentation
`templates/github-actions/README.md` MUST document inputs, permissions, the exit-code contract, and the copy-vs-`uses:` choice.

#### Scenario: New adopter onboarding
- **WHEN** a user reads the template README
- **THEN** they learn the required `contents: read` permission, the `version`/`fail_on_drift` inputs, and that drift is informational unless opted into strict mode
