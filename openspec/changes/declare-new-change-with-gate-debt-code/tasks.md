# Tasks: Declare `QUALITY_NEW_CHANGE_WITH_GATE_DEBT`

## 1. BFS — Reproduce the false green first

- [x] Build a fixture where `workspace_check.py` emits the code and exits 1
- [x] Run the real `governance-quality` concern against it
- [x] Record the result: `PASS`, "0 of 93 governance findings matched this
      concern's scope", exit 0
- **Done when** the false green is a measurement, not an argument.

## 2. DFS — One coherent contract

- [x] `proposal.md` — why, what changes, what does not
- [x] `specs/generic-gate-contract/spec.md` — MODIFIED the code-ownership
      requirement with two scenarios (undeclared → false green; declared → FAIL)
- [x] ADDED a requirement that every declared code states its rationale
- [x] `design.md` — the measured table, the rejected alternatives, blast radius
- [x] `tasks.md` — this file
- **Done when** `openspec validate --changes --strict` passes on this change.

### 2.1 Declare the code

- [x] Add `QUALITY_NEW_CHANGE_WITH_GATE_DEBT` to
      `GOVERNANCE_QUALITY_DECLARED_CODES`
- [x] Write the rationale beside the constant, naming the failure its absence
      would cause
- **Done when** the entry is in the array and the reason is in the file.

### 2.2 Pin it with a test

- [x] `a_new_change_on_a_repository_carrying_gate_debt_is_a_gate_failure`
- [x] Asserts `FAIL`, exit 1, 1 finding, severity `Error`, rule name present
- [x] The test fails if the declaration is removed — otherwise it is not
      testing the declaration
- **Done when** removing the constant turns the test red and restoring it turns
      it green. Measured, not assumed.

## 3. BFS — Re-audit every consumer

- [x] `GOVERNANCE_QUALITY_DECLARED_CODES` has exactly two readers: the filter
      that forms the verdict, and the refusal that names the supported codes.
      Both are in `src/gate/governance/mod.rs` and both read the same constant,
      so one entry reaches both. No second site to update.
- [x] Confirm narrowing still works — a project declaring a subset is still
      filtered downward, unchanged.
- [x] Confirm widening is still refused — the refusal test is against the
      declared set, and adding a code does not widen what a project may declare.
- [x] Confirm no other concern filters on the same codes
      (`src/gate/product_quality.rs`, `src/gate/release_evidence.rs`,
      `src/gate/source_file_size.rs` read their own sets).
- **Done when** the only file needing a change is the one already changed.

## 4. BFS — Spec, validation and verification

- [x] `openspec validate --changes --strict` — validation runs on the whole
      change, not just the new spec file
- [x] `sh tests/packaging/run_all.sh` — the `change_workflow` suite validates
      this file's shape, and it is part of `cargo test`
- [x] `cargo test` — full suite, counting passes and failures. Baseline before
      this change: 908 passed, 0 failed, 29 suites. This change adds one test.
- [x] Negative control: remove the constant, confirm the new test reports
      `left: Pass, right: Fail`, restore the constant, confirm green
- **Done when** the numbers are in `HANDOFF.md` as measurements.

## 5. BFS — Commit

- [x] Commit 1: `proposal.md`, `design.md`, `specs/`, `tasks.md`,
      `src/gate/governance/mod.rs`, `src/gate/governance/tests.rs`
- [x] Commit 2: `HANDOFF.md` only
- [x] Explicit paths, never `git add -A`
- [x] On `main`. No branch, no worktree, no push, not archived.
