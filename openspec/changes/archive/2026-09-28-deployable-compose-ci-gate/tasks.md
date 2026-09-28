# Tasks: Enforce deployable Compose and CI contracts

## 1. BFS — Baseline and impact coverage

- [x] 1.1 Map the new profile and both concerns to manifest resolution,
  execution, aggregation, templates, and packaging tests.
- [x] 1.2 Confirm existing profiles and non-deployable repositories remain
  unchanged.

## 2. DFS — Requirement-by-requirement implementation

- [x] 2.1 Add stable concern IDs and the built-in `deployable` profile.
- [x] 2.2 Add unit/integration tests for required bindings and command outcomes.
- [x] 2.3 Add repository-relative bootstrap and CI templates.
- [x] 2.4 Add packaging assertions for template contents and portability.

## 3. BFS — Cross-surface regression and completeness

- [x] 3.1 Verify dry-run, human output, JSON output, persistence, and existing
  profile behavior remain compatible.
- [x] 3.2 Verify missing, malformed, unavailable, and failing command cases are
  visible and blocking where required.
- [x] 3.3 Review the diff for machine-specific paths, provider coupling, and
  unowned scope.

## 4. Verification

- [x] 4.1 Run formatting, unit/integration tests, clippy, packaging tests, and
  strict OpenSpec validation.
- [x] 4.2 Update this task list and handoff evidence only from fresh command
  output.
