# Design: A required Gate check with no bound command fails closed

## 1. Implementation boundary

Repository `/home/paul/code/driftwatchdog`, Rust 2021.

- **Extension points** are exactly: `src/gate/types.rs` (one new
  `GateStatus` variant, one `as_str`/`from_str` arm, one severity mapping),
  `src/gate/commands` consumers of that enum, `src/gate/aggregate.rs` (the
  aggregation arm for the new status and the new `unbound` outcome vector),
  `src/commands/gate.rs:228-243` (the `None` arm of the command match, which
  is the only site that produces the new result), `src/gate/aigate.rs`
  (the `not_applicable` and `binding_policy` parse plus the required-binding
  check beside the existing `commands` cross-reference at lines 159-180),
  `src/gate/manifest.rs` (the `validate` rule, the two new
  `ManifestError` variants, and the `not_applicable`/`binding_policy`
  fields on `GateManifest`), the exemption budget computed beside
  `unbound_required_checks` in the same pure resolve pass (§3.5), and the
  render/persistence sites in `src/commands/gate.rs` that print and store
  results.
- **No new module, no new table, no migration, no new dependency.** The
  result is carried inside the existing `gate_runs.results_json` string and
  the new `unbound` vector on `AggregateOutcome`.
- **No new execution surface.** The new status is produced only on the
  branch taken when a required check has no command, so it cannot make the
  runtime spawn anything. The `sh -c` project-runtime adapter contract,
  its timeout, output cap, process-group kill, and secret redaction are
  untouched.
- **One delta is deliberately out of this package's single spec file:**
  `generic-gate-contract`'s `Normalize gate results` and
  `Aggregate gate results` requirements describe the status vocabulary and
  the aggregation that this change alters. That companion delta is task
  4.2 and is required before the change may be archived; it is not merged
  into this package's `gate-project-configuration` delta because OpenSpec
  applies a delta to the capability its file is named after.

## 2. Where the gap actually is

This section corrects the framing the change was briefed with, because the
correction changes the design.

**What already works.** `src/gate/aggregate.rs:95-101` pushes a *required*
`NOT_APPLICABLE` into `pending_reviews`, and `aggregate.rs:132-140` blocks
on pending reviews when `review_required_blocks` is set. All 76 workspace
manifests set `blocking: [FAIL, REVIEW_REQUIRED]`, so
`aigate.rs:198` sets `review_required_blocks = true`. On today's code a
required unbound check therefore already blocks the aggregate and
`driftwatch gate` already exits 1. "Make the unbound check fail" is not
sufficient as a description of the defect, and a spec written on that
description would change nothing.

**What is actually broken**, in three separable parts:

1. **The status is overloaded.** `NOT_APPLICABLE` is the correct terminal
   verdict for a command that ran and reported "not applicable"
   (`adapters.rs:1143` maps it to exit code 0 and admits it into the
   result), and it is also what `commands/gate.rs:228-243` records when no
   command exists and nothing ran. A proven exemption and an unproven
   absence are indistinguishable in the record. This is the core defect and
   it is invisible to any consumer that reads results rather than the
   aggregate.
2. **The result record is non-failing.** The unbound record is
   `NotApplicable` + `GateSeverity::Info` + zero findings + empty evidence.
   The persisted `gate_runs.results_json`, the human render, the JSON
   render, and the evidence export all carry that record. `FAIL` is
   reserved for a command that ran and failed, so there is no status that
   means "you asserted this and never ran it".
3. **The ruling is delegated to a project-controlled toggle.** Because the
   unbound case reaches the aggregate as a *review*, whether an unexecuted
   mandatory assertion blocks is decided by the `blocking:` list, which the
   project edits. A manifest with `blocking: [FAIL]` and a required,
   unbound `tests` aggregates to `PASS` and persists a green run. The
   central Gate's honesty is therefore opt-out, which is the opposite of
   fail-closed. One workspace manifest already declines to list
   `REVIEW_REQUIRED`.

The design fixes (1) with a distinct status, (2) with `Error` severity and
a named remediation, and (3) by making the unbound case block
unconditionally.

## 3. Behavioral model

### 3.1 Status vocabulary

Add `GateStatus::Unbound`, serialising as `"UNBOUND"`, with
`GateSeverity::Error`. `from_str` accepts it so persisted and exported
payloads round-trip. `UNBOUND` is produced in exactly one place: the `None`
arm of the command match in `commands/gate.rs`, now gated on `required`.

