# Tasks: bind-governance-quality-checks

## 1. BFS — Baseline and impact coverage

- [x] 1.1 Record the baseline before any edit. Measured on `main` at `61f1ae2`
      with a clean tree:

      | Gate | Baseline |
      |---|---|
      | `cargo fmt --check` | clean, exit 0 |
      | `cargo test` — lib | **604 passed**, 0 failed |
      | `cargo test` — integration suites | **every suite passes** (32, 12, 6, 10, 7, 3, 13, 14, 15, 19, 13, 15, 7, 2, 5, 6, 15) |
      | `cargo test --test packaging` | **FAILS** — `packaging_bash_and_node_suites_pass` |
      | `openspec validate --changes --strict` | recorded at implementation time |

      **The packaging failure is pre-existing and is not this change's to
      absorb.** `tests/packaging/run_all.sh` reports two failing suites:

      - `installer` — `unexpected installed path`. **Pre-existing and
        unrelated.** `tests/packaging/test_installer.sh` never reads
        `openspec/`; it exercises `scripts/install.sh` and `scripts/lib`,
        which commit `61f1ae2` did not touch. The last commit to change
        either file is `45eb8c7`, and `61f1ae2` is the only commit since.
      - `change_workflow` — `bind-governance-quality-checks: tasks.md missing
        BFS phase heading`. **This one was mine.** The previous version of
        this file used `## 1. Boundary` / `## 2. The adapter` headings and
        carried no BFS/DFS/BFS phase at all, violating the workflow
        `AGENTS.md` documents and `test_change_workflow.sh` enforces. This
        file has been restructured into the house three-phase form. The test
        was **not** relaxed; the artifact was corrected to match it.

      The change is acceptable only if `installer` remains the sole failure
      afterwards, and that failure is reported to the owner rather than
      quietly suppressed.

- [x] 1.2 Re-measure the blast radius. The numbers `design.md` §7 recorded are
      **stale** and the design says so at §7's own figures. `design.md` was
      written when the claim checker reported 4 contradicted repositories;
      `workspace-governance` `833ca9c` established that all four were **false
      positives** and corrected the evidence rules, returning the fleet to its
      `errors=211` baseline with **zero** `QUALITY_README_CLAIM_CONTRADICTED`
      findings. Re-measured live:

      | Code | Level | Count at `61f1ae2` | Count now |
      |---|---|---|---|
      | `QUALITY_README_CLAIM_CONTRADICTED` | ERROR | 4 | **0** |
      | `QUALITY_README_CLAIM_UNSCOPED` | WARNING | 1 aggregate | **1** aggregate |
      | `RULES_DRIFT_NEW` | ERROR | 0 | **0** |
      | `RULES_DRIFT_BASELINE_STALE` | WARNING | 0 | **0** |

      The consequence is stated in task 3.3: the concern still binds, and this
      repository's own result is now `PASS` rather than a failure. That is the
      correct outcome, not a weakened check.

## 2. DFS — Requirement-by-requirement implementation

### The adapter

- [ ] 2.1 Add the `governance-quality` concern under `src/gate/`, reusing the
      existing command binding, envelope parsing, exit-code mapping and
      aggregation unchanged. No second mechanism.
- [ ] 2.2 Run the governance checker and read its `--json` output. Keep only
      findings whose `code` is in the configured set. Report the **filtered**
      count, never the checker's total.
- [ ] 2.3 Implement the envelope mapping: 0 matching → `PASS` (0);
      matching WARNINGs → `REVIEW_REQUIRED` (2); matching ERRORs → `FAIL` (1);
      ERROR dominates a mixed run.
- [ ] 2.4 Implement the failure path for every row of design §5: missing file →
      `UNBOUND`; unexpected exit, unparseable JSON, timeout, unrecognised level →
      `REVIEW_REQUIRED`. No row may yield `PASS`.
- [ ] 2.5 Default code set: `QUALITY_README_CLAIM_CONTRADICTED`,
      `QUALITY_README_CLAIM_UNSCOPED`, `RULES_DRIFT_NEW`,
      `RULES_DRIFT_BASELINE_STALE`. A project may narrow it; an empty set while
      the concern is enabled is a `REVIEW_REQUIRED` configuration error naming
      the empty scope.
