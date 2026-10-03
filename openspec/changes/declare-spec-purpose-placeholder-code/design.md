# Design: Declare `SPEC_PURPOSE_PLACEHOLDER` in the governance-quality concern

## 1. Implementation boundary

One string in one constant, plus tests and documentation. The change is
deliberately shaped so its blast radius is provable: the list is a
compile-time constant, `default_governance_codes()` (`src/gate/manifest.rs:599`)
derives from it, and no other code in this repository enumerates the codes
individually.

| Site | Effect of the change |
|---|---|
| `src/gate/governance/mod.rs:73` `GOVERNANCE_QUALITY_DECLARED_CODES` | gains `"SPEC_PURPOSE_PLACEHOLDER"`; stays sorted, stays the closed ceiling |
| `src/gate/manifest.rs:599` `default_governance_codes()` | unchanged code; returns the 5-element default to any project that configures no scope |
| `src/gate/governance/mod.rs:292` scope validation | unchanged; the new code is now *accepted* where it was previously refused as an undeclared widening |
| `src/gate/governance/mod.rs:453` filter | unchanged; the new code is now matched rather than filtered out |
| `src/gate/governance/mod.rs:517` `level_status` | unchanged; the mapping is level-driven and already correct |

Nothing else moves. In particular the mapping, `EXPECTED_EXITS`, the exemption
rules and the failure path are untouched, because none of them is what was
broken.

## 2. Why declare ahead of emission, and what that costs

The alternative to declaring now is declaring later, when the sibling repository
ships the code. That ordering is strictly worse and is why the sibling
repository cannot simply set its flag after the fact: between the flag flipping
and this constant changing, the gate is the only thing standing between a new
ERROR code and a silent `PASS`, and during that window the fleet reports a
failure nobody is told about.

**Measured cost of declaring ahead: none to the status, and one honest gap to
coverage.** A declared code that the checker has not emitted contributes no
finding, and the status is computed from observed findings only
(`level_status(has_error, has_warning)` at `mod.rs:517`). So:

- it does **not** turn the gate `REVIEW_REQUIRED`,
- it does **not** fail CI,
- it is **not** named in the result, because there is nothing to report,
- and it does **not** change the manifest digest — verified: `sha256:61878ee79ab8617b`
  before and after, byte-identical gate output (task 1.5).

The cost is a coverage gap, not a status risk: a declared code and a code the
checker can never emit are indistinguishable from each other, and from a code
that was never declared. That gap is §5's subject, and it is not closed here.

## 3. The three tests, and why there are three

Two facts need pinning, and they are not the same fact.

1. **The gate can now see the code.** `SPEC_PURPOSE_PLACEHOLDER` at ERROR
   matches the default scope and is `FAIL`/1, with `rule` naming the code.
   Without this test the change would be an assertion nothing exercises — and
   note the baseline it fixes: before the change, that exact run read
   `PASS`/0.
2. **A declared, unemitted code is inert.** A report with no such finding over
   the full declared scope is `PASS`/0, and the code appears nowhere in the
   result. This is the empirical answer to the safety question, written down so
   a future reader does not have to re-derive it — and it is also the assertion
   that keeps fact 1 honest: without it, test 1 would be satisfied by a concern
   that fails on everything.
3. **A project may narrow to the new code alone.** `["SPEC_PURPOSE_PLACEHOLDER"]`
   used to be refused as an undeclared widening and is now the narrowest legal
   scope. This is the sibling repository's scenario "A project cannot opt in
   from its own manifest", inverted, and it is the practical consequence a
   repository enabling the rule will rely on.

The fourth change is a strengthening rather than an addition:
`reported_count_is_the_filtered_count_not_the_total` wrote `4` and `215` as
literals and would have failed on a stale number the moment a code was added. It
now derives both from `GOVERNANCE_QUALITY_DECLARED_CODES.len()`. The property it
protects is unchanged — the reported count is the filtered count, not the
checker's total — and it can no longer drift.

## 4. This repository's own gate is a null measurement, and that is expected

`.ai-gate/gate.yaml` declares no `commands:` block, so all eight required
checks record `UNBOUND` and the aggregate is `FAIL`. `governance-quality` never
reaches the adapter at runtime here, so this repository's own run **cannot**
demonstrate the change, before or after: the output is byte-for-byte identical.

That is not a gap in the change, it is the same state the concern was designed
in (`bind-governance-quality-checks` design §6, task 3.3): a template cannot know
a sibling repository's path, and binding one would make this repository's gate
depend on a sibling's on-disk layout while being the only one of eight checks
actually bound. The consequence for verification is that the before/after
measurement in `proposal.md` and task 1.4 is taken against a **bound fixture**,
which is the only place the mapping is live. The repository's own gate is
recorded as the unchanged baseline it is, and the gate manifest is not edited.

## 5. The guard that cannot be built here, and the contract change it needs

**Stated plainly: no durable guard is possible inside this repository alone.**

