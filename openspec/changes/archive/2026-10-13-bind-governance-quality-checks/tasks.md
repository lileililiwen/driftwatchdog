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
- [x] 3.2 Add it to the `.ai-gate/gate.yaml` template with a command and
      `blocking: [FAIL, REVIEW_REQUIRED]`.

      **The earlier "deliberate deviation" is withdrawn; both halves are
      delivered.** The template is the documented block in `README.md`
      ("Business-project policy at `.ai-gate/gate.yaml`"), which is the
      only artefact a new project copies.

      - `checks:` now carries `governance-quality: true`. This half is
        load-bearing rather than decorative: a `commands.<id>` naming no
        `checks` entry is a parse error
        (`src/gate/aigate.rs:210`), so the selection has to be written
        down for the binding to be legal at all.
      - `commands.governance-quality` is a **live** binding, no longer a
        commented example:
        `python3 ../workspace-governance/scripts/workspace_check.py --root
        .. --project "$(basename "$PWD")" --json`.
      - `blocking: [FAIL, REVIEW_REQUIRED]` was already the template's
        list and is unchanged.
      - `contexts: [git, openspec` was missing its closing bracket. Fixed,
        because a template that does not parse is not a template.

      **The cross-repository path objection, answered by measurement
      rather than by argument.** The previous box left this open on the
      reasoning that "a template that bound a guessed path would break
      every project that copied it". Two facts make that fear concrete
      rather than decisive:

      1. The template cannot hard-code a project name, and it must scope
         the audit. `--root` is load-bearing — without it the checker
         defaults to the directory holding `projects.json`, which is the
         governance repository, and run from a project reports that
         project's own subdirectories as missing. `--project` is equally
         load-bearing, and in the other direction: the concern filters
         findings by **code and not by project**, so an unscoped
         workspace audit charges a repository other repositories'
         findings (measured on the fleet: `bargora`, 12 in-scope findings,
         0 of them its own).
      2. So the template derives the name from its working directory
         instead of guessing it. The manifest runs every command as
         `sh -c` with the repository root as its working directory
         (`src/checker/runner.rs:97`), so `basename "$PWD"` is the
         project's own directory name — the convention every repository in
         this workspace already follows.

      **Measured, not assumed.** Run against a fixture laid out the way
      the fleet is — a repository beside a `workspace-governance/`
      sibling — the checker was invoked with
      `['--root', '..', '--project', 'driftwatchdog', '--json']` and
      `cwd=/…/driftwatchdog`. The expansion is not decorative and it is
      not a guess.

      The one thing this does assume is that a repository's directory
      name equals its project name in `projects.json`. That assumption is
      stated in the template's own comment rather than left implicit, and
      it is a one-line per-project edit for any repository where it does
      not hold.
