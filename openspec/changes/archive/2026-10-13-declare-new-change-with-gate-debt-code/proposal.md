# Proposal: Declare `QUALITY_NEW_CHANGE_WITH_GATE_DEBT` on the governance-quality concern

## Why

`workspace-governance` commit `0611298` added a rule that blocks a repository
carrying unresolved gate debt from opening a new OpenSpec change. The rule
emits `QUALITY_NEW_CHANGE_WITH_GATE_DEBT` at ERROR and exits 1 when it fires.

The `governance-quality` concern filters the producing checker's findings down
to the codes it declares in `GOVERNANCE_QUALITY_DECLARED_CODES`. A code that is
not in that set is not reported and not counted. So an undeclared code leaves
the concern reading `PASS` — the exact "checker that reports OK without
checking" failure this workspace keeps finding.

**The false green was reproduced, not reasoned about.** On a fixture where the
producing checker emitted the code and exited 1, the concern reported
`PASS`, "0 of 93 governance findings matched this concern's scope", exit 0.
With the code declared it reports `FAIL`, "1 of 93", the rule named, exit 1.
That flip is this change.

This is not a new hazard. `QUALITY_VERIFICATION_COMMAND_UNBOUND` is declared for
exactly the same reason. What is new is that this workspace now has a code that
**every** legacy repository would emit, so the gap would have been hit fleet-wide
rather than on one project.

## What this changes

One constant and one test.

- `src/gate/governance/mod.rs` — add `QUALITY_NEW_CHANGE_WITH_GATE_DEBT` to
  `GOVERNANCE_QUALITY_DECLARED_CODES`, with the reason written down beside the
  existing declaration rather than left to the next reader.
- `src/gate/governance/tests.rs` — a test that the code produces a `FAIL` gate
  result, not a report line.

## What this does not change

No behaviour. A project that already narrows its scope to a subset still
reports only its subset. A project that tries to widen past the declared set is
still refused as `REVIEW_REQUIRED`. No exemption is added, no check is skipped,
and no test is weakened.

## Scope note

This repository holds the declaration only. The rule, the ratchet, the
exceptions record and the generator live in `workspace-governance` `0611298`.
A declaration without the rule is inert; a rule without the declaration is a
false green. Both halves are required, which is why this change is small.