Why a new status rather than reusing `FAIL`: reusing `FAIL` would collapse
"the gate ran and rejected your code" with "the gate never ran", which is
the precise confusion this change exists to remove, and it would also make
the remediation text lie. Why not a new *aggregate* status: the aggregate
vocabulary is deliberately three-valued and every consumer keys on
`blocked`, so a fourth aggregate status would break `gate_runs.status`
consumers for no gain. A required `UNBOUND` aggregates to `FAIL`; the
distinction survives in the per-result record and in the new `unbound`
vector.

### 3.2 The four-way check state

| State | Trigger | Result | Blocks when required |
|---|---|---|---|
| Bound | `commands.<id>` non-empty | command runs; status as reported | as reported |
| In-process | id is an in-process concern (`source-file-size` today) | scanner runs | as reported |
| Unbound-required | required, no command, no exemption | `UNBOUND`, `Error` | **always** |
| Unbound-optional | `optional`, no command | `UNBOUND`, `Warning` | no, still listed |

A check selected `false` is deselected and produces no result and no
`UNBOUND`. The in-process row is a first-class exception, not an oversight:
`source-file-size` is deliberately command-free and 41 repos inherit it
through the `product`/`rust-product` profiles, so failing it closed would
break the one concern that actually runs.

### 3.3 Manifest vocabulary

Two new `.ai-gate/gate.yaml` fields, both additive and both optional:

```yaml
not_applicable:            # concern id -> non-empty reason string
  accessibility: "CLI with no UI surface; no a11y target exists"
binding_policy: report     # "report" (default) | "reject"
```

`not_applicable` is the honest home for a check that genuinely has no
implementation on the project's stack. It is the answer for
`accessibility` (4 repos, all unbound), `format` (3), `lint` (2), and for
the 34 single-repo check kinds. It carries a mandatory non-empty reason
because an unexplained absence is the defect being fixed; the reason is
bounded in length and passes through the existing `gate::redact`
credential redaction before it is persisted or rendered, since it is
project-authored free text on a durable record.

`binding_policy` selects where the rule is enforced:

- `report` (default) — a required unbound check produces `UNBOUND` and
  blocks the run. The manifest still parses, the gate still executes, and
  a `gate_runs` row is still persisted.
- `reject` — the manifest is refused during parse/resolve, before any
  check executes, with the offending check id and the remediation.

The default is `report` because rejecting at parse time produces **no run
row at all**, which destroys the very inventory the migration needs. That
is the honest reason for the default, and it is also the reason the
default is temporary — see §6 and the open decisions in §8.

### 3.4 Aggregation

`aggregate` gains an arm for the new status: required → `failures`
(aggregate `FAIL`, `blocked: true`); optional → recorded in the new
`unbound` vector with an `optional:` marker, never blocking. A required
`UNBOUND` bypasses the `blocking:` list entirely. The `not_applicable`
vector keeps its meaning for proven exemptions and declared
`not_applicable` entries; `unbound` is a separate vector so the two are
never merged. `AggregateOutcome` gains one field; `gate_runs.status` keeps
its existing PASS/FAIL/REVIEW_REQUIRED domain.

### 3.5 Exemption budget (owner decision, 2026-10-02)

**The gap.** The `not_applicable` vocabulary in §3.3 gives a repo a lawful way
to say "this gate does not exist on my stack" — for every one of its checks if
it likes. Nothing bounded that, so the three lawful outcomes the proposal
enumerates collapse in practice: a repo can bind nothing, exempt everything,
and hold a green run. The proposal made capping a non-goal and §8 open decision
4 asked the owner whether the workspace would set a ceiling. The owner
answered on **2026-10-02**. This section records the answer as a decision, not
an accident; the next reader should be able to see who ruled and why.

**The decision, as given.**

- A repo may exempt at most **`min(floor(0.25 × required_checks), 5)`**
  required checks. The ruling's premise was that repos declare roughly
  10–25 checks, so the rule would yield a hole of 2–5 checks. **That premise
  was measured wrong and is corrected below** — the formula is unchanged,
  because the owner decided it and revising it would be a second decision.
- **`build`, `tests`, `security` and `repository` are never exempable.** An
  exemption naming any of these four is a hard `FAIL`, regardless of the
  numeric cap. They are the floor every other check stands on. The
  repository-integrity concern is also protected under its other workspace
  spelling, `repository_integrity` — see "The spine, and the alias it
  needs" below.
