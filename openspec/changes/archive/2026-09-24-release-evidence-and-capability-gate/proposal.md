# Proposal: Add release-evidence and capability Gate concerns

## Why

Driftwatchdog executes project checks, but the current Gate vocabulary does not
make release provenance, artifact integrity, or declared platform capability
coverage first-class. Projects can run build/tests without proving that the
released artifact matches the source or that required platform concerns were
verified.

## What Changes

- Add generic concern IDs for `capability-conformance` and
  `release-evidence`.
- Preserve project-owned commands and report normalization.
- Require bounded evidence for revision, version, artifacts, digests, SBOM,
  provenance, and publication state where a project selects release gating.
- Keep optional checks visible and prevent metadata-only PASS claims.

## BFS Impact Map

- **Capabilities:** Gate planning, execution, aggregation, history, and JSON.
- **Consumers:** Workspace Governance, Forge, Jenkins-local, and release
  workflows across C#, Rust, TypeScript, and Python projects.
- **Persistence:** existing Gate result storage; no schema migration unless a
  bounded evidence field cannot fit the current envelope.
- **Failure:** missing required evidence is REVIEW_REQUIRED or FAIL according
  to project policy; stale or contradictory evidence cannot pass.
- **Compatibility:** checker-only `driftwatch check` remains unchanged.

## Capabilities

- `release-evidence-gate`.
- `capability-conformance-gate`.

## Non-goals

- No release publisher, package registry client, signer, SBOM generator, or
  deployment executor inside Driftwatchdog.
- No automatic capability adoption or project migration.
- No runtime claim from a Gate result without the declared evidence.
