# Design: Release evidence and capability Gate concerns

## Concern model

`capability-conformance` checks a project-owned report against its capability
declaration. `release-evidence` checks a project-owned report describing the
release candidate and artifacts. Driftwatchdog treats both as generic command
results; it does not parse Cargo, NuGet, npm, Docker, or Jenkins internals.

## Evidence requirements

Required release evidence may include source revision, version, toolchain,
artifact digest, SBOM, provenance/signature status, checks, and publication
state. A project profile selects which fields are mandatory. Missing, stale,
unparseable, or contradictory evidence produces REVIEW_REQUIRED or FAIL and
retains remediation in the Gate result.

## Aggregation

Required failed/review-required concerns block. Optional missing evidence is
NOT_APPLICABLE and remains visible. The manifest digest, rule-pack version,
command output, and bounded evidence are persisted through the current Gate
pipeline.

## Verification

Use temporary producer commands for complete, incomplete, stale, contradictory,
and malformed reports. Verify human/JSON parity, dry-run no-write behavior,
redaction, history readiness, and unchanged checker-only behavior.
