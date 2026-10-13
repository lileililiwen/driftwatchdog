# Design: Declare `QUALITY_NEW_CHANGE_WITH_GATE_DEBT` on the governance-quality concern

## 1. The problem, measured

`workspace-governance` `0611298` added a rule that blocks a repository carrying
unresolved gate debt from opening a new OpenSpec change. It emits
`QUALITY_NEW_CHANGE_WITH_GATE_DEBT` at ERROR and the checker exits 1.

`src/gate/governance/mod.rs` filters the producing checker's findings to
`GOVERNANCE_QUALITY_DECLARED_CODES` before forming a verdict. A code outside that
set is dropped: not reported, not counted, and it cannot change the status.

The false green was reproduced before anything was changed. On a fixture where
the checker emitted the code and exited 1:

| | concern status | evidence line | exit |
|---|---|---|---|
| code undeclared | `PASS` | "0 of 93 governance findings matched this concern's scope" | 0 |
| code declared | `FAIL` | "1 of 93", rule named | 1 |

So shipping the rule without the declaration would have put a blocking rule into
the workspace that no gate reads. It would have reported clean.

## 2. Why this is not a new class of problem

`QUALITY_VERIFICATION_COMMAND_UNBOUND` is already declared for exactly this
reason, and the doc comment on the constant says so. The declaration is the
mechanism; this change adds one more entry to it.

What is new is the blast radius. The earlier codes fire on at most a handful of
repositories. This one fires on **every** legacy repository that opens a change
while carrying debt — 22 repositories at the time of writing. An undeclared code
here would have been a fleet-wide false green, not a per-project one.

## 3. Decision

Add `QUALITY_NEW_CHANGE_WITH_GATE_DEBT` to the declared set, and write down why
beside the constant rather than leaving the next reader to infer it.

Rejected alternatives:

- **Leave it undeclared and accept a report line.** No. A rule that blocks but
  is never read is a rule whose author believes the fleet is protected when it
  is not. This is the exact failure the constant's own doc comment warns about.
- **Widen the concern to report every code.** That would make the code set
  meaningless and would make `REVIEW_REQUIRED`-on-widening unenforceable, since
  there would be nothing left to widen into.
- **Exempt legacy repositories from the concern.** That is a per-project scope
  decision, already supported by the narrowing rule, and it is the wrong lever
  here: the problem is the code is invisible, not that it is too loud.

## 4. The rationale must be stated, not implied

The new requirement "A declared code states why it is declared" exists because
an unstated declaration cannot be audited. A reader who finds a code in the set
has no way to tell an intentional exclusion from an oversight, and the cheapest
way for a future change to reintroduce a false green is to add a code quietly.
The rationale is prose beside the constant, in the same place as the existing
one.

## 5. Blast radius

One line in one array, and one test. No behaviour outside the declared-code
lookup changes:

- narrowing still works, because narrowing filters the declared set downward;
- widening is still refused, because the refusal test is against the declared
  set;
- no check is skipped, no test is weakened, no exemption is introduced.

## 6. Verification

- `cargo test` — the full suite, counting passes and failures. Baseline before
  this change: 908 passed, 0 failed, 29 suites. This change adds one test.
- The new test asserts `FAIL`, exit 1, one finding, severity `Error`, and the
  rule name on the finding — so it fails if the declaration is removed, which is
  the regression this whole change exists to prevent.
