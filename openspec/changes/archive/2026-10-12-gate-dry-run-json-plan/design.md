# Design: `gate-dry-run-json-plan`

## 1. The defect, located

`src/commands/gate.rs:121-124`, inside `gate()`:

```rust
if args.dry_run {
    print!("{}", manifest::render_plan(&resolved));
    return Ok(EXIT_OK);
}
```

This is an unconditional return that sits **above** the format dispatch:

```
121  if args.dry_run { ... return Ok(EXIT_OK); }
126  let mut db = crate::repo::Db::open(&proj.db_path)?;      // never reached on a rehearsal
127  let (results, plan) = execute_plan(...)?;
128  let outcome = aggregate(&plan, &results);
133  Gates::insert(...)                                       // the gate_runs row
148  match args.format {
149      Json     => print_json(...),
150      Human    => print_human(...),
151      Platform => print_platform(...),
```

Two consequences follow from one ordering decision:

1. `args.format` is never read on a rehearsal, so `--format json`,
   `--format platform` and `--format human` all render the same prose.
2. `print_json` composes its document from `AggregateOutcome` and
   `Vec<GateResult>`. Both are *products of execution*
   (`execute_plan` → `aggregate`). A rehearsal has neither, so it cannot
   reach that writer without inventing the very thing it must not claim.

The fix is therefore not "move the branch". Moving it would compile a real
JSON document out of nothing and call it a verdict. The fix is a **third
document kind** for the rehearsal, composed from the resolved plan.

## 2. Module and file boundary

| module | owns | line budget |
|---|---|---|
| `src/gate/dry_run_plan.rs` | the rehearsal document: contract constant, planned-check rows, blocking prediction, the three-arm dry-run printer | < 600 |
| `src/gate/dry_run_plan_tests.rs` | unit tests for the above | < 600 |
| `tests/gate_dry_run_json.rs` | end-to-end coverage through the binary | < 600 |
| `src/commands/gate.rs` | one delegating call | **does not grow** |

`src/commands/gate.rs` is 1,110 lines today and is already an offender for
the `source-file-size` concern. This change must not make that worse, so the
whole dry-run printing branch — format dispatch included — moves into the new
module:

```rust
// src/commands/gate.rs, replacing the four lines above
if args.dry_run {
    return Ok(dry_run_plan::print_dry_run(&args.format, &resolved, &manifest));
}
```

Four lines out, three in, one import added. The net effect on the offending
file is −1 line.

Placing the module under `src/gate/` rather than `src/commands/` follows
`platform_result.rs`: it is a pure projection of gate domain types onto a
document, with no command dispatch of its own.

**No JSON-Schema dependency.** The portfolio rule already recorded in
`ROADMAP.md` for `--format platform` applies identically here: *the consumer
owns validation, and a second copy of the schema's rules inside the producer
would mask schema drift rather than surface it.*

## 3. The document

### 3.1 Root

| key | type | why |
|---|---|---|
| `contract` | string | `driftwatch-gate-plan/0.1.0`. Precedent: `src/checker/json.rs:41` (`CHECKER_REPORT_CONTRACT` → root `contract`), then `platform_result.rs` (`platform.gate-result/1.0.0`). One spelling, one constant. |
| `document_kind` | string | `gate-plan-rehearsal`. Names the *kind* of thing this is, in a field whose only legal value for this producer is the rehearsal kind. |
| `dry_run` | bool | Always `true`. Forge's `GateEvidence` already models a rehearsal with exactly this marker (`/home/paul/code/forge/src/gate/model.rs:117-131`) and never persists one. |
| `execution_mode` | string | `dry-run`, carried from `ResolvedGatePlan::execution_mode`. The human plan prints the same line, so the two surfaces say the same thing. |
| `executed` | bool | Always `false`. |
| `persisted` | bool | Always `false`. Stated, not merely true, because "a rehearsal wrote a `gate_runs` row" is precisely the claim a consumer must be able to rule out from the document alone. |
| `notice` | string | One sentence saying what this is and is not, for the human who opens the file first. |
| `manifest_digest` | string | Plan identity. Same value the real document and the human plan carry. |
| `rule_pack_version` | string | Rule-pack identity. |
| `profile` | string | Project-owned profile, verbatim. |
| `plan_version` | u32 | `ResolvedGatePlan::plan.version` (the generic gate contract version). |
| `blocking_policy` | object | `{ "review_required_blocks": <bool> }` — the resolved policy, not a boolean shorthand. |
| `default_timeout_ms` | u64 | The default every undeclared check resolves to. |
| `planned_checks` | array | §3.2. |
| `exempted_checks` | object | Check id → bounded reason. An acknowledged absence, stated as an input, not as a result. |
| `unbound_required_checks` | array | Check ids that assert coverage and bind no command. A **plan fact**, already computed by `resolve()`. |
| `exemption_budget` | object | `ExemptionBudget` plus `headroom` and a `verdict` word. All resolved at plan time. |
| `not_scheduled` | array | Concerns explicitly excluded, so an absence is never read as a silent skip. |
| `context_providers` | array | Names only. |
| `project_command_names` | array | Names only — see §3.4. |
| `source_size_policy` | object | The built-in scanner's resolved policy. It runs in-process, so a rehearsal must still say what it *would* apply. |
| `governance_quality_codes` | array | The concern's resolved code scope. |
| `blocking_prediction` | object | §4. |

