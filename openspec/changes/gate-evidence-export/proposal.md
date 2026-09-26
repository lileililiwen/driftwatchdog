# Proposal: Export gate-run evidence for portfolio release governance

## Why

The Gate already normalises release evidence (`release-evidence-gate`) and
records bounded artifacts (`evidence-and-artifacts`), but nothing exports a
completed run in the vocabulary Workspace Governance uses for the
`release_evidence` block of `.project.json`. As a result the portfolio's seven
deployable projects carry hand-written `configured` states that point at
pilot-context placeholders, and the governance audit cannot distinguish an
executed gate from a declaration. 73 `.ai-gate/gate.yaml` declarations exist in
the workspace and only this repository's own CI runs one.

## What Changes

- Add a governance-facing evidence export produced from an existing Gate run.
- Map only checks that actually ran and passed to `verified`; every other check
  stays `unverified` or `blocked`.
- Refuse `verified` when the run's revision differs from the project's current
  revision.
- Consume Workspace Governance's `release_evidence` field names and state
  vocabulary (`docs/capabilities.md`); do not re-declare them in Rust.
- Keep Driftwatchdog an executor and aggregator: no publisher, signer, or
  registry client.

## BFS Impact Map

- **Capabilities:** Gate planning, execution, aggregation, evidence
  persistence, and JSON output.
- **Consumers:** Workspace Governance (audit), Forge, Jenkins-local, and
  project release workflows.
- **Persistence:** reuse the existing bounded-evidence store; no new schema
  unless a required field cannot fit the current envelope.
- **Failure:** absent, unrun, or stale evidence is `unverified`; a Gate that
  cannot execute a check never yields `verified`.
- **Compatibility:** `driftwatch check`, the existing concern IDs, and the
  existing release-gate envelope are unchanged.

## Capabilities

- `gate-evidence-export`.

## Non-goals

- No release publisher, package-registry client, signer, SBOM generator, or
  deployment executor.
- No new evidence vocabulary; Workspace Governance owns the field and state
  names.
- No automatic promotion of a project to `deployable`.
- No change to existing concern IDs, envelopes, or text-mode behaviour.

## Package Boundary and Split Assessment

**Single outcome:** a completed Gate run can be exported as an evidence record
that Workspace Governance's audit consumes.

**Included:** `src/gate/**` evidence/export path, the Gate JSON output, tests,
and documentation.

**Excluded:** the governance-side consumption (that repository's own change),
the release-evidence concern itself (shipped), and any promotion decision.

**Split signals considered:** the export and its governance mapping share one
owner, one lifecycle (the Gate result), one contract (the governance field and
state names) and one oracle (a governance-accepted record). The mapping cannot
be separated from the export without producing a record nobody can consume.

**Dependencies:** Workspace Governance's `release-evidence` vocabulary (shipped)
and its `registry-and-declaration-integrity` audit (shipped). Blocks the
`gate-run-evidence` portfolio package's consumption step.

## Sibling and Shared Architecture Reconnaissance

| Candidate | Evidence path/symbol | Reusable code/config/architecture | Compatibility gap | Owner and release boundary | Decision |
|---|---|---|---|---|---|
| this repository | `openspec/specs/release-evidence-gate/spec.md`, `src/gate/concerns.rs` | release-evidence concern, envelope, stale/required guards | produces a Gate result, not a governance record | driftwatchdog | **extend shared owner** |
| this repository | `openspec/specs/evidence-and-artifacts/spec.md` | bounded evidence, digests, redaction | already satisfies the artifact boundary | driftwatchdog | **adopt** (reuse, do not replace) |
| workspace-governance | `docs/capabilities.md`, `.project.json` `release_evidence` | the field and state vocabulary | governance-side consumption is out of scope here | workspace-governance | **adapt through a generic adapter** (export in its vocabulary) |
| forge | `forge gate` integration | consumes Gate results | different surface | forge | **not applicable** |
| jenkins-local | `deployment/projects.json` | deployment execution | consumes promotion ledger, not gate evidence | jenkins-local | **not applicable** |

## Evidence boundary

This change can prove that a Gate run produces a governance-vocabulary evidence
record with correct `verified`/`unverified` states and staleness behaviour. It
cannot prove that a project's artifact is releasable, that a registry received a
package, or that a deployment happened.
