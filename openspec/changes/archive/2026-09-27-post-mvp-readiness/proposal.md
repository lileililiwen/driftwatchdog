# Proposal: Post-MVP README readiness for Driftwatchdog

## Why

Driftwatchdog v0.1–v0.6 and the v1.1–v1.3 Gate releases are shipped and the
packaging suite is green. The README is usable but has three documentation gaps
that publication readiness should close:

1. No screenshot or checked-in terminal capture. The README shows fenced
   command text, but nothing shows an actual `driftwatch run` failure
   (§"Example"), a detected recurring bug, or a Gate run, so a reader cannot see
   the product working.
2. Rule catalog depth. The built-in profiles (`product`, `rust-product`,
   `release`) and stable concern IDs (`product-code-boundary`,
   `placeholder-threshold`, `release-evidence`, `capability-conformance`) are
   documented in separate prose sections; there is no single catalog tying each
   profile to its concerns, blocking default, and text-mode fallback behaviour.
3. Integration recipe. The README explains `.ai-gate/gate.yaml` and `gate.toml`
   in pieces, but a project owner has no ordered recipe from "no manifest" to a
   first `driftwatch gate` run.

Driftwatchdog owns the central workspace Gate: 75 `.ai-gate/gate.yaml`
declarations name `runtime: driftwatchdog`, so this README is a shared reference
for the portfolio. The change is documentation-only and does not expand the Gate.

## What Changes

- Add a checked-in terminal capture under `docs/assets/` showing a
  `driftwatch run` failure and recurring-bug report (and, optionally, a Gate
  run) on a synthetic project; redacted and reproducible offline.
- Add a consolidated built-in profile and concern catalog: profile → concern IDs
  → required/optional default → exit-code authority → text-mode fallback.
- Add an ordered integration recipe from no manifest to a first
  `driftwatch gate` run, covering `gate.toml`, `.ai-gate/gate.yaml`, blocking
  policy, and optional context providers.
- Keep the existing command, MCP, checker, and Gate sections accurate.

## Package Boundary and Split Assessment

**Single outcome:** a reader can see Driftwatchdog work, look up any built-in
profile/concern, and integrate it into a project by following one recipe.

**Included:** `README.md`; new capture assets under `docs/assets/`; the change
folder.

**Excluded:** `src/**` (especially `src/gate/**`), `Cargo.toml`, `tests/**`,
`templates/**`, `examples/**`, `npm/**`, `scripts/**`, and any canonical spec
beyond promoting this change.

**Split signals considered:** capture, catalog, and recipe share one owner
(`README.md`), one lifecycle (documentation), one contract (the shipped Gate
vocabulary and `tests/packaging/test_change_workflow.sh`), and one oracle (a
project owner can integrate and see the output). Splitting would leave the
shared Gate reference partial.

**Dependencies:** the shipped Gate profiles/concerns (`product-quality-gate-contract`,
`release-evidence-and-capability-gate`, `checker-machine-output`) and the
packaging suite.

## Sibling and Shared Architecture Reconnaissance

| Candidate | Evidence path/symbol | Reusable code/config/architecture | Compatibility gap | Owner and release boundary | Decision |
|---|---|---|---|---|---|
| this repository (central Gate owner) | `openspec/specs/generic-gate-contract/`, `product-quality-gate-contract/`, `release-evidence-gate/`, `capability-conformance-gate/`; `src/gate/concerns.rs` | the built-in profiles, stable concern IDs, exit-code authority rule, and envelope contracts documented across the README | the catalog is scattered; no capture or ordered recipe | driftwatchdog | **extend shared owner** (README only) |
| 75 workspace projects | `.ai-gate/gate.yaml` (`runtime: driftwatchdog`) | the Gate is the shared executor/aggregator for the portfolio | they consume the contract; the vocabulary is documented here, not re-declared | each project | **keep-local** (document the existing shared contract; no shared package) |
| workspace-governance | `docs/capabilities.md`, `release_evidence` | release-evidence field/state vocabulary | consumed by driftwatchdog's release-evidence concern; already documented | workspace-governance | **adopt** (document the consumption; do not re-declare) |
| forge | `driftwatch-cli-alignment` consumption of `driftwatch check --format json` | versioned checker document | different surface | forge | **not applicable** |
| jenkins-local | `deployment/projects.json` | deployment promotion ledger | consumes promotion, not Gate docs | jenkins-local | **not applicable** |

**Keep-local default and reason:** the README, capture, catalog, and recipe are
repository-specific documentation. The Gate contract is already shared and lives
here; this change **documents** it and must not modify `src/gate/**`, concern
IDs, envelopes, or CLI behavior. No shared package or library is proposed.

## BFS Impact Map

- **Capabilities:** `generic-gate-contract`, `product-quality-gate-contract`,
  `release-evidence-gate`, `capability-conformance-gate`, `checker-machine-output`,
  `quality-cicd-docs-ux`, `linux-macos-distribution`; documentation only.
- **Users/flows:** a project owner integrating the Gate for the first time; a
  reader evaluating the runtime-memory CLI; a reviewer checking a concern.
- **Contracts/data/persistence:** no runtime or storage change; the documented
  profile/concern names must match `src/gate/concerns.rs` and the canonical
  specs.
- **Integrations/config:** `gate.toml`, `.ai-gate/gate.yaml`, `driftwatch.toml`
  checkers, the reusable GitHub Actions template, optional context providers.
- **Callers:** the reader following the recipe; `tests/packaging/*` guard the
  change shape and repo hygiene.
- **Failure/boundary:** docs-only; must not expand the Gate, concern IDs,
  envelopes, or CLI.
- **Tests:** the packaging bash suite (including `test_change_workflow.sh`),
  `cargo test`, strict OpenSpec validation, `git diff --check`.
- **Dependencies/compatibility:** no code, dependency, or wire-contract change.
- **Privacy/security:** captures are redacted; the local-first, no-network
  default is restated, not changed.
- **Unaffected:** `src/**`, SQLite schema, checker protocol, MCP tools, npm
  launcher, installer, CI templates.

## Capabilities

### New Capabilities

- `readiness`: repository documentation readiness for Driftwatchdog (checked-in
  terminal capture, consolidated rule catalog, ordered integration recipe).

### Modified Capabilities

- None. No Gate behavior changes.

## Non-goals

- No change to `src/gate/**`, concern IDs, envelope wire version, or the
  exit-code authority rule.
- No new Gate profile, concern, rule, or checker.
- No change to `driftwatch check`, `driftwatch gate`, or any CLI surface.
- No publisher, signer, SBOM generator, or deployment executor behavior.
- No new runtime dependency, network call, or telemetry.
