# Proposal: A required Gate check with no bound command fails closed

## Why

Driftwatchdog owns the central Gate for a workspace of sibling repositories.
76 of them declare Gate policy in `.ai-gate/gate.yaml`, and all 76 name
`runtime: driftwatchdog`. 70 of those 76 declare **no `commands:` block at
all**.

A check declared `tests: true` is a required check: `src/gate/aigate.rs`
turns the `true` selection into `ManifestCheck { required: true }`, and
`src/gate/manifest.rs::resolve` carries that into
`PlannedCheck { required: true }`. Nothing in the parse path requires that
the check also bind a command — `validate` rejects an *empty* command for
an enabled check (`manifest.rs:460-468`), never an *absent* one. So the
manifest commits an assertion of coverage that the manifest never provides
an implementation for, and it parses clean.

Execution then takes the `None` arm in `src/commands/gate.rs:228-243`,
which records `GateStatus::NotApplicable` with `GateSeverity::Info`, zero
findings, and the diagnostic "no command declared for this concern; no
execution was attempted".

Two things follow from that, and the second is the one that matters:

1. `NOT_APPLICABLE` is **overloaded**. It means both "a command ran and
   legitimately reported not-applicable" (`src/gate/adapters.rs:1143`
   maps `NOT_APPLICABLE` to exit code 0 and admits it) and "we never ran
   anything at all". Those are opposite claims — a proven exemption and an
   unproven absence — and they share one status, one severity, and an empty
   findings list.
2. A **required** `NOT_APPLICABLE` does reach `pending_reviews` in
   `src/gate/aggregate.rs:95-101`, and all 76 manifests list
   `blocking: [FAIL, REVIEW_REQUIRED]`, so the aggregate does block and
   `driftwatch gate` does exit 1. The honesty of the system therefore rests
   entirely on a review-policy toggle the project controls, while the
   *result record* that every other consumer reads — the persisted
   `gate_runs.results_json`, the human render, the JSON render, the
   evidence export — carries a non-failing, `Info`-severity, zero-finding
   record for a check the project declared mandatory. Drop
   `REVIEW_REQUIRED` from a `blocking:` list and that same required,
   never-executed check aggregates to `PASS` and a green run is persisted.
   That is the fail-open.

The measured blast radius is larger than the manifests alone suggest:

- `tests` is required in 74 repos and bound in 6 — **68 unbound**.
- `security` is required in 74 and bound in 6 — **69 unbound**.
- `build` is required in 72 and bound in 6 — **66 unbound**.
- `repository` is required in 63 and bound in 6 — **61 unbound**.
- 41 of the 76 sit on the built-in `product` (33) or `rust-product` (8)
  profile, which additionally injects `product-code-boundary` and
  `placeholder-threshold` as required concerns that no repo binds. Only
  `source-file-size` runs in-process and needs no command.
- Check-kind coverage is presence-only: 34 check kinds are declared by
  exactly one repo, `accessibility` by 4, `format` by 3, `lint` by 2 — and
  every one of those is unbound.

So a repo can assert a test, security, build, and accessibility gate,
execute none of them, and hold a Gate record that shows no findings, no
failures, and no severity above `Info`. The central Gate is measuring
declaration, not coverage.

## What Changes

- Add a distinct, fail-closed result status for a required check that has
  no bound command, so "asserted and never ran" stops sharing a status
  with "ran and is legitimately exempt". The unbound result carries
  `Error` severity, names the check and the missing `commands.<id>` key,
  and blocks the aggregate **regardless of the project's `blocking:` list** —
  closing the `blocking: [FAIL]` fail-open.
- Add parse/resolve-time validation that a required check either binds a
  command or is covered by an in-process concern, under a new
  `binding_policy` manifest key. The first release accepts `report` (the
  fail-closed runtime status, no manifest rejection) and `reject` (the
  manifest is refused before any check executes, matching the existing
  fail-before-execution posture).
- Add a first-class `not_applicable` declaration — concern id to a
  non-empty, bounded, redacted reason — so a repo whose stack genuinely
  has no such gate (a Rust CLI declaring `accessibility`) states that
  explicitly and on the record instead of leaving the absence implicit.
  This is the answer for `accessibility`, `lint`, and `format` as well as
  for every one of the 34 single-repo check kinds: three lawful outcomes
  (bind a command, declare not-applicable with a reason, or mark the check
  `optional`), and no silent ignore.
- **Cap the exemptions one repo may declare**, at
  `min(floor(0.25 × required_checks), 5)` computed from that repo's own
  manifest, so the exemption path above cannot be used to exempt an entire
  gate. `build`, `tests`, `security`, and `repository` — and the
  `repository_integrity` alias the workspace also uses — are never exempable
  and an exemption naming one of them is a hard `FAIL`; reaching the cap is
  a `WARN` and exceeding it is a blocking `FAIL`. Owner decision of
  2026-10-02; see `design.md` §3.5 for the rationale.
- Record unbound coverage in the persisted `gate_runs` row and in the
  human and JSON renders, so the fleet's coverage debt is enumerable from
  the database rather than inferred from exit codes.
