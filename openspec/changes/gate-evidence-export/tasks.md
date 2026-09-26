# Tasks: Export gate-run evidence for portfolio release governance

## 0. Caveats and confirmations

- [ ] **Do not rebuild the release-evidence guards.**
  `openspec/specs/release-evidence-gate/spec.md` already owns the
  required-evidence guard, the stale-revision guard, the exit-code authority rule
  and the no-text-mode-fallback rule. Reuse them; this change adds the *export*.
- [ ] **Do not add a second evidence store.**
  `openspec/specs/evidence-and-artifacts/spec.md` owns bounded evidence, digests
  and redaction; the export references it.
- [ ] **Read the governance vocabulary, never restate it.**
  `workspace-governance/docs/capabilities.md` owns the `release_evidence` field
  and state names. Needing a field outside that set is a governance change, not a
  Rust one.
- [ ] **Keep this repository's required task shape.** `AGENTS.md` requires
  `proposal.md` to be the BFS impact map and `tasks.md` to keep explicit BFS, DFS
  and final-BFS groups; follow the two-commit completion workflow and do not push.
- [ ] **Name the governance-side consumption as the next action.** Consuming the
  export belongs to `workspace-governance`; do not claim it here.
- [ ] **Stage only this change directory.** `.project.json` is already modified and
  `scripts/check-openspec-change-names.mjs` is untracked; keep them out of the
  change's commits.

## 1. BFS — Baseline and impact coverage

- [ ] Inventory the Gate result structure, the release-gate adapter, the
  bounded-evidence store, JSON output, and the CLI entry points.
- [ ] Confirm Workspace Governance's `release_evidence` field and state names
  from `docs/capabilities.md` and record them as the consumed contract.
- [ ] Add fixtures for ran/PASS, ran/FAIL, not-scheduled, could-not-execute and
  stale-revision runs before implementation.
- [ ] Confirm the change does not alter concern IDs, envelopes, or
  `driftwatch check`.

## 2. DFS — Requirement-by-requirement implementation

- [ ] Implement the export document and the per-check → per-field state mapping.
- [ ] Wire the export entry point to a completed run; refuse when no run exists.
- [ ] Implement the stale-revision rule (all fields `unverified`).
- [ ] Implement the unknown-field construction error.
- [ ] Add human and JSON output, and documentation of the export schema.

## 3. BFS — Cross-surface regression and completeness

- [ ] Re-run the full Gate, doctor, adapter and checker-only suites.
- [ ] Verify dry-run writes nothing and prints the document.
- [ ] Verify redaction and bounded-evidence references are preserved.
- [ ] Verify Driftwatchdog remains an executor: no publisher, signer, or
  registry client was added.

## 4. Verification

- [ ] `cargo fmt --check && cargo test && cargo clippy --all-targets
  --all-features -- -D warnings`.
- [ ] Run the applicable local Gate and record its actual status.
- [ ] `openspec validate --changes --strict --no-interactive`.
- [ ] Record the governance-side consumption as the next action; do not claim it
  here.
