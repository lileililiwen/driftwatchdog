# Proposal: Declare `SPEC_PURPOSE_PLACEHOLDER` in the governance-quality concern

## Why

`workspace-governance` is adding a new ERROR code, `SPEC_PURPOSE_PLACEHOLDER`, in
its `spec-purpose-backfill-gate` change. A new ERROR code that no blocking gate
reports is a warning in all but name, which is the failure class ruling D18 names
— *a verifier that no gate runs does not gate*.

The concern that could report it already exists here. `governance-quality` runs
`workspace_check.py`, parses its JSON envelope, and filters findings through the
closed allow-list `GOVERNANCE_QUALITY_DECLARED_CODES`
(`src/gate/governance/mod.rs:73`). That list holds four codes, and
`SPEC_PURPOSE_PLACEHOLDER` is not one of them, so the finding is filtered out at
`mod.rs:453` before the status is ever computed.

This is not a theoretical gap. Measured on this repository, with a real binding
and a real report, the code that ships in the sibling repository is **invisible
to the gate today**:

```
fixture: workspace_check.py-shaped report, one finding
  {"code":"SPEC_PURPOSE_PLACEHOLDER","level":"ERROR",...}
  summary.errors = 1, checker exit status 1

BEFORE this change (4 declared codes)
  process exit      : 0
  aggregate status  : PASS
  concern status    : PASS severity: info
  concern findings  : 0
  diagnostic        : 0 of 1 governance findings matched this concern's scope
                      (0 error, 0 warning, 1 of this concern's codes outside
                      the scope or unrecognised); checker exit 1 is expected for
                      this run and does not set the status, which maps to exit 0
```

An ERROR finding, a checker that exits 1, and a gate that reports `PASS` with
exit 0. That is D18 exactly, and it is why the sibling repository cannot set
`SPEC_PURPOSE_AUDIT_ENABLED = True`: doing so would make its own CI red while
the bound gate reports success.

## What Changes

- Add `"SPEC_PURPOSE_PLACEHOLDER"` to `GOVERNANCE_QUALITY_DECLARED_CODES`. One
  constant, one line. The list stays sorted and stays the closed ceiling that
  refuses any code a project names beyond it.
- Pin, by test, both halves of what declaring ahead of emission means: the gate
  now **reports** the code once the checker emits it, and a declared code the
  checker has not emitted stays **inert** rather than becoming a verdict.
- Make `reported_count_is_the_filtered_count_not_the_total` derive its counts
  from the declared set instead of writing `4` and `215` out, so the next code
  cannot leave that test pinning a stale number.
- Record the structural weakness this change does **not** fix, and the exact
  cross-repo contract change that would fix it (design §5).

## Package Boundary

This change touches only this repository. It adds a name to a list; it does not
modify `workspace_check.py`, does not add the rule, does not enable
`SPEC_PURPOSE_AUDIT_ENABLED`, and does not touch any sibling manifest.

`SPEC_PURPOSE_PLACEHOLDER` **does not exist in the checker yet.** At the time of
writing it appears in `workspace-governance` only inside
`openspec/changes/spec-purpose-backfill-gate/` and behind
`SPEC_PURPOSE_AUDIT_ENABLED = False`. This change makes the gate *able* to see
the code; it does not make the code exist, and nothing here asserts that the
rule is gated. Enabling the rule is a separate, still-fenced step in the sibling
repository.

## No sibling manifest needs widening

The claim that no `gate.yaml` outside this repository declares
`governance_quality` is **re-verified, read-only**, at task 1.3. Measured: 77
`gate.yaml` files in the workspace, exactly one naming `governance-quality`
(this repository's own), and **zero** anywhere declaring a `codes:` block. Every
project therefore inherits the new default the moment this constant changes, and
none has a pinned list to update.

## Non-Goals

- Not adding the rule to `workspace_check.py`, and not flipping
  `SPEC_PURPOSE_AUDIT_ENABLED`. Both belong to the sibling repository's
  in-flight change and are fenced there for a reason.
- Not binding a command in this repository's `.ai-gate/gate.yaml`. It binds
  nothing today, which is the expected baseline: a template cannot know a
  sibling repository's path. The aggregate is `FAIL` with `governance-quality
  UNBOUND` and must stay that way.
- Not building a declared-versus-emitted guard. It needs a checker-envelope
  contract change owned by `workspace-governance`; design §5 names it and records
  it as a cross-repo dependency rather than shipping a half-guard that would
  either never fire or fire on every run.
- Not touching the filter, the mapping, `EXPECTED_EXITS`, or the exemption rules.
  The mapping is correct; what was missing was a name in a list.