- Correct the workspace inventory in `driftwatchdog/README.md`, which
  claims "75 workspace `.ai-gate/gate.yaml` files" where the measured count
  is 76. (Documentation correction recorded as a task; this planning
  package does not edit it.)

This package fixes the **runtime semantics** only. Moving the 70 repos from
unbound to bound is a per-repository migration and is deliberately not in
this package — see Non-goals.

## Package Boundary and Split Assessment

| Package | Single outcome | Owner/project and language | Boundary/contract | Depends on | Independent oracle |
|---|---|---|---|---|---|
| `gate-unbound-check-fails` | An unbound required check cannot produce a non-failing result or a green run | driftwatchdog, Rust | `GateStatus` vocabulary, `.ai-gate/gate.yaml` `binding_policy` / `not_applicable`, aggregation and persisted `gate_runs` row | existing `aigate` parse, `manifest::resolve`, `aggregate` | fixture manifests + gate-run contract tests over the existing in-process harness |
| fleet command binding (not this package) | Each of the 70 repos binds, exempts, or relaxes its own checks | 70 sibling repositories, heterogeneous | per-repo `.ai-gate/gate.yaml` edit | this runtime change landing | each repo's own gate run goes green honestly |

The split is the point: the runtime can be made honest in one change, but
only 6 repos are currently in a position to stay green, so the migration
is inherently a per-repo and per-language exercise (Node, Rust, .NET,
Python). Bundling them would make the semantics unreviewable and the
rollout unreviewable in the same way the current state is.

## Sibling and Shared Architecture Reconnaissance

| Candidate | Evidence path/symbol | Reusable code/config/architecture | Compatibility gap | Owner and release boundary | Decision |
|---|---|---|---|---|---|
| Gate aggregation | `src/gate/aggregate.rs:95-101`, `PlannedCheck.required`, `BlockingPolicy` | The required/`optional` axis and the explicit blocking policy are exactly the right axes; no new concept needed | Maps a required `NOT_APPLICABLE` to a review, so a project-controlled `blocking:` toggle decides whether an unexecuted assertion blocks | driftwatchdog release | **extend shared owner** |
| Result normalization | `src/gate/types.rs:12`, `src/gate/adapters.rs:1143-1152` | `GateStatus` / `GateSeverity` and the exit-code authority table | `NOT_APPLICABLE` is overloaded across proven-exempt and never-attempted | driftwatchdog release | **extend shared owner** (vocabulary addition) |
| Manifest parse | `src/gate/aigate.rs:159-180` (commands cross-reference), `src/gate/manifest.rs:470` `validate` | Fail-before-execution posture, `unknown_field_error` remediation style, `ManifestError` variants | Rejects empty commands, never absent ones; no `binding_policy` or `not_applicable` vocabulary | driftwatchdog release | **extend shared owner** |
| Product-quality and release profiles | `src/gate/manifest.rs:28-41` module contract | Already documents "required + missing-command path" for the 41 product-profile repos | Inherits two required concerns no repo can bind | driftwatchdog release | **adopt, inherit the new ruling** |
| Evidence and report surfaces | `src/commands/gate.rs:332-346`, `gate_runs.results_json` | One persisted row per run already exists | Per-result records carry the `Info` unbound result unchanged | driftwatchdog release | **extend shared owner** |
| Repo entrypoints (`make gate`, npm/dotnet scripts) | `openpanel/Makefile:62`; 7 of 81 repos have a Makefile | Per-repo command binding is already expressed as a free-form string in `commands.<id>` | ~5 entrypoint shapes for one logical action | each repo's own build system | **defer — see Non-goals** |
| CI gate integration | repo CI workflows; 57 run tests, 0 run both | Existing CI posture | A gate that cannot go green is not yet wired anywhere | workspace-governance repo | **out of repo — integration concern only** |

## BFS Impact Map

- **Actors:** the Gate runtime (single owner of the ruling), the 76 project
  Gate manifests, the `GateStatus`/`GateSeverity` vocabulary and its
  consumers (human render, JSON render, `gate_runs.results_json`, evidence
  export, MCP read tools), and the per-repo maintainers who must eventually
  bind or exempt their own checks.
- **State:** a check is now four-way rather than three-way — bound
  (executes, PASS/FAIL/REVIEW_REQUIRED/NOT_APPLICABLE as its command
  reports), in-process (executes without a command; `source-file-size`
  today), unbound-required (new: fails closed), and
  unbound-optional (reported, non-blocking). A project may additionally
  move a check into an acknowledged exemption with a recorded reason, and
  the number of exemptions it may hold is now bounded by its own manifest
  (`min(floor(0.25 × required_checks), 5)`, with four non-exempable spine
  checks), so "acknowledged" cannot become "everything".
- **Persistence:** no new table and no migration. The unbound result is
  carried inside the existing `gate_runs.results_json` payload, and the
  aggregate's existing `not_applicable` vector is extended with an
  `unbound` vector so a run row alone distinguishes proven-inapplicable
  from never-attempted. `gate_runs.status` stays inside the existing
  PASS/FAIL/REVIEW_REQUIRED vocabulary.
