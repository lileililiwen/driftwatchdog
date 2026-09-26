# Design: Export gate-run evidence for portfolio release governance

## 1. Implementation boundary

**Repository:** `/home/paul/code/driftwatchdog` — Rust.

**Files to change:** the Gate result/evidence modules under `src/gate/**` (the
aggregation result, the release-gate adapter, and the JSON output), a new export
entry point on the existing CLI surface, and the integration tests that drive a
synthetic project command. Read `src/gate/concerns.rs` for the concern IDs and
`src/gate/*` for the existing envelope/normalizer pattern before editing.

**Must not change:** existing concern IDs, the release-gate envelope wire
version, `driftwatch check`, the bounded-evidence store, or redaction.

## 2. Language and runtime

Rust (edition as pinned by the workspace), stdlib plus existing dependencies. No
new dependency for this change.

- `cargo fmt --check`
- `cargo test`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `openspec validate --changes --strict --no-interactive`

## 3. Ownership and shared code

Gate-local. The vocabulary is owned by Workspace Governance and **consumed** here.

- **Exported contract:** the export document (see §5).
- **Consumers:** Workspace Governance's audit; documented for Forge and
  Jenkins-local.
- **Dependency direction:** driftwatchdog consumes the governance vocabulary; the
  governance repository never imports driftwatchdog.
- **Release boundary:** ships with the CLI; a vocabulary change is a governance
  change, not a driftwatchdog one.

## 4. Behavioral model

| Gate outcome for a required check | Export state |
|---|---|
| ran and PASS, revision matches | `verified` |
| ran and FAIL | `unverified` |
| ran and REVIEW_REQUIRED | `unverified` |
| not scheduled | `unverified` |
| could not execute (tooling/environment) | `blocked` |
| run revision differs from the project revision | all fields `unverified` |

**Field mapping** (governance `release_evidence` fields, consumed not
re-declared): `revision`, `version`, `toolchain`, `artifacts`, `digests`, `sbom`,
`provenance`, `checks`, `publication`. A field with no executed check supplying
it MUST be emitted `unverified`, never omitted and never guessed.

**Invariants:** the export is a pure read of a completed run; it never mutates
the run, the project, or any registry.

## 5. Contract and compatibility

- The export is a new, versioned JSON document with `schema_version: 1`,
  `project_id`, `revision`, `toolchain`, and one entry per governance
  `release_evidence` field carrying `state` and an `evidence_ref` into the
  bounded-evidence store.
- **Compatibility:** additive. Existing Gate results, envelopes, concern IDs,
  text-mode behaviour, and `driftwatch check` are unchanged.
- The export MUST NOT emit a field name outside the governance set; an unknown
  field is a hard error at construction.

## 6. Failure and boundary policy

| Case | Result |
|---|---|
| No completed run for the project | export refuses, names the project, exits non-zero |
| Run revision differs from project revision | every field `unverified`; export succeeds with a diagnostic |
| Check could not execute | field `blocked` |
| Evidence artifact missing or unredactable | field `unverified`; never `verified` |
| Unknown governance field requested | construction error; never emitted |
| Export requested in dry-run | no write, prints the document |

## 7. Verification oracle

- `cargo test` — new integration tests drive synthetic project commands covering
  ran/PASS, ran/FAIL, not-scheduled, could-not-execute, and stale-revision cases,
  and assert the resulting per-field states.
- An export for a stale revision contains no `verified` state.
- `cargo fmt --check && cargo clippy --all-targets --all-features -- -D warnings`
  — clean.
- `openspec validate --changes --strict --no-interactive` — pass.
- Consuming the export must not be required to prove this change; a governance
  audit acceptance run is named as the next action, not claimed here.

## 8. Decision ledger

**Assumptions:** Workspace Governance's field and state names are stable
(`docs/capabilities.md`); the bounded-evidence store can carry the references.

**Resolved alternatives:**

- (a) Export from the Gate result vs a separate scanner → chose **export**, so a
  single execution is the only evidence source.
- (b) Emit `verified` for a passing aggregate vs per-check → chose
  **per-check**, so a partially-run gate cannot over-claim.
- (c) Own the vocabulary in Rust vs consume governance's → chose **consume**, to
  avoid the duplication the portfolio is unwinding.
- (d) Fail the export on a stale revision vs emit all-unverified → chose
  **emit all-unverified**, so the document is still diagnosable.

**Deferred:** the governance-side consumption (its own change).

**Blockers:** none.