### 3.2 A planned check

```json
{ "id": "smoke", "required": true, "exempt": false,
  "selected_by": "manifest", "command_source": "manifest", "timeout_ms": 60000 }
```

* `id`, `required`, `exempt`, `selected_by` come straight from
  `PlannedCheck` (`selected_by` is `PlannedCheck::source`, renamed so it
  cannot be confused with a command origin).
* `command_source` answers "where would the command come from", and is one of:
  * `in-process` — the built-in `source-file-size` scanner. The runtime runs
    it itself; no binding is needed and none is expected. Same predicate
    `unbound_required_checks` uses (`is_source_file_size_concern`).
  * `manifest` — the manifest binds a `command` for this check.
  * `none` — nothing is bound. For a required check this is the coverage debt
    `ResolvedGatePlan::unbound` already lists.
* `timeout_ms` is the effective deadline from `ResolvedGatePlan::check_timeouts`,
  default included.

**There is no `status`, `result`, `severity`, `diagnostic`, `finding` or
`outcome` key on a planned check, at any level.** A rehearsal has no outcome
to report. The most a plan can say is "this would run, here is its deadline",
and that is what it says.

### 3.3 The no-verdict rule, mechanically

The prohibition is enforced by a test, not by prose. The test walks the
parsed document recursively — every object and every array, at every depth —
and fails on any key in a closed deny-set:

```
status, result, results, passed, blocked, outcome, aggregate, verdict_outcome,
exit_code, gate_status, ok, success, failures, pending_reviews
```

`blocking_prediction.would_block` is deliberately **not** in the deny-set: it
is the one field whose entire job is to be about blocking, and it is safe
because (a) it is never named `blocked`, (b) it can only be `true` or `null`,
never `false`, and (c) it is nested inside an object whose sibling keys are
`kind`, `derived_from`, `sufficient_but_not_necessary`, `reasons` and `note`,
none of which is verdict-shaped.

This is also why the prediction's own outcome word is **`kind`**, not
`outcome`: naming that key `outcome` would put a key called `outcome` in the
document, and the deny-set rule must have no exceptions. `kind` carries the
same information with no self-contradiction.

### 3.4 What is deliberately *not* in the document

* **Command text.** A check command is an arbitrary project-authored string
  that can carry credentials and can be arbitrarily long. `ResolvedGatePlan`
  does not carry it (the manifest does, and
  `bound_results_json` bounds what reaches the database at
  `MAX_RESULTS_JSON_BYTES`). A rehearsal gains nothing from reproducing it, so
  the document names the *source* of each command and nothing more. The human
  dry-run plan already prints command text, byte-identically, for anyone who
  needs it.
* **`evaluated_at`.** The platform document carries it because a verdict is
  about a moment. A rehearsal has no moment; `chrono::Utc::now()` would be the
  first clock read this module performs and would make the document
  unreproducible for no gain.
* **`gate_id`.** The platform document needs a subject identity. A rehearsal
  is not about a revision; `manifest_digest` identifies what was planned.

## 4. `blocking_prediction` — what is knowable, and how it is worded

### 4.1 The answer

**Blocking is partially knowable pre-execution, in one direction only.**

`aggregate()` (`src/gate/aggregate.rs:89-208`) computes `blocked` from
`Vec<GateResult>`:

| plan state | what `execute_plan` produces | what `aggregate` does | predictable? |
|---|---|---|---|
| required check with a bound command | whatever the child does: `PASS` / `FAIL` / `REVIEW_REQUIRED` | depends entirely on the child | **no** |
| required check, no command, no exemption (`ResolvedGatePlan::unbound`) | `GateStatus::Unbound`, `Error`, rule `unbound-required-check` | `failures.push(id)` → `status: FAIL, blocked: true` | **yes, forces a block** |
| exemption budget `OverCap` / `HardFail` | a required `FAIL` plan-level result, rule `exemption-budget-exceeded` | `failures.push(id)` → `blocked: true` | **yes, forces a block** |
| exemption budget `Exhausted` | an **optional** `REVIEW_REQUIRED` | `pending_reviews.push("optional:…")` → never blocks | forces nothing |
| optional check, any outcome | whatever | collected, never blocks | irrelevant |
| `ai-review` (added only when `[ai] enabled`) | adapter-dependent | may add a blocking review | **no** — not on the plan at all |

