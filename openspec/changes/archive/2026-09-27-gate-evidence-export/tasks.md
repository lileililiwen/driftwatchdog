# Tasks: Export gate-run evidence for portfolio release governance

## 0. Caveats and confirmations

- [x] **Do not rebuild the release-evidence guards.**
  `openspec/specs/release-evidence-gate/spec.md` already owns the
  required-evidence guard, the stale-revision guard, the exit-code authority rule
  and the no-text-mode-fallback rule. Reuse them; this change adds the *export*.
- [x] **Do not add a second evidence store.**
  `openspec/specs/evidence-and-artifacts/spec.md` owns bounded evidence, digests
  and redaction; the export references it.
- [x] **Read the governance vocabulary, never restate it.**
  `workspace-governance/docs/capabilities.md` owns the `release_evidence` field
  and state names. Needing a field outside that set is a governance change, not a
  Rust one.
- [x] **Keep this repository's required task shape.** `AGENTS.md` requires
  `proposal.md` to be the BFS impact map and `tasks.md` to keep explicit BFS, DFS
  and final-BFS groups; follow the two-commit completion workflow and do not push.
- [x] **Name the governance-side consumption as the next action.** Consuming the
  export belongs to `workspace-governance`; do not claim it here.
- [x] **Stage only this change directory.** `.project.json` is already modified and
  `scripts/check-openspec-change-names.mjs` is untracked; keep them out of the
  change's commits.

## 1. BFS — Baseline and impact coverage

- [x] Inventory the Gate result structure, the release-gate adapter, the
  bounded-evidence store, JSON output, and the CLI entry points.
- [x] Confirm Workspace Governance's `release_evidence` field and state names
  from `docs/capabilities.md` and record them as the consumed contract.
- [x] Add fixtures for ran/PASS, ran/FAIL, not-scheduled, could-not-execute and
  stale-revision runs before implementation.
- [x] Confirm the change does not alter concern IDs, envelopes, or
  `driftwatch check`.

## 2. DFS — Requirement-by-requirement implementation

- [x] Implement the export document and the per-check → per-field state mapping.
- [x] Wire the export entry point to a completed run; refuse when no run exists.
- [x] Implement the stale-revision rule (all fields `unverified`).
- [x] Implement the unknown-field construction error.
- [x] Add human and JSON output, and documentation of the export schema.

## 3. BFS — Cross-surface regression and completeness

- [x] Re-run the full Gate, doctor, adapter and checker-only suites.
- [x] Verify dry-run writes nothing and prints the document.
- [x] Verify redaction and bounded-evidence references are preserved.
- [x] Verify Driftwatchdog remains an executor: no publisher, signer, or
  registry client was added.

## 4. Verification

- [x] `cargo fmt --check && cargo test && cargo clippy --all-targets
  --all-features -- -D warnings`.
- [x] Run the applicable local Gate and record its actual status.
- [x] `openspec validate --changes --strict --no-interactive`.
- [x] Record the governance-side consumption as the next action; do not claim it
  here.
