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

- [x] 2.1 Add the `governance-quality` concern under `src/gate/`, reusing the
      existing command binding, envelope parsing, exit-code mapping and
      aggregation unchanged. No second mechanism.
- [x] 2.2 Run the governance checker and read its `--json` output. Keep only
      findings whose `code` is in the configured set. Report the **filtered**
      count, never the checker's total.
- [x] 2.3 Implement the envelope mapping: 0 matching → `PASS` (0);
      matching WARNINGs → `REVIEW_REQUIRED` (2); matching ERRORs → `FAIL` (1);
      ERROR dominates a mixed run.
- [x] 2.4 Implement the failure path for every row of design §5: missing file →
      `UNBOUND`; unexpected exit, unparseable JSON, timeout, unrecognised level →
      `REVIEW_REQUIRED`. No row may yield `PASS`.
- [x] 2.5 Default code set: `QUALITY_README_CLAIM_CONTRADICTED`,
      `QUALITY_README_CLAIM_UNSCOPED`, `RULES_DRIFT_NEW`,
      `RULES_DRIFT_BASELINE_STALE`. A project may narrow it; an empty set while
      the concern is enabled is a `REVIEW_REQUIRED` configuration error naming
      the empty scope.
- [x] 2.6 The concern is **not exempable**. Confirm the existing cap
      `min(floor(0.25 × required), 5)` and `NON_EXEMPTABLE_ALIASES` are untouched
      and that this concern cannot be removed by the exemption mechanism.

### Registration

- [x] 3.1 Add `governance-quality` to the built-in `product` and `rust-product`
      profiles, alongside `repository`, `build`, `tests` and `security`.
- [ ] 3.2 Add it to the `.ai-gate/gate.yaml` template with a command and
      `blocking: [FAIL, REVIEW_REQUIRED]`.

      **Deliberate deviation; the "with a command" half is not delivered.**
      The documented template shows the concern's shape — the
      `governance_quality: codes: [...]` scope block and a commented
      example of the `commands.governance-quality` binding — but ships no
      live command. The checker lives in a sibling repository whose path no
      template can know, and a template that bound a guessed path would
      break every project that copied it. `blocking: [FAIL, REVIEW_REQUIRED]`
      was already the template's blocking list and is unchanged. Binding is
      each project's own decision, which is what design §6 requires.

      Left **unchecked** rather than claimed: the task asked for a bound
      command and there is none. The half that was delivered is
      documentation, and the half that was not is an owner decision about
      whether a template may name a cross-repository path at all.
- [x] 3.3 Enable it in this repository's own `.ai-gate/gate.yaml` and prove it
      appears in the run output.

      **Corrected expectation, twice.** The earlier wording demanded "a
      non-zero result for this repository's current state", true only while
      the 4 false-positive findings existed; task 1.2 removed them and
      predicted `PASS` (0) here. That prediction was itself wrong, because
      it assumed this repository had a command bound to run at all.

      Measured: **this repository binds no commands whatsoever.** Its
      `.ai-gate/gate.yaml` declares only `checks:` booleans and `blocking:`,
      with no `commands:` section, so all eight of its required checks record
      `UNBOUND` and the aggregate is `FAIL` before and after this change.
      The honest result for `governance-quality` here is therefore
      **`UNBOUND` (1)** — the same state as the six checks above it, and
      exactly what design §6 anticipates: "a concern that is enabled in the
      profile but unbound in a project is `UNBOUND`, which is a real result
      and is reported as one."

      **Decision: enable the concern and bind nothing** (rather than binding
      the sibling checker). Binding `python3
      ../workspace-governance/scripts/workspace_check.py --json` would make
      this repository's gate depend on a sibling repository's on-disk
      layout, and would be the only one of eight checks actually bound —
      inconsistent with the repository's own state. The cost is that this
      repository's own run cannot demonstrate the mapping; task 4.6 and
      `tests/gate_governance_quality.rs` do that instead, with a real
      binding and a fixture.

      The concern is wired, not merely enabled: it resolves from the
      `rust-product` profile, appears in `--dry-run` and in the run output
      with a real status, and its result is earned by the same code path
      that produces `PASS` in the tests.

## 3. BFS — Cross-surface regression and completeness

- [x] 4.1 One test per cell of the envelope mapping table, including ERROR
      dominating a mixed run.
- [x] 4.2 One test per row of the failure-path table, each asserting the exact
      status and exit code, plus an explicit assertion that no cannot-run case
      yields `PASS`.
- [x] 4.3 A test proving the adapter reports the filtered count, not the total:
      a 215-finding run scoped to 4 findings must report 4.
- [x] 4.4 A test proving an empty code set is a configuration error, not `PASS`.
- [x] 4.5 A test proving the concern is `UNBOUND` in a project with no bound
      command — the state of 70 repositories today.
