# Tasks: declare-spec-purpose-placeholder-code

## 1. BFS — Baseline and impact coverage

- [x] 1.1 Record the baseline before any edit. Measured on `main` at `8933b10`
      with a clean tree. **The briefing named `a9c8808` as HEAD; the tree was
      actually at `8933b10`** ("migrate(taxonomy): write resolved role/stack",
      `.project.json` only, unrelated to this change). Measured at `8933b10`:

      | Gate | Baseline |
      |---|---|
      | `cargo fmt --check` | clean, exit 0 |
      | `cargo test` | **869 passed, 0 failed** across 28 suites (623 lib + 0 bin + 246 integration) |
      | `cargo clippy --all-targets --all-features -- -D warnings` | clean, exit 0 |
      | `driftwatchdog gate --format json` | `status: FAIL`, `blocked: true`, digest `sha256:61878ee79ab8617b`, 7 unbound, `governance-quality: UNBOUND` |

- [x] 1.2 Re-verify, read-only, that the new code does not yet exist in the
      producing checker. Confirmed: `SPEC_PURPOSE_PLACEHOLDER` appears in
      `workspace-governance` only inside
      `openspec/changes/spec-purpose-backfill-gate/`, in
      `scripts/spec_purpose_backfill.py` (behind `SPEC_PURPOSE_AUDIT_ENABLED`),
      and in prose. It is not in `scripts/workspace_check.py`'s emitted codes. So
      the declared set becomes 5 while at most 4 codes can ever match — which is
      what makes task 1.4 a measurement rather than a formality.

- [x] 1.3 Re-verify the no-widening claim, read-only, across the whole
      workspace. Measured:

      | Check | Result |
      |---|---|
      | `gate.yaml` / `gate.yml` files in `/home/paul/code` | **77** |
      | files naming `governance_quality` | **1** — `driftwatchdog/.ai-gate/gate.yaml:17` (`governance-quality: true`, a boolean) |
      | files naming `governance-quality` | **1**, the same file |
      | files declaring a `codes:` block | **0** |

      No sibling manifest pins a code list, so none needs widening; every project
      inherits the new default automatically. `openpanel` declares
      `release_governance`, `agent_governance` and `governance_contract` — three
      different concerns, none of them this one. `rust-ecommerce`'s
      `openspec/governance/aspects.yaml` matched only on
      `governance-aspect-*` spec names. **No manifest outside this repository
      names codes explicitly.** Nothing outside this repository was edited.

- [x] 1.4 **Measure** what a declared-but-never-emitted code does, rather than
      reason about it. Built a bound fixture in a scratch directory — a real
      `product`-profile manifest binding a real command that prints a
      checker-shaped report and exits 1 — and ran the **4-code** binary against
      a report whose only finding is `SPEC_PURPOSE_PLACEHOLDER` at `ERROR`:

      ```
      BEFORE (4 declared codes)
        process exit      : 0
        aggregate status  : PASS
        concern status    : PASS severity: info
        concern findings  : 0
        diagnostic        : 0 of 1 governance findings matched this concern's
                            scope (0 error, 0 warning, 1 of this concern's codes
                            outside the scope or unrecognised); checker exit 1 is
                            expected for this run and does not set the status,
                            which maps to exit 0
      ```

      **Answer: a declared-but-unemitted code is inert.** It does not turn the
      gate `REVIEW_REQUIRED`, it does not fail CI, and it is not named anywhere
      in the result. The status is `level_status(has_error, has_warning)` over
      *observed* findings, and a code with no finding contributes nothing to
      either. So declaring ahead of emission is **safe**: there is no red-gate
      risk. The honest cost is coverage, not status — see task 6.2.

      The same measurement also records the problem being fixed: an ERROR
      finding, a checker exiting 1, and a gate reporting `PASS` with exit 0.