- **At the cap** the run records a `WARN` whose diagnostic says the headroom is
  exhausted. **Over the cap** the run is a blocking `FAIL`.
- A repo whose stack genuinely needs more must be **reviewed**, not
  self-authorised. The exemption `reason` is required text, not a free pass.
- The cap is computed **per repository from its own manifest**, never
  hard-coded globally.

**The spine, and the alias it needs.** The ruling names four check *ids*.
Three of them are single-spelling in this workspace — `build` (75
manifests), `tests` (75), `security` (75). The fourth is not:
repository-integrity is spelled **`repository`** in 63 manifests and
**`repository_integrity`** in 2 (`labrys` and `openpanel`), and a fleet scan
found no third spelling. Implemented as four literal ids, the rule left those
two repos able to exempt the concern the spine exists to protect under the
other name — a hole that is **latent rather than live today**, because both
of them bind every required check (`repository_integrity: "git diff
--check"`, zero required-unbound) and exempting a bound check is already a
parse-time `ExemptionConflictsWithCommand` error. It is fixed anyway: a rule
that is safe only because of a coincidence about today's manifests is not a
rule, and the first manifest to declare the alias selected-but-unbound would
have opened it. The fix is an explicit alias table rather than a longer list:
the runtime matches every spelling of a non-exempable concern, keyed by the
concern, so a future spelling is added as a visible decision against the
concern it names instead of being an id someone appends. Diagnostics and the
plan render keep naming the four concerns, so an operator reads a concern
rather than a spelling.

**Corrected distribution: what the evidence now shows.** A read-only
`gate --dry-run` pass over all 76 real manifests (recorded in `tasks.md`
group 7) measures required-check counts of min 3, **median 7**, max 29, not
the 10–25 assumed. The resulting allowances are therefore:

| Allowance | Repos | Which |
|---|---|---|
| 0 | 2 | `kairosa`, `retakeKit` (3 required checks each) |
| 1 | 60 | the majority (38 repos declare 7, 16 declare 4) |
| 2 | 2 | `labrys` (8), `vowra` (10) |
| 5 | 1 | `openpanel` (29) |

So the hole is **1 for 60 of 65** parseable repos and **0 for 2**, not 2–5.
Two consequences follow, and the second is the important one:

1. The cap is a real but *secondary* bound. With a median of 7, it limits the
   non-spine remainder to roughly one check; it is not the mechanism that
   prevents a repo from emptying its gate.
2. **The spine is what actually closes the escape**, and the measurement
   proves it: all 59 repos carrying required-unbound debt have spine debt
   (unbound `build`, `tests`, `security`, or a repository spelling), and
   **0** repos can reach a green run by exempting their debt. A typical repo
   — 7 required, 6 unbound, 4 of them spine — can spend its single allowance
   on one non-spine check and still ends with 5 red checks. Remove the spine
   and the same repo would be green after two exemptions; keep the spine and
   the cap's exact number barely matters.

This makes the alias defect above more consequential, not less: the spine is
carrying the rule, so a single unaliased spelling would have reopened the
escape for the two repos that use it. It is also a stronger justification for
the spine list than "it is the floor every other check stands on" — the
measurement is the argument.

**Why a percentage is not enough on its own, and what pairs with it.** A
percentage alone fails in both directions. Unbounded on a large check list: a
repo declaring 40 required checks would earn a 10-check hole under 0.25, which
is a quarter of the gate gone. Impossible on a small one: floor(0.25 × 3) is
zero, so a small repo could not exempt anything at all and the percentage
would silently become a total ban rather than a budget. The **paired absolute
cap of 5** bounds the large end so the hole can never scale with the manifest,
and the **non-negotiable spine** (`build`, `tests`, `security`, `repository`
and its `repository_integrity` alias) guarantees the floor at both ends
regardless of what the arithmetic yields. The two together are what make the
rule safe: the number can be generous, and the floor cannot be bought. On the
measured fleet the spine does most of that work, for the reason given above.

**Counting rules the formula depends on.**

- `required_checks` is the count of **required** checks the manifest selects:
  profile defaults (built-in or project-defined) with explicit `[[checks]]`
  applied as overrides, resolved by the same `selected_without_triggers` the
  `unbound_required_checks` rule uses. Changed-surface triggers do **not**
  move the cap, for the reason already recorded beside that function: a
  manifest-level governance rule must not depend on the diff being gated.
  Counting only required checks also stops a repo from inflating its own
  budget by declaring a pile of optional checks.
