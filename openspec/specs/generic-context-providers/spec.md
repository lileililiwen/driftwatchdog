# generic-context-providers Specification

## Purpose
TBD - created by archiving change generic-context-providers. Update Purpose after archive.
## Requirements
### Requirement: Load generic context

The system MUST load selected context providers into bounded, hashed, generic
documents without requiring a specific specification framework.

#### Scenario: Git-only project

- **WHEN** a project enables only the Git provider
- **THEN** Gate planning works without OpenSpec files or dependencies

#### Scenario: OpenSpec project

- **WHEN** a project enables the OpenSpec provider
- **THEN** selected proposal, design, task, and scenario context is available as
  generic documents

#### Scenario: Missing optional provider

- **WHEN** an optional provider has no matching files
- **THEN** the plan records `NOT_APPLICABLE` or unavailable context without
  claiming requirements were satisfied

### Requirement: Keep providers read-only and bounded

Context providers MUST not mutate project files and MUST enforce path, size, and
redaction limits.

#### Scenario: Context path escape

- **WHEN** a configured context path escapes the project root
- **THEN** collection fails safely and does not read the target

#### Scenario: Oversized document

- **WHEN** a document exceeds the configured limit
- **THEN** the provider records truncation/unavailability and never loads
  unbounded content