So the plan is a **sufficient but not necessary** condition for blocking. It
can prove "this will block"; it can never prove "this will pass". Two
consequences, both load-bearing:

* The document must be allowed to say `would_block: true`.
* The document must be **structurally unable** to say anything that reads as a
  pass. Hence `null`, never `false`.

Evidence that `ResolvedGatePlan::unbound` is exactly the forcing set: it is
built by `unbound_required_checks` (`src/gate/manifest.rs:1128-1144`), which
filters on `required && !is_source_file_size_concern(id) &&
!not_applicable.contains(id) && !binds_command(manifest, id)` — i.e. it
excludes in-process checks (which run) and exempted checks (which do not reach
the unbound branch in `execute_plan`). The one remaining divergence, an
exempted *non-exemptable* concern, is covered by the budget arm: `hard_fails()`
is true exactly when `exemption_budget.non_exempable` is non-empty, and that is
the same manifest declaration `execute_plan` reads on the
`non-exempable-concern` branch.

### 4.2 The shape

```json
"blocking_prediction": {
  "kind": "plan-forced-block" | "undetermined-before-execution",
  "derived_from": "resolved-plan",
  "would_block": true | null,
  "sufficient_but_not_necessary": true,
  "reasons": [ { "rule": "unbound-required-check", "checks": ["governance-quality"] } ],
  "note": "A prediction from the resolved plan alone, never an observed outcome. ..."
}
```

* `kind: "plan-forced-block"` ⟺ `would_block: true`. The plan already decides
  the answer.
* `kind: "undetermined-before-execution"` ⟺ `would_block: null`. The bound
  commands decide, and nothing about them is knowable from here. `null` means
  *unknown*, never *no*.
* `would_block: false` is **never emitted**, by construction: there is no code
  path that produces it. A test asserts the field is never `false` across a
  matrix of fixtures.
* `reasons` is empty when `kind` is `undetermined-before-execution`.

## 5. `--format platform --dry-run`: reported, not invented

Today `--dry-run --format platform` prints the human plan and exits `0`,
because the dry-run branch precedes the format dispatch. That is accidental,
but it is also the only honest answer available:

* `platform.gate-result` is an **outcome** contract. Its `result` and
  `checks[].status` fields have no value for "planned". A rehearsal cannot
  honestly produce one.
* Refusing with exit `2` would invent a new semantic (`EXIT_UNREPRESENTABLE`
  currently means "a real run could not be represented") and would change a
  surface this change does not own.
* Emitting anything else would be exactly the coercion the platform work was
  built to refuse.

**Decision: unchanged.** The branch becomes an explicit, documented arm so the
behaviour is no longer accidental, and it is pinned by a test so a future
reader finds the decision rather than the accident. Reported in HANDOFF as a
known, deliberate, unchanged combination.

## 6. Exit code

**Kept at `0`.** The rule, cited:

> `openspec/specs/gate-cli-and-memory-integration/spec.md`, requirement *Run
> Gate locally*: "The system MUST provide a `gate` command that resolves the
> project Gate plan, executes applicable checks, persists the result, and
> **returns a nonzero exit code for blocking failures or unresolved blocking
> reviews**." — with scenario *Dry run*: "**WHEN** `driftwatch gate --dry-run`
> is invoked **THEN** the resolved plan is shown **without child execution or
> persistence**."

The nonzero condition is defined over what the run *observed*. A rehearsal
executes nothing, so it can observe neither a blocking failure nor an
unresolved blocking review. `0` is the specified outcome, not a leftover.

It is independently pinned by an existing test:
`src/commands/gate.rs:903` `gate_dry_run_shows_plan_without_execution_or_persistence`,
whose comment reads *"Even though the check would fail, dry-run exits zero and
persists nothing."* That test is kept unchanged.

Changing the exit code would therefore require editing a canonical spec of an
unrelated capability and adding a second test pinning old *and* new
behaviour. The answer the exit code cannot give — "would this plan block?" —
is now available as data instead, in a field that cannot be confused with an
exit code.

**No test pinning old-and-new exit behaviour is added, because the behaviour
did not change.**

## 7. Side-effect freedom

The rehearsal path must stay byte-for-byte side-effect-free:

* The new module opens no database, spawns no child, reads no clock, writes
  no file. It is a pure function of `(&ResolvedGatePlan, &GateManifest)`.