- **Every** declared `not_applicable` entry consumes budget, including one on
  an optional check. Otherwise `required = false` becomes a one-character cap
  bypass, which is precisely the "self-authorised absence" the decision
  exists to close.
- Small-manifest consequence, stated rather than hidden: a project selecting
  fewer than four required checks gets a cap of **zero**, so it may not exempt
  anything and must bind a command or drop the check. That is the literal
  formula, it is intentional, and it is the reason the spine matters — the
  small repo is not left with a gate that can be emptied.

**Where the verdict is produced.** Resolution time, in the same pure pass that
already computes `unbound` and `exemptions`, so `--dry-run` names the budget
before anything executes and no new execution surface is introduced. The
resolved budget is carried on `ResolvedGatePlan::exemptions_budget` and
materialised at execution as one plan-level result under the id
`exemption-budget`, appended to the effective plan the way the optional
`ai-review` check already is. That choice is deliberate: reusing existing
statuses (`REVIEW_REQUIRED` + `Warning` for the at-cap warning, `FAIL` +
`Error` for both failure conditions) means the aggregate, the persisted
`gate_runs.results_json`, the human render, the JSON render, and the evidence
export carry the verdict with **no new status, no new table, and no new
consumer to keep in step**. A per-check `FAIL` is *also* recorded for each
non-exempable concern that was exempted, so no surface ever shows an
acknowledged exemption for a check that may not be exempted; the plan-level
result is the one that blocks, which keeps the verdict independent of whether
the exempted concern was marked `required`.

**Not in this decision.** The alias question *was* decided on 2026-10-02 as a
correction to the same ruling: `repository_integrity` is protected as a
spelling of the repository-integrity check, not as a fifth concern, because
the ruling's intent — the concern may never be exempted — is unaffected by how
a manifest spells it and the alternative reading would reopen the escape for
`labrys` and `openpanel`. What remains undecided: whether the spine extends to
other concern families, whether a *new* spelling introduced by a future
manifest is protected by default or must be added to the alias table
deliberately (this change takes the latter: a rename is a reviewable edit, not
an automatic widening), and whether a repo that genuinely needs more than the
cap gets a per-repo override — an override would reintroduce exactly the
self-authorisation the cap removes, so it must be a reviewed workspace change,
not a manifest key.

## 4. Contract and compatibility

**Manifest schema (additive, `version: 1` unchanged).** Two new optional
keys. Unknown-key handling is unchanged, so both keys must be added to
`YAML_FIELDS` in `aigate.rs` and to the native `gate.toml` field list or
they will be rejected as unknown fields — the schema addition and the
validator addition must land together.

**Result contract.** `GateStatus` gains one variant. Consumers that match
the enum exhaustively — the human render, the JSON render, the evidence
export, the MCP read tools, and `adapters.rs:1143-1152`'s status-to-exit
and status-to-severity tables — must gain an arm in the same change. An
unmapped status in the exit-code table would be a silent behaviour
regression for a bound command that ever reports it, so the tables are in
scope, not a follow-up.

**Measured compatibility surface.** The 6 binding repositories were checked
individually and every one of them has zero required checks left unbound:

| Repository | Bound commands | Required checks left unbound |
|---|---|---|
| `byok-translator` | 5 | none |
| `careBridge` | 5 | none |
| `labrys` | 8 | none |
| `openpanel` | 29 | none |
| `therapist-commons` | 5 | none |
| `vowra` | 10 | none |

None of the 6 changes status and none needs a manifest edit. The
heavy-shell command strings in `therapist-commons` and `vowra` are
unaffected: they bind a command, so they take the execution path exactly as
before.

**Unchanged by design.** `driftwatch check`, the checker protocol, the AI
review path and its missing-provider policy, the `blocking:` list's
meaning for `REVIEW_REQUIRED`, the foreign-runtime no-op, native
`gate.toml` precedence over `.ai-gate/gate.yaml`, and the `commands`
key-must-reference-a-selected-check rule.

## 5. Failure and boundary policy

- Required + no command + no exemption → `UNBOUND`/`Error`, aggregate
  `FAIL`, `blocked: true`, nonzero exit, regardless of `blocking:`.
