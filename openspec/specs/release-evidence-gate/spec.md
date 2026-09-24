# release-evidence-gate Specification

## Purpose
Generic Gate contract extension that adds the `release-evidence` and
`capability-conformance` concern IDs through a new `release` built-in
profile, and normalises the result with a versioned JSON envelope under
exit-code authority. The contract stays local-first, language-agnostic,
and free of provider SDKs, language scanners, SBOM generators, signing
tooling, deployment executors, or embedded LLMs; the project owns every
command binding and every envelope producer. A `PASS` claim for
`release-evidence` MUST include `revision`, `product_version`, at
least one `artifacts` entry, and `provenance`; a `PASS` claim for
`capability-conformance` MUST include a non-empty `verified` list
whose every id is in the resolved plan. Stale revisions and out-of-
scope verified ids downgrade to `REVIEW_REQUIRED`; missing or
malformed envelopes are `REVIEW_REQUIRED` and there is no text-mode
fallback, so missing coverage can never be silently treated as a pass.
## Requirements
### Requirement: Gate declared capability conformance

The Gate MUST support a project-owned `capability-conformance` command whose
result identifies declared, configured, verified, and unverified capabilities.

#### Scenario: Verified capability report

- **WHEN** the command emits a valid report with required capabilities verified
- **THEN** the Gate records PASS with bounded evidence

#### Scenario: Missing capability evidence

- **WHEN** a required declared capability has no evidence
- **THEN** the Gate records REVIEW_REQUIRED and blocks according to policy

### Requirement: Gate release evidence

The Gate MUST support a project-owned `release-evidence` command and MUST
require the selected profile's revision, version, artifact, integrity, and
provenance evidence before reporting a release gate PASS.

#### Scenario: Complete release evidence

- **WHEN** all profile-required evidence is valid and current
- **THEN** the Gate records PASS and persists the evidence identity

#### Scenario: Stale or contradictory evidence

- **WHEN** evidence names a different revision, has expired freshness, or
  contradicts the command exit status
- **THEN** the Gate refuses PASS and records FAIL or REVIEW_REQUIRED

### Requirement: Keep release execution generic

Driftwatchdog MUST execute and aggregate project commands without becoming a
package publisher, signer, SBOM generator, or deployment executor.

#### Scenario: Project-owned publisher

- **WHEN** a project command produces release evidence
- **THEN** Driftwatchdog records the result and does not publish or mutate an
  external registry

### Requirement: Recognize release-gate concern IDs

The Gate contract MUST recognize `release-evidence` and `capability-conformance`
as stable concern IDs without embedding a release publisher, capability
scanner, or language toolchain.

#### Scenario: Stable wire identifiers

- **WHEN** a Gate manifest selects either concern id
- **THEN** the Gate routes the command to a dedicated envelope adapter and
  the resolved plan lists the concern under its stable id

### Requirement: Built-in release profile

The Gate contract MUST expose a built-in `release` profile that schedules
both `release-evidence` and `capability-conformance` and treats them as
required by default.

#### Scenario: Profile schedules both concerns

- **WHEN** a manifest selects `profile = "release"`
- **THEN** the resolved plan contains both concerns in sorted order with
  `required = true` (unless an explicit `[[checks]]` relaxes one)

#### Scenario: Release concerns stay explicit on other plans

- **WHEN** a manifest selects a non-release profile
- **THEN** the resolved plan records both release-gate concerns in
  `explicitly not scheduled` so a reader sees they were consciously excluded

### Requirement: Versioned release-gate JSON envelope

The release-gate adapter MUST consume a versioned JSON envelope on stdout
(wire version `1`) carrying `status`, optional `severity`, optional
`revision`, optional `product_version`, optional `artifacts` list, optional
`provenance` (object or string), optional `sbom`, optional
`publication_state`, plus the shared `findings` / `evidence` /
`missing_evidence` / `diagnostic` / `remediation` fields.

#### Scenario: Wire-version mismatch

- **WHEN** the envelope's `version` is unknown or missing
- **THEN** the adapter records REVIEW_REQUIRED with the missing evidence key

### Requirement: Exit-code authority for release-gate envelopes

The release-gate adapter MUST enforce the same exit-code authority rule as
the product-quality contract: a status/exit mismatch downgrades the result
to REVIEW_REQUIRED with a bounded diagnostic and a missing evidence key.

#### Scenario: PASS claim with nonzero exit

- **WHEN** the envelope claims PASS and the command exits nonzero
- **THEN** the adapter records REVIEW_REQUIRED and the diagnostic names the
  actual exit and the exit-code-authority rule

### Requirement: Release-evidence required-evidence guard

A release-evidence PASS claim MUST include `revision`, `product_version`,
at least one entry in `artifacts`, and `provenance`; a missing or empty
field downgrades the result to REVIEW_REQUIRED with the missing fields
named in the diagnostic.

#### Scenario: PASS without provenance

- **WHEN** the envelope claims PASS but `provenance` is absent
- **THEN** the adapter records REVIEW_REQUIRED naming `provenance` as missing

### Requirement: Release-evidence stale-revision guard

When the project provides a current source revision (typically captured
from `git`) and the envelope's `revision` is present and disagrees with
it, the adapter MUST downgrade a PASS claim to REVIEW_REQUIRED so stale
evidence never silently passes the gate.

#### Scenario: Stale revision

- **WHEN** the envelope's `revision` does not match the captured current
  revision and the envelope claims PASS
- **THEN** the adapter records REVIEW_REQUIRED naming both revisions in
  the diagnostic

### Requirement: Capability-conformance required-evidence guard

A capability-conformance PASS claim MUST include a non-empty
`capabilities.verified` list whose every id appears in the resolved
plan; otherwise the adapter downgrades the result to REVIEW_REQUIRED.

#### Scenario: Empty verified list

- **WHEN** the envelope claims PASS but `capabilities.verified` is empty
- **THEN** the adapter records REVIEW_REQUIRED naming the empty `verified`
  list as the reason

#### Scenario: Out-of-scope verified id

- **WHEN** the envelope claims PASS and `capabilities.verified` contains
  an id that is not in the resolved plan
- **THEN** the adapter records REVIEW_REQUIRED naming the out-of-scope
  id in the diagnostic

### Requirement: No text-mode fallback for release-gate concerns

Unlike the product-quality contract, the release-gate adapter MUST NOT
fall back to text-mode mapping when the command exits cleanly without
emitting an envelope: missing coverage is REVIEW_REQUIRED so a project
cannot accidentally pass a release gate by emitting empty output.

#### Scenario: Legacy text-mode command

- **WHEN** a release-gate command emits no envelope and exits 0
- **THEN** the adapter records REVIEW_REQUIRED with the missing evidence
  key and a bounded diagnostic naming the absence