- **Security:** no new process, no new subprocess contract, no new
  dependency, and no new code path that executes anything. The new status
  is produced only on the branch where no command exists, so it cannot
  introduce an execution surface. The `not_applicable` reason is a
  project-authored free-text string that is persisted, so it is length
  bounded and passed through the existing `gate::redact` credential
  redaction before it reaches a render, the JSON envelope, or the run row.
  Command strings remain bound by the unchanged `sh -c` adapter contract.
- **Failure:** a required check with no command and no exemption fails
  closed with a named remediation. A required check with an *empty* command
  string keeps its existing parse-time `EmptyCheckCommand` error. A
  `commands` key naming an unselected check keeps its existing parse-time
  error. A `.ai-gate/gate.yaml` naming a foreign runtime keeps its existing
  "not executed, no run recorded, exit zero" behaviour. A manifest
  declaring an unrecognised check kind is not rejected by id — that
  vocabulary stays open in this change — but it is bound by the same
  fail-closed rule, so an invented kind can never be silently ignored.
- **Compatibility:** all 6 repositories that bind commands — `byok-translator`,
  `careBridge`, `labrys`, `openpanel`, `therapist-commons`, `vowra` — were
  measured to have **zero** required checks left unbound, so none of them
  changes status and none needs a manifest edit. Bound-command execution,
  the in-process `source-file-size` scanner, the AI review path, the
  `driftwatch check` compatibility entry point, and every `.ai-gate/gate.yaml`
  field already in the schema are untouched. The `blocking:` list keeps its
  meaning for `REVIEW_REQUIRED`; it simply no longer governs whether an
  unexecuted required check blocks.
- **Verification:** parse fixtures for bound/unbound/optional/disabled/
  in-process/exempt manifests; unit tests for the new status and the
  `binding_policy` switch; aggregation tests proving a required unbound
  check blocks under `blocking: [FAIL]`, `[FAIL, REVIEW_REQUIRED]`, and
  with an empty blocking list, and that an optional unbound check does not;
  a run-persistence test proving the `unbound` vector survives into
  `gate_runs.results_json`; a redaction test for an `not_applicable`
  reason; a fleet inventory run over the 76 real manifests recording
  exactly which repos would go red; and the 6 binding repos replayed
  unchanged.

## Capabilities

- **Modified:** `gate-project-configuration` — the manifest-to-plan
  mapping that currently turns an unbound required selection into a
  non-failing `NOT_APPLICABLE`, plus the new `binding_policy` and
  `not_applicable` vocabulary and the corrected workspace inventory.
- **New:** none. The status vocabulary and aggregation text that this
  change also changes live in `generic-gate-contract`; that companion
  delta is recorded as a required follow-up task rather than smuggled into
  this package's single spec file.

## Non-goals

- **Not** binding commands for the 70 unbound repos. That is 70
  language-specific, per-repository migrations (Node, Rust, .NET, Python)
  and is a separate change; this package only makes their current state
  visible and red.
- **Not** mandating a uniform `make gate` entrypoint. Only 7 of 81 repos
  have a Makefile, and driftwatchdog executes language-neutral
  `sh -c` strings; requiring `make` would either exclude repos whose
  real entrypoints are `npm run`, `dotnet test`, or `msbuild`, or force a
  build-tool shim into 70 repos before any gate can run. Standardising the
  entrypoint is a fleet-migration concern. What this change does owe that
  problem is that every unbound check must now name its remediation and
  every exemption must carry a written reason, so the per-repo entrypoint
  choice becomes an explicit, reviewable artifact even though it is not
  yet a uniform one.
- **Not** introducing a closed concern-id vocabulary. 34 check kinds are
  declared by exactly one repo; closing the vocabulary would reject those
  manifests and is a breaking change for ~40 repos. Unbound unknown kinds
  are handled by the same fail-closed rule instead.
- ~~**Not** capping or inventorying `not_applicable` exemptions.~~ **Decided
  2026-10-02 and now in this package:** exemptions are capped per repository
  at `min(floor(0.25 × required_checks), 5)`, and `build`, `tests`,
  `security`, and `repository` may never be exempted. The cap, the spine, and
  the `WARN`/`FAIL` mapping are normative requirements in
  `specs/gate-project-configuration/spec.md`; a repo that genuinely needs more
  is reviewed, not self-authorised.
- **Not** building a workspace-level exemption *budget* or coverage metric
  across repos, and not granting any per-repo override of the cap. The cap
  answers "can this repo exempt its way to green?"; the cross-repo budget
  remains a workspace-governance concern, and an override would reintroduce
  the self-authorisation the cap removes.
- **Not** changing CI wiring. 57 repos run tests in CI and 0 run both
  tests and the gate; that belongs to the workspace-governance repository
  and is noted here only as an integration concern.
- **Not** changing the AI review path, the evidence export format beyond
  carrying the new status, or the `driftwatch check` protocol.