- Required + empty-string command → unchanged parse-time
  `ManifestError::EmptyCheckCommand`; this change adds the *absent* case
  and must not merge the two.
- `commands.<id>` naming an unselected check → unchanged parse-time error.
- `not_applicable` naming a check that is not selected, or carrying an
  empty/whitespace reason → parse-time error naming the key.
- `not_applicable` naming a check that *is* bound → parse-time conflict
  error; a project may not simultaneously claim a command and an
  exemption.
- `not_applicable` reason exceeding the bound, or containing a
  credential-shaped string → truncated with a marker and redacted before
  persistence, never silently stored raw.
- Declared exemptions **at** `min(floor(0.25 × required_checks), 5)` → a
  `REVIEW_REQUIRED`/`Warning` `exemption-budget` result on an optional plan
  check: recorded, visible, and never blocking.
- Declared exemptions **over** that limit → a `FAIL`/`Error`
  `exemption-budget` result on a required plan check: aggregate `FAIL`,
  `blocked: true`, nonzero exit, independent of `blocking:`. The diagnostic
  names the declared count, the limit, and the required-check count.
- `not_applicable` naming `build`, `tests`, `security`, `repository`, or a
  workspace alias of one of them (`repository_integrity`) → hard `FAIL`
  whatever the count, plus a per-check `FAIL` for that concern so no surface
  records it as an acknowledged exemption. The exempted check's
  own `required` flag is irrelevant: the plan-level budget check is what
  blocks.
- Exceeding the limit is a **verdict, not a refusal**: the manifest still
  parses, the gate still executes, and a `gate_runs` row is still written, so
  the over-cap state is queryable exactly like the unbound state. A
  `binding_policy: reject` manifest with an over-cap budget still resolves for
  the same reason; the refusal path stays reserved for unbound checks.
- A project with no `not_applicable` entries → no budget result at all; the
  cap adds no noise to a project that has not used the feature.
- Unknown or invented check kind (the 34 singletons) → no id validation in
  this change, but the fail-closed rule applies identically, so it produces
  `UNBOUND` rather than being ignored. A repo cannot invent a check name to
  escape the ruling.
- `binding_policy: reject` → no `gate_runs` row is written; the refusal
  names every offending check id at once rather than one per invocation.
- Unknown `binding_policy` value → parse-time error listing the accepted
  set, matching the existing unknown-`blocking`-status behaviour.
- Foreign runtime → unchanged: nothing executed, no run persisted, exit
  zero.
- `--dry-run` → unchanged and still pure; the plan render names each
  unbound required check and its remediation so an operator can audit
  before executing, and now also names the exemption budget
  (declared / limit / required count / non-exempable ids) without executing
  anything.

## 6. Migration and rollout

The runtime semantics and the fleet migration are separate packages. This
package owns phases 0 and 2.

- **Phase 0 — shadow (this package).** The new status ships and is
  recorded in the run row, but the *blocking* switch for it is not yet
  flipped for the fleet. One full workspace cycle produces the exact
  inventory of repos and checks that would go red, from persisted run rows
  rather than from exit codes. Nothing is red yet; every unbound required
  check is named.
- **Phase 1 — fleet migration (not this package).** Each repository binds
  a real command, declares `not_applicable` with a reason, or moves the
  check to `optional`. This is 70 language-specific changes and is where
  the review effort belongs. Measured starting debt: 68 unbound `tests`,
  69 unbound `security`, 66 unbound `build`, 61 unbound `repository`, plus
  41 repos inheriting unbound `product-code-boundary` and
  `placeholder-threshold` from the built-in product profile.
- **Phase 2 — enforcement (this package).** Once the inventory is reviewed
  and the migration has had a window to land, the new status becomes
  blocking by default.
- **Phase 3 — parse-time default (owner decision).** After the fleet is
  green, `binding_policy` defaults to `reject`. This is the only point at
  which parse-time rejection is safe to default to, because before then it
  would turn every unbound manifest into a config error with no run record.

An unreviewable flood is avoided by the ordering, not by softening the
rule: phase 0 makes the whole debt visible in one artifact before any repo
is allowed to fail, and phase 1 is a separate reviewable change per repo.

## 7. Verification oracle

