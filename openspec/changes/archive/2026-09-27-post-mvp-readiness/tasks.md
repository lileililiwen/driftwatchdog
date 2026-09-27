# Tasks: Post-MVP README readiness for Driftwatchdog

## 1. BFS — Baseline and impact coverage

- [x] 1.1 Inventory the README sections, the built-in profiles and stable
  concern IDs in `src/gate/concerns.rs`, and the shipped canonical specs
  (`product-quality-gate-contract`, `release-evidence-gate`,
  `capability-conformance-gate`, `checker-machine-output`).
- [x] 1.2 Confirm the exact catalog facts: profile → concerns, required/optional
  defaults, exit-code authority, and which concerns keep a text-mode fallback.
- [x] 1.3 Capture real output from a synthetic local project for the terminal
  capture (`driftwatch run` failure + recurring bug, optional `driftwatch gate`).
- [x] 1.4 Confirm the change is documentation-only and touches no `src/**`,
  concern ID, envelope, CLI, or canonical spec.

## 2. DFS — Requirement-by-requirement implementation

- [x] 2.1 Add the checked-in terminal capture under `docs/assets/` and embed it
  in `README.md` with a resolving relative link.
- [x] 2.2 Add the consolidated built-in profile and concern catalog (profile →
  concern IDs → default → exit-code authority → text-mode fallback).
- [x] 2.3 Add the ordered integration recipe from no manifest to a first
  `driftwatch gate` run (`gate.toml`, `.ai-gate/gate.yaml`, blocking policy,
  optional context providers).
- [x] 2.4 Note the central workspace Gate ownership where the catalog introduces
  the shared contract, without changing the contract.
- [x] 2.5 Verify the command, MCP, checker, and Gate sections still match the
  code.

## 3. BFS — Cross-surface regression and completeness

- [x] 3.1 Re-read the README end to end for stale or duplicated Gate claims
  against `ROADMAP.md`/`HANDOFF.md` and the canonical specs.
- [x] 3.2 Verify every new and pre-existing relative link resolves and no
  external image host was introduced.
- [x] 3.3 Verify captures are redacted, drawn from a synthetic project, and
  match real output.
- [x] 3.4 Confirm no Gate expansion: no new profile, concern, rule, envelope,
  wire version, or CLI surface was added, and driftwatchdog remains an
  executor/aggregator.
- [x] 3.5 Confirm the diff touches only `README.md`, `docs/**`, and the change
  folder.

## 4. Verification

- [x] 4.1 `cargo fmt --check && cargo test && cargo clippy --all-targets
  --all-features -- -D warnings`.
- [x] 4.2 `sh tests/packaging.sh` passes (8/8) including
  `test_change_workflow.sh` and `test_repo_hygiene.sh`.
- [x] 4.3 Run the applicable local Gate (`driftwatch gate`) and record its actual
  status (synthetic project, `smoke` check, `PASS (blocked: false)`).
- [x] 4.4 `openspec validate --changes --strict --no-interactive` passes for this
  change.
- [x] 4.5 `git diff --check` is clean; record exact evidence in `HANDOFF.md`
  (status, next action, module map) and update `ROADMAP.md` only when
  sequencing or surface changes.