- [ ] 2.6 The concern is **not exempable**. Confirm the existing cap
      `min(floor(0.25 × required), 5)` and `NON_EXEMPTABLE_ALIASES` are untouched
      and that this concern cannot be removed by the exemption mechanism.

### Registration

- [ ] 3.1 Add `governance-quality` to the built-in `product` and `rust-product`
      profiles, alongside `repository`, `build`, `tests` and `security`.
- [ ] 3.2 Add it to the `.ai-gate/gate.yaml` template with a command and
      `blocking: [FAIL, REVIEW_REQUIRED]`.
- [ ] 3.3 Enable it in this repository's own `.ai-gate/gate.yaml` and prove it
      appears in the run output.

      **Corrected expectation.** The earlier wording demanded "a non-zero
      result for this repository's current state", which was true only while
      the 4 false-positive findings existed. Per task 1.2 they are gone, and
      this repository contributes none of the 3 unscoped statements, so the
      honest result for `driftwatchdog` itself is **`PASS` (0)**.

      That is not a gap in the proof and must not be turned into one. A gate
      concern is proven wired by two things, both of which are achievable:
      the concern appears in this repository's own gate run, and the failure
      paths return non-zero under test (task 4.2). Manufacturing a non-zero
      fleet result to satisfy a stale sentence would be the "checker that
      reports OK without checking" failure D18 exists to prevent, run in
      reverse. Recorded rather than papered over.

## 3. BFS — Cross-surface regression and completeness

- [ ] 4.1 One test per cell of the envelope mapping table, including ERROR
      dominating a mixed run.
- [ ] 4.2 One test per row of the failure-path table, each asserting the exact
      status and exit code, plus an explicit assertion that no cannot-run case
      yields `PASS`.
- [ ] 4.3 A test proving the adapter reports the filtered count, not the total:
      a 215-finding run scoped to 4 findings must report 4.
- [ ] 4.4 A test proving an empty code set is a configuration error, not `PASS`.
- [ ] 4.5 A test proving the concern is `UNBOUND` in a project with no bound
      command — the state of 70 repositories today.
- [ ] 4.6 A test proving this repository's own `PASS` is earned rather than
      assumed: the same binding, pointed at a fixture carrying one matching
      ERROR, must return `FAIL` (1). Without this, task 3.3's `PASS` would be
      indistinguishable from a concern that never runs.

## 4. Documentation and the deferred dependency

- [ ] 5.1 Update the Gate concern documentation and the `.ai-gate` reference with
      the envelope mapping, the failure path, and the scope-versus-exemption
      distinction.
- [ ] 5.2 State the ordering constraint in the docs: the concern reaches the 70
      repositories that still have no bound command only when
      `gate-unbound-check-fails` task 5.1 lands. Until then it is live here and
      `UNBOUND` elsewhere. Do not claim fleet coverage this change does not have.
- [ ] 5.3 Record the **corrected** blast radius in the handoff: 0 of 211 errors
      and 1 of 18 warnings are surfaced today, against 211 errors deliberately
      left to the governance audit alone. `design.md` §7's figures of 4 and 215
      are superseded by task 1.2 and the handoff must not repeat them.

## 5. Verification

- [ ] 6.1 `cargo fmt --check && cargo test && cargo clippy --all-targets
      --all-features -- -D warnings && openspec validate --changes --strict
      --no-interactive`, all no worse than the §1.1 baseline — that is, lib
      still 604+ passing, every integration suite still passing, and
      `installer` the **only** packaging failure.
- [ ] 6.2 `openspec validate bind-governance-quality-checks --strict
      --no-interactive` passes, and the delta is filed under an existing
      capability name in `openspec/specs/`.
- [ ] 6.3 Prove the concern is wired: it appears in this repository's own gate
      run, and task 4.6 proves the same binding fails on a matching ERROR. A
      verifier that no gate runs does not gate.
- [ ] 6.4 `git diff --check` clean. Do not archive; this change is not complete
      until 5.2's dependency is recorded as still-open.