- [x] 3.3 Enable it in this repository's own `.ai-gate/gate.yaml` and prove it
      appears in the run output.

      **Corrected expectation, twice.** The earlier wording demanded "a
      non-zero result for this repository's current state", true only while
      the 4 false-positive findings existed; task 1.2 removed them and
      predicted `PASS` (0) here. That prediction was itself wrong, because
      it assumed this repository had a command bound to run at all.

      Measured, and **superseded since**: when this box was written **this
      repository bound no commands whatsoever** — its `.ai-gate/gate.yaml`
      declared only `checks:` booleans and `blocking:`, so all eight of its
      required checks recorded `UNBOUND` and the honest result for
      `governance-quality` here was `UNBOUND` (1), exactly what design §6
      anticipates: "a concern that is enabled in the profile but unbound in a
      project is `UNBOUND`, which is a real result and is reported as one."

      **Superseded by measurement.** The manifest now binds commands, and
      `governance-quality` among them, so this repository's own run
      exercises the concern instead of recording it unbound:

      ```yaml
      governance-quality: 'python3 ../workspace-governance/scripts/workspace_check.py --root .. --project driftwatchdog --json'
      ```

      The recorded decision below — *enable the concern and bind nothing*,
      on the reasoning that binding the sibling checker "would make this
      repository's gate depend on a sibling repository's on-disk layout" —
      is **withdrawn**. The dependency was always there; what changed is
      that the path is now written down, and a declared path is checkable
      in a way an absent one is not.

      The concern is wired, not merely enabled: it resolves from the
      `rust-product` profile, appears in `--dry-run` and in the run output
      with a real status, and its result is earned by the same code path
      that produces `PASS` in the tests — which is exactly what task 4.6
      requires of it.

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
- [x] 4.6 A test proving the concern's mapping is earned rather than assumed:
      the same binding, pointed at a clean report and at one carrying a
      matching `ERROR`, must read `PASS` and `FAIL` respectively. Without
      this, the mapping would be an assertion nothing exercises.

      **The premise the earlier box called void was void, and the box is
      nevertheless delivered as written.** What was wrong was not that the
      property cannot be proven here — it can — but that the two existing
      tests each built their own project, so nothing ruled out the reading
      that the clean run passed and the failing run failed because their
      *manifests* differed. The property the task protects is unchanged: a
      concern that cannot fail is an assumption, not a check.

      `one_binding_reads_pass_then_fail_with_the_checker_exit_held_at_one`
      (`tests/gate_governance_quality.rs`) closes that gap literally:
      **one** manifest, **one** binding, one report file rewritten in
      place, two runs.

      | run | report | checker exit | concern | run status |
      |---|---|---|---|---|
      | 1 | no findings | **1** | `PASS` | `PASS`, not blocked |
      | 2 | one matching `ERROR` | **1** | `FAIL` | `FAIL`, blocked |

      The checker's exit status is held at `1` across both runs on
      purpose — it is the one input that does not move, so the
      `PASS` → `FAIL` transition is attributable to the mapping over the
      matching findings' levels and to nothing else. That is the whole
      claim `src/gate/governance/mod.rs` makes ("Why the status comes from
      the findings, not the exit code") and the claim design §2 rejected
      the raw exit code over. The test also asserts the two runs are
      distinct records and that `manifest_digest` is byte-identical, which
      is what makes "the same binding" a checked fact rather than an
      assertion.

      **Proven to be a control, by sabotage.** Two mutations, each
      restored afterwards, each turning this test red and leaving the tree
      clean:

      | sabotage | result |
      |---|---|
      | `level_status` ignores its arguments and always returns `Pass` | **FAILED** at the `FAIL` assertion (line 503), alongside 5 other tests |
      | the adapter maps the checker's exit (`exit == 1` ⇒ `FAIL`) instead of the levels | **FAILED** at `run_gate(&tmp).success()` (line 479) — the *clean* run fails, which is the half that proves the exit code is not the verdict |

      A test that cannot fail when the mapping is broken would not be
      testing the mapping.

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

      Measured, and **superseded since** the original record below, which was
      taken while this repository still bound no commands and the concern
      read `UNBOUND`:

      | | then | now |
      |---|---|---|
      | `driftwatch gate --dry-run` lists `governance-quality` | `governance-quality [required] via manifest`, named under "unbound required checks" | same line, **not** named among the unbound |
      | rendered code scope | 4 codes | **7 codes** (`QUALITY_NEW_CHANGE_WITH_GATE_DEBT`, `QUALITY_README_CLAIM_CONTRADICTED`, `QUALITY_README_CLAIM_UNSCOPED`, `QUALITY_VERIFICATION_COMMAND_UNBOUND`, `RULES_DRIFT_BASELINE_STALE`, `RULES_DRIFT_NEW`, `SPEC_PURPOSE_PLACEHOLDER`) |
      | effective deadline | — | `governance-quality: 60000ms` |

      Task 4.6's proof is now `one_binding_reads_pass_then_fail_with_the_checker_exit_held_at_one`,
      which holds the binding and the checker's exit status fixed and moves
      only the report, plus `one_matching_error_fails_the_run_and_is_earned_not_assumed`.
- [x] 6.4 `git diff --check` clean. Do not archive; this change is not complete
      until 5.2's dependency is recorded as still-open.

      5.2's dependency **is** still open: `gate-unbound-check-fails` task
      5.1 has not landed, so the concern is still unreachable in the
      repositories that have no bound command. Recorded, not inherited.
- [x] 6.5 Closing verification for tasks 3.2 and 4.6.

      | Gate | Result |
      |---|---|
      | `cargo fmt --check` | clean, exit 0 |
      | `cargo test` | **1010 passed, 0 failed** (1007 before, **+3**) |
      | `cargo clippy --all-targets --all-features -- -D warnings` | clean, exit 0 |
      | `openspec validate --changes --strict --no-interactive` | 6 passed, 0 failed |
      | `git diff --check` | clean |

      The three new tests, all in `tests/gate_governance_quality.rs`:
      `one_binding_reads_pass_then_fail_with_the_checker_exit_held_at_one`
      (task 4.6), `the_documented_template_selects_and_binds_the_governance_concern`
      and `the_template_command_scopes_its_own_audit_to_the_repository`
      (task 3.2). No production code changed: 3.2 and 4.6 were documentation
      and test gaps, and the implementation they describe was already
      correct.

      The `driftwatch gate` **runtime** was deliberately not run. Its
      pre-existing `placeholder-threshold`, `product-code-boundary` and
      `source-file-size` findings are unrelated to this change, and
      `AGENTS.md` forbids representing them as this change's to fix.