- [x] 1.5 Measure this repository's own gate before and after. The output is
      **byte-for-byte identical**, digest included:

      ```
      $ diff <before> <after>
      (no output)
      status: FAIL | blocked: true | digest: sha256:61878ee79ab8617b
      unbound: build, governance-quality, placeholder-threshold,
               product-code-boundary, repository, security, tests
      governance-quality: UNBOUND
      ```

      This is expected and is not evidence of a no-op. `.ai-gate/gate.yaml` has
      no `commands:` block, so `governance-quality` never reaches the adapter at
      runtime here and this repository's own run **cannot** measure the change in
      either direction. The digest is stable because `is_default()`
      (`src/gate/manifest.rs:594`) omits the scope from the canonical form when
      it equals the default — which the manifest does, before and after. The real
      before/after is task 2.3. **The gate manifest was not edited.**

## 2. DFS — Requirement-by-requirement implementation

- [x] 2.1 Add `"SPEC_PURPOSE_PLACEHOLDER"` to
      `GOVERNANCE_QUALITY_DECLARED_CODES` (`src/gate/governance/mod.rs:73`). One
      string. The list stays sorted (`Q` < `R` < `S`) and stays the closed
      ceiling, so the filter at `mod.rs:453` and the refusal at `mod.rs:292` are
      untouched. Extend the constant's doc comment to state the declared-versus-
      emitted distinction, because a reader comparing 5 declared codes against 4
      emitting ones will otherwise read it as a bug.

- [x] 2.2 Pin the declaration is load-bearing. Three unit tests in
      `src/gate/governance/tests.rs`:
      `a_declared_code_the_checker_has_not_emitted_is_inert` (the task 1.4
      answer, asserted so it need never be re-derived),
      `spec_purpose_placeholder_is_owed_once_the_checker_emits_it` (matching
      ERROR is `FAIL`/1 with the code in `rule`), and
      `a_project_may_narrow_its_scope_to_the_new_code_alone` (naming the new code
      in a manifest is now the narrowest legal scope, not a refused widening).

- [x] 2.3 Prove it end to end, not only at the adapter. Two tests in
      `tests/gate_governance_quality.rs`:
      `a_spec_purpose_placeholder_finding_blocks_a_bound_project` — a bound
      `product` project, the default scope, a report carrying the code at ERROR
      and exiting 1 must be `FAIL`, blocked, exit 1, `rule` naming the code; and
      `a_report_without_the_code_still_passes` — the same binding against a
      report without the finding is `PASS`, and the code is absent from the
      diagnostic. Without the second, the first would be satisfied by a concern
      that fails on anything.

- [x] 2.4 Remove the second manual maintenance point.
      `reported_count_is_the_filtered_count_not_the_total` wrote `4` and `215` as
      literals and would have failed the moment a code was added. It now derives
      both from `GOVERNANCE_QUALITY_DECLARED_CODES.len()`. The property it
      protects — the reported count is the filtered count, never the checker's
      total — is unchanged and still asserted. **Strengthened, not weakened.**

- [x] 2.5 Confirm nothing else needed moving, by measurement rather than by
      reading: `a_project_may_not_widen_the_scope` and
      `an_empty_scope_is_a_configuration_error_not_a_pass` pass **unchanged**, so
      the list is still a ceiling and an empty scope is still a configuration
      error. `EXPECTED_EXITS`, `level_status`, the exemption rules and the
      failure path were not touched.

- [x] 2.6 Update the user-facing documentation. `README.md`'s scope table gains
      the code and a paragraph stating what declaring ahead of emission does and
      does not claim.

## 3. BFS — Cross-surface regression and completeness

- [x] 3.1 Full suite, no worse than the 1.1 baseline, nothing weakened, skipped
      or deleted. Measured: **874 passed, 0 failed** across 28 suites — the 869
      baseline plus 5 new (lib 623 → **626**, `gate_governance_quality` 11 →
      **13**). `cargo fmt --check` clean; `cargo clippy --all-targets
      --all-features -- -D warnings` clean, exit 0.

- [x] 3.2 `source-file-size` is unchanged at 6 over-limit files; this change adds
      no file to `src/`. `src/gate/governance/mod.rs` is 611 lines against the
      1,000-line ceiling. The `change_workflow`, `openspec_change_names` and
      `repo_hygiene` packaging tests all exit 0 against this change's
      `tasks.md`, so the BFS/DFS/BFS phase headings satisfy the workflow check.

