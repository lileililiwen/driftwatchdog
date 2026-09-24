# capability-conformance-gate Specification

## Purpose
Generic Gate contract extension that adds the `capability-conformance`
concern id, declaring the wire shape, exit-code authority, and
required-evidence rules for project-owned capability reports. A `PASS`
claim MUST include a non-empty `capabilities.verified` list whose
every id appears in the resolved plan; an empty list, an out-of-scope
id, or a missing/malformed envelope all downgrade the result to
`REVIEW_REQUIRED`. The contract stays local-first, language-agnostic,
and free of provider SDKs, capability scanners, or embedded LLMs; the
project owns every command binding and every envelope producer, so
Driftwatchdog never embeds a capability scanner. The
`capability-conformance` adapter is part of the same envelope-driven
release-gate family as `release-evidence` and shares the same wire
version, exit-code authority rule, bounded fields, and secret
redaction; both are selected together by the `release` built-in
profile.
## Requirements
### Requirement: Capability-conformance versioned JSON envelope

The `capability-conformance` adapter MUST consume a versioned JSON envelope
on stdout (wire version `1`) with `status`, optional `severity`, a
`capabilities` sub-object (`declared` / `configured` / `verified` /
`unverified` arrays of strings), and the shared `findings` / `evidence` /
`missing_evidence` / `diagnostic` / `remediation` fields.

#### Scenario: Missing capabilities sub-object

- **WHEN** the envelope omits `capabilities`
- **THEN** the parser treats every list as empty and the required-evidence
  guard refuses a `PASS` claim

### Requirement: Capability-conformance scope guard

Every id in `capabilities.verified` MUST appear in the resolved Gate
plan (`scheduled_ids`); a verified id outside the plan downgrades a
`PASS` claim to `REVIEW_REQUIRED` so a project cannot claim
verification the gate cannot reproduce.

#### Scenario: Out-of-scope id

- **WHEN** `capabilities.verified` contains an id that is not in the
  resolved plan
- **THEN** the adapter records REVIEW_REQUIRED naming the out-of-scope
  ids in the diagnostic

### Requirement: Capability-conformance NOT_APPLICABLE semantics

A `NOT_APPLICABLE` envelope for an optional `capability-conformance`
check is the project-owned opt-out: the aggregate MUST NOT block. The
same status on a required check MUST be treated as missing coverage
and block, consistent with the existing aggregate rule.

#### Scenario: Optional NOT_APPLICABLE does not block

- **WHEN** the manifest sets `capability-conformance` to
  `required = false` and the envelope reports `NOT_APPLICABLE`
- **THEN** the aggregate status is `PASS` and the run is unblocked

#### Scenario: Required NOT_APPLICABLE blocks

- **WHEN** the manifest keeps `capability-conformance` as required and
  the envelope reports `NOT_APPLICABLE`
- **THEN** the aggregate status is `REVIEW_REQUIRED` and the run is
  blocked

