## ADDED Requirements

### Requirement: Resolve verified tool versions

The system MUST resolve each Gate tool from a pinned manifest and MUST verify
its digest or signature before execution.

#### Scenario: Cached verified tool

- **WHEN** the requested version and digest are present in the cache
- **THEN** the cached executable is used without network access

#### Scenario: Missing tool during bootstrap

- **WHEN** bootstrap is explicitly requested and the tool is absent
- **THEN** the system downloads, verifies, atomically caches, and reports the
  selected version

#### Scenario: Verification mismatch

- **WHEN** downloaded content fails verification
- **THEN** the content is deleted, execution does not start, and bootstrap fails

### Requirement: Support execution modes

The system MUST support managed, container, native, and project-runtime modes
with explicit policy and diagnostics.

#### Scenario: Project runtime

- **WHEN** a C#, Rust, or Python project declares its build command
- **THEN** Driftwatchdog executes the declared argv without installing that SDK

#### Scenario: Container unavailable

- **WHEN** a required container check has no available container runtime
- **THEN** the result is `REVIEW_REQUIRED` or `FAIL` according to policy and
  explains remediation

### Requirement: Diagnose and bootstrap prerequisites

`doctor` MUST report project runtimes, Gate tools, container capability, cache,
and AI-provider configuration without claiming readiness from configuration alone.

#### Scenario: New machine

- **WHEN** a project is cloned on a machine without cached Gate tools
- **THEN** `doctor` reports missing tools and `bootstrap` provides the exact
  next action or fails with the blocked prerequisite
