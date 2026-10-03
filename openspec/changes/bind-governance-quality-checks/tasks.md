## 1. Boundary

- [ ] 1.1 Record the baseline before any edit: `cargo fmt --check`, `cargo test`,
      `cargo clippy --all-targets --all-features -- -D warnings`, and
      `openspec validate --changes --strict --no-interactive`. Report the exact
      numbers; the change is not acceptable if any gate is worse afterwards.

## 2. The adapter

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

## 3. Registration

- [ ] 3.1 Add `governance-quality` to the built-in `product` and `rust-product`
      profiles, alongside `repository`, `build`, `tests` and `security`.
- [ ] 3.2 Add it to the `.ai-gate/gate.yaml` template with a command and
      `blocking: [FAIL, REVIEW_REQUIRED]`.
- [ ] 3.3 Enable it in this repository's own `.ai-gate/gate.yaml` and prove it
      appears in the run output with a non-zero result for this repository's
      current state.

## 4. Tests

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

## 5. Documentation and the deferred dependency

- [ ] 5.1 Update the Gate concern documentation and the `.ai-gate` reference with
      the envelope mapping, the failure path, and the scope-versus-exemption
      distinction.
- [ ] 5.2 State the ordering constraint in the docs: the concern reaches the 70
      repositories that still have no bound command only when
      `gate-unbound-check-fails` task 5.1 lands. Until then it is live here and
      `UNBOUND` elsewhere. Do not claim fleet coverage this change does not have.
- [ ] 5.3 Record the blast radius in the handoff: 4 of 215 errors and 1 of 18
      warnings are surfaced, and 211 errors are deliberately left to the
      governance audit alone.

## 6. Verification

- [ ] 6.1 `cargo fmt --check && cargo test && cargo clippy --all-targets
      --all-features -- -D warnings && openspec validate --changes --strict
      --no-interactive`, all no worse than the §1.1 baseline.
- [ ] 6.2 `openspec validate bind-governance-quality-checks --strict
      --no-interactive` passes, and the delta is filed under an existing
      capability name in `openspec/specs/`.
- [ ] 6.3 Prove the concern is wired: it appears in this repository's own gate
      run. A verifier that no gate runs does not gate.
- [ ] 6.4 `git diff --check` clean. Do not archive; this change is not complete
      until 5.2's dependency is recorded as still-open.
