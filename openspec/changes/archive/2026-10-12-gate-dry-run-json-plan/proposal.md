# Proposal: Make `driftwatch gate --dry-run --format json` machine-readable without letting a rehearsal be read as a verdict

## Why

`driftwatch gate --dry-run --format json` ignores `--format`. Measured on this
repository's own build at `cf823dc`, with a three-check fixture:

```
$ driftwatch gate --dry-run --format json
gate plan (dry-run; nothing was executed)
contract version: 1 | profile: minimal | rule-pack: workspace-governance@1 | manifest: sha256:a221e8e3a93b303f
blocking: review_required_blocks=true
execution mode: dry-run
...
```

Byte-identical to `--format human`, and the exit code is `0`. The `--format`
value selects a document; the dry run prints prose and returns before the
document writer is reached.

**Root cause**, one branch, `src/commands/gate.rs:121-124`:

```rust
if args.dry_run {
    print!("{}", manifest::render_plan(&resolved));
    return Ok(EXIT_OK);
}
```

The `match args.format` that dispatches to `print_json` /
`print_human` / `print_platform` sits at `src/commands/gate.rs:148-158`,
twenty-four lines *after* this unconditional return. `--dry-run` therefore
chooses its own output format and its own exit code before the flag is ever
read.

## The consumer that is already affected

Forge (`/home/paul/code/forge`) runs the rehearsal as a bounded argument
array `["gate", "--dry-run", "--format", "json"]`
(`src/gate/engine.rs:259-264`) and documents the consequence in its own module
header (`src/gate/mod.rs:13-21`): *"the dry-run surface prints a
human-readable plan and exits before the JSON writer, so a rehearsal is
reported as a plan preview that is never persisted and never journaled."*
Its `GateOutcome` therefore has a `PlanPreview { plan: Vec<String> }` arm
(`src/gate/model.rs:117-131`) that scrapes text lines. Forge's `AGENTS.md`
mandates a `forge gate --dry-run` rehearsal before every archive, and that
mandatory rehearsal is not machine-readable today.

Nothing in Forge is modified by this change. Forge is the reader of the fix.

## The hard part is not emitting JSON, it is not lying with it

A rehearsal and a verdict are the same question to a naive consumer. The
failure this repository exists to prevent is the false green: a simulated
quality score read as an observed one. So the emitted document must be
provably incapable of being read as a verdict:

* **No verdict-shaped field at all.** Not `status`, `result`, `passed`,
  `blocked`, `outcome` — at the root or anywhere else. The real `--format
  json` document's whole key set is exactly that vocabulary, and this
  document must not be a near-miss of it.
* **Declared as a rehearsal**, twice over: a `dry_run: true` marker *and* a
  document-kind field, plus a root `contract` versioned the way
  `src/checker/json.rs:41` already does it (`driftwatch-checker/0.1.0`).
  Forge's own `GateEvidence` models rehearsals as `dry_run: true` and never
  persists them.
* **Planned checks carry no outcome.** A planned check has an id, whether it
  is required, where its command would come from, and its effective
  deadline. It has no status, because nothing ran and there is nothing to
  report.

## The question the document must answer without executing anything

"A rehearsal is most useful if it can answer *would this plan block?*" — that
is answerable, and only in one direction. `aggregate()`
(`src/gate/aggregate.rs:89-208`) derives `blocked` from results, so the
outcome of every check with a bound command is unknowable before it runs.
But two plan facts are deterministic, decided at resolution time and
independent of any child's exit code:

1. `ResolvedGatePlan::unbound` — required, non-exempt, non-in-process checks
   with no bound command. `execute_plan` gives each of them
   `GateStatus::Unbound`, and `aggregate()` turns a required `UNBOUND` into a
   failure, so **one such check forces `blocked: true` whatever every other
   command does**.
2. `ResolvedGatePlan::exemption_budget` — `OverCap` and `HardFail` become a
   required `FAIL` plan-level result, and so also force a block. `Exhausted`
   becomes an *optional* `REVIEW_REQUIRED` and forces nothing.

So the plan can prove a block and can never prove a pass. The document says
exactly that, in a `blocking_prediction` object that is explicitly a
prediction, never named `blocked`, and that emits `would_block: true` only
when the plan already decides the answer. When the plan cannot decide, the
field is `null` with an explicit "undetermined" outcome word — never `false`,
because `false` is a claim that nothing ran and passed.

## What Changes

- **New `src/gate/dry_run_plan.rs`** — the only place that composes the
  rehearsal document. It is a pure function of an already-resolved plan and
  the already-loaded manifest: no clock, no database, no child process, no
  filesystem write.
- **New `src/gate/dry_run_plan_tests.rs`** — the unit tests, split out
  following the `platform_result_tests.rs` precedent so neither file can
  cross the 1,000-line source-file-size gate.
- **`src/commands/gate.rs`** — the dry-run branch (lines 121-124) delegates to
  the new module instead of printing text unconditionally. The file does not
  grow: it loses four lines and gains a call plus one import.
- **New `tests/gate_dry_run_json.rs`** — end-to-end coverage through the
  binary: the document round-trips, carries no verdict field, the human and
  real-JSON surfaces are byte-identical to a captured baseline, and no
  `gate_runs` row is created.

## Compatibility surface (unchanged, and proven by test)

| surface | before | after |
|---|---|---|
| `--dry-run --format human` | `render_plan` text | **byte-identical** |
| `--format json` (real run) | status document | **byte-identical** |
| `--format human` (real run) | status table | **byte-identical** |
| `--format platform` (real run) | `platform.gate-result` 1.0.0 | untouched |
| `--dry-run --format platform` | human plan text, exit 0 | **unchanged**, reported not invented |
| `--dry-run --format json` exit code | `0` | **`0`**, with the rule cited |
| persistence on a rehearsal | none | none |

## Exit code: unchanged, and the rule that pins it

`openspec/specs/gate-cli-and-memory-integration/spec.md`, requirement *Run
Gate locally*, is the only canonical requirement that speaks about the gate
command's exit code: *"returns a nonzero exit code for blocking failures or
unresolved blocking reviews"*, with scenario *Dry run*: *"the resolved plan is
shown without child execution or persistence"*. Both halves of that sentence
are about what the **run** observed. A rehearsal observes neither a blocking
failure nor an unresolved blocking review, so exit `0` is the specified
behaviour, not an accident. It is also pinned by an existing test
(`src/commands/gate.rs:903`, `gate_dry_run_shows_plan_without_execution_or_persistence`,
commented *"Even though the check would fail, dry-run exits zero and persists
nothing"*). Changing it would mean editing a canonical spec of an unrelated
capability and pinning a second behaviour with a test, for no gain: the new
`blocking_prediction.would_block` carries the answer the exit code never
gave, in a form that cannot be mistaken for the exit code itself.

## Non-goals

- No new `--format` value. `human`, `json` and `platform` are unchanged.
- No change to `GateStatus`, `aggregate()`, `NOT_APPLICABLE` / `UNBOUND`
  semantics, the manifest, the SQLite schema, or the gate pipeline.
- No change to `src/gate/platform_result.rs` or its tests.
- No JSON-Schema dependency. The consumer validates.
- No edit to any of the eight open changes. `per-check-gate-timeout` is
  adjacent and is not collided with: that change owns
  `ManifestCheck.timeout_ms` and its resolution, this change only *reads*
  `ResolvedGatePlan::check_timeouts`, which already exists on `main`.
- No change to Forge or any other repository.