- **Parse fixtures** under the existing fixture convention: a required
  check with a command; a required check with no command; an optional
  check with no command; a disabled check with no command; the in-process
  `source-file-size` check with no command; an exemption with a reason; an
  exemption with an empty reason; an exemption on a bound check; an
  exemption on an unselected check; an unknown `binding_policy` value.
- **Unit tests** on the status vocabulary: `as_str`/`from_str` round-trip
  for the new variant, the exit-code and severity table arms, and the
  unchanged mappings for the four existing variants.
- **Aggregation tests**: a required unbound check blocks with
  `blocking: [FAIL, REVIEW_REQUIRED]`, with `blocking: [FAIL]`, and with
  no `blocking:` key at all — the third and second are the actual fail-open
  regression tests; an optional unbound check does not block but is still
  listed; an unbound check never merges into `not_applicable`; a
  single-result bound `PASS` still aggregates to `PASS`.
- **Run-persistence test**: the `unbound` vector and the per-result record
  survive into `gate_runs.results_json` and into the JSON render, so the
  debt is queryable without re-running the gate.
- **Redaction test**: an exemption reason carrying a credential-shaped
  string is redacted and a bounded one is truncated with a marker before
  persistence.
- **Compatibility replay**: each of the 6 binding repositories' manifests
  is parsed and resolved and produces the same plan and the same result
  set as today, with no manifest edit.
- **Fleet inventory**: one read-only pass over the 76 real manifests that
  records, per repo, every required check that would be `UNBOUND`, and
  that total is compared against the measured debt in §6. This is the
  acceptance oracle for the blast radius and it is a report, not a test
  that changes state.
- **Cap arithmetic**: the formula is pinned at its boundaries with real
  numbers — 10 required → 2, 20 → 5, 24 → 5 (the absolute cap binds), 40 → 5
  — plus the two ends that matter for this fleet: 3 required → 0 and 0
  declared → no budget result. The percentage-alone failure mode is proved by
  the 40-check case and the absolute cap is proved by the 24-check case.
- **Cap behaviour, end to end**: an exemption set below the limit produces no
  budget result; exactly at the limit produces the `Warning`
  `exemption-budget` result and a non-blocked run; over the limit produces the
  blocking `FAIL`; exempting each of `build`, `tests`, `security`,
  `repository` fails whatever the count; a small manifest (cap 0) fails on any
  exemption; a large manifest (cap 5) is no wider than a 20-check one; and a
  manifest with no exemptions is byte-identical to its pre-cap behaviour.
- **Fleet impact of the cap**: measured against the real manifests, reporting
  what the cap changes *beyond* the unbound rule — principally, how far each
  repo's declared debt sits above its own allowance, i.e. that "exempt
  everything and go green" is no longer an available outcome. Reported, not
  suppressed: no `gate.yaml` in the fleet is edited to make this pass.

No test may assert that a check *passed*; the only success condition this
change introduces is a truthful red.

## 8. Decision ledger

**Resolved.** The new status is a distinct `UNBOUND` rather than a reuse
of `FAIL`, because the two claims are opposite and the operator's
remediation differs. The aggregate vocabulary stays three-valued and
`UNBOUND` aggregates to `FAIL`, because `blocked` is the machine signal
every consumer already keys on. A required unbound check blocks
unconditionally and is no longer governed by the project-controlled
`blocking:` list, because that toggle is the fail-open. `required` is
**not** a new concept — `ManifestCheck.required` / `PlannedCheck.required`
already carry the `true` / `"optional"` / `false` selection from
`aigate.rs`, and the change gives that existing axis teeth rather than
inventing a parallel one. `source-file-size` stays exempt as an
in-process concern. Check ids stay an open vocabulary in this change. The
uniform `make gate` entrypoint is out of scope (§Non-goals of the
proposal) with the unbound remediation naming each repo's real command as
the partial answer. On **2026-10-02** the owner ruled that exemptions are
capped per repository at `min(floor(0.25 × required_checks), 5)` and that
`build`, `tests`, `security`, and `repository` (plus the
`repository_integrity` alias) are never exempable; the
rationale and the counting rules are in §3.5.

**Open decisions for the owner, deliberately not settled here.**

1. **When does `binding_policy` default to `reject`?** The package ships
   `report` as the default because parse-time rejection writes no run row
   and would blind the phase-0 inventory. The date or milestone at which
   the default flips is a policy call about the fleet, not about the
   runtime.
