# ci-test-gates Specification

## Purpose
Continuous-integration quality gates: formatting, clippy with warnings as
errors across all targets and features, the workspace test suite, MSRV
verification, advisory/license auditing via cargo-deny, coverage reporting,
shell linting, npm auditing, and the end-to-end release smoke test.
## Requirements
### Requirement: CI test gates
CI SHALL run fmt check, clippy with -D warnings, and the full test suite on every push/PR.
#### Scenario: Push occurs
- **WHEN** a push is made to the repo
- **THEN** clippy and tests run and fail on regressions
#### Scenario: PR opened
- **WHEN** a pull request is opened
- **THEN** the same gates block merge on failure