The weakness is real and it is the same class as the unbounded `kind` list
found in `vocabulary.json`: a manual allow-list with nothing that fails when it
falls behind the producer. The obstacle is not that the guard is hard to write,
it is that the information needed to write a correct one is not on this side of
the contract boundary.

**Why the obvious guard is broken.** A `workspace_check.py` run is a
fleet-wide audit: it emits findings for every code configured anywhere in the
workspace, not just this concern's. A rule of the form "saw a code the gate
does not declare → `REVIEW_REQUIRED`" inside the concern would fire on *every*
run, because the live audit carries at least a dozen codes outside the five
declared here (`PUBLICATION_UNVERIFIED`, `DISCOVERABILITY_MISSING`,
`LICENSE_MISSING`, `HANDOFF_SCAFFOLD`, `DEPLOYMENT_PROMOTION_STALE`,
`DISCOVERED_UNREGISTERED`, and the checker's own `VOCABULARY` coverage finding).
Those codes are the governance audit's business, not this concern's, and
`bind-governance-quality-checks` design §2 already rejected folding the portfolio
backlog into a code-scoped concern for exactly this reason. Such a rule would be
a permanently red gate, which is the same failure as a permanently green one.

**The exact contract change required.** The guard becomes writable when the
checker's envelope carries the set of codes the checker *can emit* — not the
codes it happened to emit this run. Concretely, in
`workspace-governance/scripts/workspace_check.py` (owned by the sibling
repository, currently being edited by another agent):

```python
# today, the whole --json contract:
print(json.dumps({"summary": summary,
                  "findings": [asdict(item) for item in findings]}, ...))

# required: a third top-level key, alongside summary and findings
print(json.dumps({"summary": summary,
                  "findings": [asdict(item) for item in findings],
                  "emittable_codes": sorted(ALL_REGISTERED_CHECK_CODES)}, ...))
```

`emittable_codes` must list every code registered in the checker under its
current configuration, including codes that produced no finding and codes behind
a disabled fence, and it must not be derived from the findings array — deriving
it from the findings is what would reproduce the broken guard, because a code
with zero findings is precisely the case the guard exists to catch.

With that key present, the concern computes a two-sided divergence and reports
it, and only then:

| Condition | Status | Why |
|---|---|---|
| a code the checker can emit that the concern does not declare | `REVIEW_REQUIRED` (2) | the gate cannot see a finding the checker can produce — D18 |
| a code the concern declares that the checker can no longer emit | `REVIEW_REQUIRED` (2) | the gate is watching a dead code and will never report it again |

Both directions are needed. The first is the failure this change exists to
close; the second is the one the next renamed or removed code will produce, and
it is invisible today.

**This change does not build that guard, and must not.** A guard that reads a
key the envelope does not have is either dead code or a permanent
`REVIEW_REQUIRED`; a guard scoped to the 5 declared codes while reading a
fleet-wide finding stream is the broken rule above. Both would be worse than
naming the dependency. It is recorded as cross-repo dependency **C1** in
`tasks.md` §6, against `workspace-governance`'s in-flight
`spec-purpose-backfill-gate` change.

## 6. Decision ledger

| # | Decision | Options | Choice | Rationale |
|---|---|---|---|---|
| D1 | When to declare the code | now, ahead of the checker · later, with the checker | **now** | The window between the checker shipping the code and the gate declaring it is exactly the window in which a new ERROR code gates nothing. The measured cost of declaring early is zero status change (§2). |
| D2 | Whether to build a declared-versus-emitted guard | in-repo heuristic · wait for the envelope contract · never | **wait, and name it (C1)** | The heuristic fires on every run against a fleet-wide finding stream; the contract change is owned by another repository. A half-guard is worse than none. |
| D3 | Whether the count test keeps literal counts | keep `4`/`215` · derive from the constant | **derive** | A literal count is a second manual maintenance point in the same change that exists because a manual list went stale. |
| D4 | Whether to bind a command here | bind the sibling path · leave unbound | **leave unbound** | Expected baseline, and the reason `bind-governance-quality-checks` 3.2/3.3 left it. The mapping is proven by fixtures instead. |
| D5 | How to answer the safety question | reason about it · measure it | **measure it** | Whether a declared-but-unemitted code reddens the gate decides whether this change is safe to land at all. It does not; the measurement is in `proposal.md` and pinned by test 2 in §3. |

## 7. Verification oracle

- `GOVERNANCE_QUALITY_DECLARED_CODES` contains `SPEC_PURPOSE_PLACEHOLDER`, is
  still sorted, and is still the closed ceiling — the existing
  `a_project_may_not_widen_the_scope` test is unchanged and still passes.
- A matching `SPEC_PURPOSE_PLACEHOLDER` ERROR is `FAIL`/1 with the code in
  `rule`, end to end through the CLI, not only at the adapter.
- The same binding with a report lacking the finding is `PASS`/0, and the code
  is absent from the diagnostic.
- The full suite is no worse than 869/0, and no existing test is weakened,
  skipped or deleted.
- The repository's own gate output is byte-identical before and after, including
  the manifest digest.
- `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`
  and `openspec validate --changes --strict` all clean.