- [ ] 4.6 A test proving the concern's mapping is earned rather than assumed:
      the same binding, pointed at a clean report and at one carrying a
      matching `ERROR`, must read `PASS` and `FAIL` respectively. Without
      this, the mapping would be an assertion nothing exercises.

      **Re-scoped under Option B.** As originally written this task asked
      for the proof to come from *this repository's own* `PASS`. That
      premise is void: this repository binds no command and reads
      `UNBOUND`, so its run cannot demonstrate the mapping at all. The test
      is therefore stated against a real binding in a fixture instead, in
      `tests/gate_governance_quality.rs`. The property the task protects is
      unchanged — a concern that cannot fail is an assumption, not a check.
      The checkbox stays **unchecked** because the wording no longer matches
      what was built; the build is in `governance/tests.rs` and
      `tests/gate_governance_quality.rs`, and the reworded task is the
      owner decision, not the implementer's.

## 4. Documentation and the deferred dependency

- [x] 5.1 Update the Gate concern documentation and the `.ai-gate` reference with
      the envelope mapping, the failure path, and the scope-versus-exemption
      distinction.
- [x] 5.2 State the ordering constraint in the docs: the concern reaches the 70
      repositories that still have no bound command only when
      `gate-unbound-check-fails` task 5.1 lands. Until then it is live here and
      `UNBOUND` elsewhere. Do not claim fleet coverage this change does not have.
- [x] 5.3 Record the **corrected** blast radius in the handoff.

      **Superseded a second time.** Task 1.2's figures (211 errors, 18
      warnings) no longer match the live audit either. Measured on 2026-10-03
      from this repository
      (`python3 ../workspace-governance/scripts/workspace_check.py --json`,
      exit 1): **157 findings, 154 `ERROR`, 3 `WARNING`**, of which **0** are
      in this concern's code scope. The handoff records these live numbers
      and states that `design.md` §7's 4/215 and task 1.2's 211/18 are both
      superseded. 154 errors are deliberately left to the governance audit
      alone; this concern surfaces none of them.

## 5. Verification

- [x] 6.1 `cargo fmt --check && cargo test && cargo clippy --all-targets
      --all-features -- -D warnings && openspec validate --changes --strict
      --no-interactive`, all no worse than the §1.1 baseline — that is, lib
      still 604+ passing, every integration suite still passing, and
      `installer` the **only** packaging failure.

      **`source-file-size` is unchanged at 6 over-limit files.** A single
      `src/gate/governance.rs` reached 1,036 lines and would have been a
      7th, so the unit tests were moved to `src/gate/governance/tests.rs`
      behind `#[cfg(test)] mod tests;` and the production code to
      `src/gate/governance/mod.rs`.

      Measured: `cargo fmt --check` clean; `cargo clippy --all-targets
      --all-features -- -D warnings` clean; lib **623 passed, 0 failed**
      (604 baseline + 19 new: 18 unit tests in `src/gate/governance/tests.rs`, 1
      in `src/gate/concerns.rs`); every integration suite at its baseline
      count plus the new `gate_governance_quality` suite at **11 passed**;
      `openspec validate --changes --strict --no-interactive` **2 passed, 0
      failed**; `sh tests/packaging/run_all.sh` **FAIL installer**, all nine
      other suites ok; `git diff --check` clean.

      **Existing tests updated, none weakened.** Adding a concern to the
      `product` profile made it unbound in product-profile fixtures, which
      blocked their runs. Each affected fixture gained an explicit binding or
      `required = false` for the new concern, which is what those fixtures
      already do for the sibling product concerns:
      `tests/gate_product_quality.rs` (a `governance_binding` helper,
      applied to the 7 fixtures that assert a passing run),
      `tests/gate_source_file_size.rs` (the `source_manifest` helper plus
      the `rust-product` fixture), and
      `tests/gate_exemption_cap.rs` (the spine diagnostic now names 5
      concerns, so the assertion checks the concern name is an element of
      that list instead of pinning a phrase that only ever abutted the last
      entry). `src/gate/manifest.rs`'s three profile/alias tests were
      updated to the new profile set and alias count.
- [x] 6.2 `openspec validate bind-governance-quality-checks --strict
      --no-interactive` passes, and the delta is filed under an existing
      capability name in `openspec/specs/`. Measured: "Change
      'bind-governance-quality-checks' is valid"; the delta is under
      `generic-gate-contract`.
- [x] 6.3 Prove the concern is wired: it appears in this repository's own gate
      run, and task 4.6 proves the same binding fails on a matching ERROR. A
      verifier that no gate runs does not gate.

      Measured: `driftwatch gate --dry-run` lists
      `governance-quality [required] via manifest`, renders
      `governance_quality: codes=[QUALITY_README_CLAIM_CONTRADICTED,
      QUALITY_README_CLAIM_UNSCOPED, RULES_DRIFT_BASELINE_STALE,
      RULES_DRIFT_NEW]`, and names it under "unbound required checks";
      `driftwatch gate --format json` reports 8 results with
      `governance-quality  UNBOUND`. Task 4.6's counterpart
      `one_matching_error_fails_the_run_and_is_earned_not_assumed` passes a
      real binding and asserts `FAIL` on a matching ERROR.
- [x] 6.4 `git diff --check` clean. Do not archive; this change is not complete
      until 5.2's dependency is recorded as still-open.