2. **Is `UNBOUND` blocking from day one, or shadowed for one cycle?** The
   design assumes phase 0 is non-blocking and phase 2 flips it. If the
   owner prefers the rule to be honest from the first release, the ~70-repo
   red flood is immediate and must be accepted deliberately rather than
   absorbed by phasing.
3. **May `not_applicable` exempt a check the profile requires by default?**
   This change allows it, with a recorded reason. The alternative — a
   profile default may only be exempted by an explicit per-check override —
   is stricter and would force the 41 product-profile repos to re-declare
   their inherited concerns. The trade-off is exemption-honesty versus
   manifest verbosity.
4. **Is there a ceiling on exemptions?** **Answered by the owner on
   2026-10-02** and implemented — see §3.5. The ceiling is
   `min(floor(0.25 × required_checks), 5)` per repository, with `build`,
   `tests`, `security`, and `repository` (and the `repository_integrity`
   alias) never exempable, a `WARN` at the
   ceiling and a blocking `FAIL` above it. The remainder of this ledger entry
   is superseded: the question is no longer open.
5. **Should the fleet migration be one workspace change or 70 per-repo
   changes?** §6 assumes per-repo. A single fleet-wide change with per-repo
   sub-tasks is plausible and is a review-process decision.
6. **Should `UNBOUND` be a distinct status, or should the existing
   `NOT_APPLICABLE` gain a `provenance` field instead?** The status
   variant is proposed because a new field on a shared struct is a weaker
   signal for machine consumers and is silently ignored by any consumer
   that does not read it. If backward-compatibility pressure on downstream
   status consumers outweighs that, the field variant is the fallback.

**Blockers:** none for the runtime semantics. The fleet migration is
blocked on the owner's answer to open decisions 1 and 2, and the
`generic-gate-contract` companion delta (task 4.2) is required before
archive.

### 8.1 Dispositions implemented in phase 1 (for the owner to confirm)

These are not answers to the open decisions above: they record what the
normative `spec.md` already fixes, so the owner reviews one list
instead of reconstructing it from the diff. None of them implements the
enforcement flip, which is the fleet migration's subject.

1. **Default to `reject` when?** Not implemented; `report` ships as the
   default because the spec fixes it (`binding_policy` defaults to
   `report` and a required unbound check blocks unconditionally). The
   flip date remains the owner's call and is *not* decided in code.
2. **Blocking from day one, or shadowed?** Implemented as blocking from
   the first release: a required `UNBOUND` is a `FAIL` in the aggregate
   and bypasses `review_required_blocks`. Measured consequence at phase 1:
   59 of the 65 workspace manifests that parse today go red (10 more are
   red on inspection but already fail to parse for a pre-existing
   unknown-field reason), and the 6 repos that bind every required
   command are unaffected. Phasing is therefore a *migration* choice
   about the fleet, not something the runtime can defer.
3. **May `not_applicable` exempt a profile default?** Implemented as
   allowed, with a recorded reason, because the spec requires the system
   to "accept a `not_applicable` declaration mapping a selected concern
   id" and a profile default is a selected concern. The stricter
   alternative in this decision is still available to the owner; it would
   require an extra parse rule, not a redesign.
4. **Ceiling on exemptions?** Implemented, on the owner's 2026-10-02 answer:
   the manifest layer now computes `min(floor(0.25 × required_checks), 5)` from
   each project's own selection, refuses nothing at parse time, warns with a
   `REVIEW_REQUIRED`/`Warning` `exemption-budget` result at the ceiling, fails
   the run above it, and hard-fails any exemption of `build`, `tests`,
   `security`, or `repository` regardless of the count. Consequence to note in
   the same breath: a project selecting fewer than four required checks has a
   ceiling of zero and may not exempt anything.
5. **One workspace change or 70 per-repo changes?** Not a runtime
   question; untouched here, as §6 assumes per-repo.
6. **Status variant or `provenance` field?** Implemented as the status
   variant, which the spec requires ("a distinct unbound outcome",
   `UNBOUND` distinct from `NOT_APPLICABLE`). The field fallback would
   need a different spec and is not a drop-in change, because the
   aggregate and persistence behaviour differs.

**Recorded, not decided:** decisions 1 and 5 remain open, and the
flipping of enforcement they gate is explicitly outside this change. Decision
4 is closed by §3.5. The change is not complete, and must not be represented
as complete or archived, while those answers are outstanding and the
final-BFS tasks in `tasks.md` remain open.