- [x] 3.3 The one test whose expectations the change necessarily moved was
      fixed, not deleted: `reported_count_is_the_filtered_count_not_the_total`
      (task 2.4). It still fails if the adapter ever reports the checker's total
      instead of the filtered count — proven by it still asserting the
      `211`-finding out-of-scope backlog is carried into no finding and into no
      reported rule.

- [x] 3.4 The fixture measurement repeated after the change, same fixture, same
      report, only the declared set differing — the direct before/after that
      task 1.4 could not give on its own:

      ```
      AFTER (5 declared codes)
        process exit      : 1
        aggregate status  : FAIL | blocked: True
        concern status    : FAIL severity: error
        concern findings  : 1
        rule              : SPEC_PURPOSE_PLACEHOLDER
        diagnostic        : 1 of 1 governance findings matched this concern's
                            scope (1 error, 0 warning, 0 of this concern's codes
                            outside the scope or unrecognised); checker exit 1 is
                            expected for this run and does not set the status,
                            which maps to exit 1
      ```

      `PASS`/0 with 0 findings → `FAIL`/1 with the code named. A verifier that
      no gate runs does not gate; this one now does.

- [x] 3.5 `openspec validate declare-spec-purpose-placeholder-code --strict
      --no-interactive` → "Change 'declare-spec-purpose-placeholder-code' is
      valid". The delta is under the existing capability `generic-gate-contract`.
      `openspec validate --changes --strict` also passes. (Noted: on the pinned
      CLI v1.6.0 `openspec list --changes --strict` is not a valid invocation;
      `openspec list --changes` and `openspec validate --changes --strict` are.)
      `git diff --check` clean. **Not archived**, per instruction.

## 4. Documentation

- [x] 4.1 `README.md`: the scope table and the declared-versus-emitted
      paragraph.
- [x] 4.2 `HANDOFF.md` records the change, the measurements, and the C1
      dependency — in the second commit, as instructed.

## 5. Non-goals, honoured

- [x] 5.1 `workspace_check.py` untouched, `SPEC_PURPOSE_AUDIT_ENABLED` untouched,
      no sibling repository edited. This change makes the gate *able* to see the
      code; it does not make the rule exist and does not claim the rule is gated.
- [x] 5.2 `.ai-gate/gate.yaml` not edited: no `commands:` block added, the
      aggregate stays `FAIL` with `governance-quality UNBOUND`.
- [x] 5.3 No guard built — see §6 for why, and for what is left undone.

## 6. Left undone, with reasons

- [ ] 6.1 **No durable declared-versus-emitted guard is built.** The information a
      correct guard needs is not on this side of the contract boundary: the
      checker's report carries only the findings it emitted, never the set of
      codes it *can* emit, and a `workspace_check.py` run is a fleet-wide audit
      whose finding stream includes at least a dozen codes this concern does not
      own. A "saw an undeclared code → `REVIEW_REQUIRED`" rule inside the concern
      would therefore fire on every run and be a permanently red gate — the same
      failure as a permanently green one. Left **unchecked** deliberately: it is
      not this change to build, and a half-guard is worse than none.

      **Cross-repo dependency C1**, owned by `workspace-governance`, against its
      in-flight `spec-purpose-backfill-gate` change:

      > `scripts/workspace_check.py --json` SHALL emit a third top-level key
      > alongside `summary` and `findings` — `emittable_codes` — listing every
      > code registered in the checker under its current configuration,
      > **including codes that produced no finding and codes behind a disabled
      > fence**. It SHALL NOT be derived from the findings array, because a code
      > with zero findings is exactly the case the guard exists to catch.

      Once that key exists, the concern reports a two-sided divergence as
      `REVIEW_REQUIRED` (2): a code the checker can emit that the concern does
      not declare (the gate cannot see a finding the checker can produce), and a
      code the concern declares that the checker can no longer emit (the gate is
      watching a dead code and will never report it again). Both directions are
      required; the second is what the next renamed or removed code will produce,
      and it is invisible today. design §5 has the full reasoning.
