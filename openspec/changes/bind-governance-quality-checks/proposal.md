# Proposal: Bind the governance quality checks as real Gate concerns

## Why

Two governance checks shipped in `workspace-governance` and currently gate
nothing at all. A verifier that no gate runs does not gate (ruling D18 in
`dotnet-platform-libs/openspec/changes/ui-shared-layer-reachability/design.md`),
so both are currently documentation.

- `QUALITY_README_CLAIM_CONTRADICTED` (ERROR) and `QUALITY_README_CLAIM_UNSCOPED`
  (WARNING), commit `f8df5ea`. Today: 4 ERROR findings across 4 repositories, and
  exactly 1 aggregate WARNING naming 3 statements.
- `RULES_DRIFT_NEW` (ERROR) and `RULES_DRIFT_BASELINE_STALE` (WARNING), commit
  `4a9edfc`, a ratchet over `rules-drift-baseline.json` (140 baselined keys across
  72 repositories). Today: 0 of each.

Two measured facts make this more than a configuration edit.

**The exit codes do not line up.** `workspace_check.py` returns 0 when clean and
1 when it has errors. It never returns 2. This repository's contract
(`src/gate/adapters.rs`, stated in its own remediation string) is `PASS=0`,
`FAIL=1`, `REVIEW_REQUIRED=2`, `NOT_APPLICABLE=0`, `UNBOUND=1`. A run whose only
findings are WARNINGs exits 0, so a direct binding would read `PASS` and the
entire REVIEW_REQUIRED class would be invisible — which is precisely the
"checker that reports OK without checking" failure this workspace keeps finding.

**Binding the raw command would redden the fleet.** The live audit is
`errors=215 warnings=18`, and almost none of it is these two checks:
`PUBLICATION_UNVERIFIED` 76, `DISCOVERABILITY_MISSING` 74, `LICENSE_MISSING` 51,
`HANDOFF_SCAFFOLD` 8, `DEPLOYMENT_PROMOTION_STALE` 7, `DISCOVERED_UNREGISTERED` 4.
Only **4** of the 215 are `QUALITY_README_CLAIM_CONTRADICTED`. A binding scoped to
the checker's exit code would surface all 215.

## What Changes

- Add a `governance-quality` Gate concern backed by a **code-scoped adapter**: it
  runs the governance checker, filters its findings to a configured code set, and
  emits this repository's envelope.
- Map findings onto the envelope by level: 0 matching findings → `PASS` (0);
  matching WARNINGs → `REVIEW_REQUIRED` (2); matching ERRORs → `FAIL` (1).
- Define the failure path for a checker that cannot run at all. None of those
  cases may be `PASS`.
- Add the concern to the built-in profiles and the `.ai-gate` template, and state
  the ordering constraint against `gate-unbound-check-fails` task 5.1.

## Package Boundary

This change touches only this repository. It binds checks that live in
`workspace-governance`; it does not modify them, and it does not re-implement
either check. The adapter reads the governance checker's `--json` output and
nothing else.

## Non-Goals

- Not changing `workspace_check.py`, including its exit-code semantics. Four
  repositories' own gates depend on the current behaviour.
- Not making either check non-ERROR. A finding that is ERROR-severity and blocks
  nowhere is a warning in all but name.
- Not exempting either concern. See decision D3.
- Not binding the checks in the 70 repositories that still have no bound command.
  That is `gate-unbound-check-fails` task 5.1, and this change states the
  dependency rather than silently inheriting it.
- Not rewriting any README, `.ai-rules` file, or baseline to make a gate green.