* `print_dry_run` returns `EXIT_OK` for all three formats, so the exit code is
  structurally independent of the document's contents — a rehearsal can never
  be non-zero because of what it plans.
* `tests/gate_dry_run_json.rs` opens `.driftwatch/state.db` read-only after
  the run and asserts `Gates::count() == 0` for the JSON rehearsal, and
  separately that the human rehearsal still writes nothing.

## 8. Compatibility surface, and how it is proven

| surface | proof |
|---|---|
| `--dry-run --format human` | byte-identical: `print_dry_run`'s `Human` arm calls the same `manifest::render_plan(&resolved)` with the same `print!`; an end-to-end test captures stdout and compares it against a golden file checked into the test, generated from the pre-change binary. |
| `--format json` (real run) | untouched code path. `print_dry_run` is called only from inside `if args.dry_run`. A test asserts the real JSON document still has the full verdict key set — it must *gain* nothing. |
| `--format human` (real run) | untouched code path; same argument. |
| `--format platform` (real run) | untouched code path; `src/gate/platform_result.rs` and `src/gate/platform_result_tests.rs` are not edited at all (verifiable with `git diff --stat`). |
| exit codes | `0` for every rehearsal; `0`/`1`/`2` unchanged for real runs, per the existing suite. |
| persistence | `gate_runs` count stays `0` across all three rehearsal formats. |

The byte-identity claim is made against a **captured pre-change sample**, not
against a re-derivation of the current code: the fixture's expected bytes are
stored in the test, and the sample in `/tmp` used for the report diff was
captured from `cf823dc` before any edit.

## 9. Consumer impact (reported, not fixed here)

Forge's `run_gate` will keep classifying the rehearsal as
`GateOutcome::PlanPreview`, because Forge's `parse_gate_document`
(`/home/paul/code/forge/src/gate/engine.rs:362-364`) requires `blocked` (bool)
and `results` (array) — and this document deliberately has neither, since
either would be a verdict-shaped field. Consequences on the Forge side, which
this change does **not** touch:

1. `GateOutcome::PlanPreview.plan` will hold this JSON document's lines rather
   than the human plan's lines. They are bounded and redacted by Forge's own
   capture path, and the classification is unchanged — a rehearsal is still
   never evidence and never a pass.
2. Forge's `GateOutcome::PlanPreview` doc comment ("answered with the real
   sibling's human plan text") and `src/gate/mod.rs:13-21` become inaccurate.
3. Forge already has the vocabulary for the intended end state: `GateEvidence`
   carries `dry_run: true` and the header says *"a forward-compatible
   parseable document from a rehearsal (`dry_run: true`, never persisted)"*.
   Making `parse_gate_document` accept a `dry_run: true` document that has no
   `blocked`/`results` is a Forge-side change, in another repository, and is
   explicitly out of scope here.

## 10. Test matrix

| # | test | kind | proves |
|---|---|---|---|
| 1 | `dry_run_json_document_round_trips_and_declares_itself_a_rehearsal` | unit | parses; `dry_run == true`; `document_kind == "gate-plan-rehearsal"`; `contract == "driftwatch-gate-plan/0.1.0"`; `executed == false`; `persisted == false` |
| 2 | `dry_run_document_carries_no_verdict_field_at_any_depth` | unit | recursive deny-set walk over every key in the document |
| 3 | `a_rehearsal_never_predicts_a_pass` | unit | `would_block` is `true` or `null` across every fixture; `false` is unreachable |
| 4 | `a_required_unbound_check_forces_a_block_prediction` | unit | `kind == "plan-forced-block"`, `reasons` names the check and the rule |
| 5 | `an_all_bound_plan_is_undetermined_not_passing` | unit | `kind == "undetermined-before-execution"`, `would_block == null`, empty reasons |
| 6 | `an_over_cap_exemption_budget_forces_a_block_prediction` | unit | budget arm fires |
| 7 | `planned_checks_carry_no_outcome_and_a_resolved_deadline` | unit | per-check keys; declared `timeout_ms` honoured; default resolved |
| 8 | `dry_run_human_output_is_byte_identical_to_the_captured_baseline` | e2e | golden-bytes comparison |
| 9 | `a_real_run_json_document_is_unchanged_and_still_a_verdict` | e2e | the real document still has `status`/`blocked`/`results` |
| 10 | `a_rehearsal_creates_no_gate_runs_row_in_any_format` | e2e | `Gates::count() == 0` |
| 11 | `a_rehearsal_exits_zero_whatever_it_plans` | e2e | exit `0` for a plan the real run would block on |
| 12 | `platform_format_with_dry_run_still_prints_the_plan_and_exits_zero` | unit | §5, pinned |