# Tasks: gate-unbound-check-fails

Ordered BFS baseline → DFS requirement implementation → BFS regression and
completeness → Verification. Phase 1 (runtime + report) is implemented; the
enforcement flip and the fleet migration are not. **Archived 2026-10-13.**
Tasks 4.2 and 5.1-5.5 were fleet-scale and are now marked **carried, not
done**: each names the `workspace-fleet-check-sweep` item that holds it, and
none was deleted. Task 4.2 is **still unanswered** — the owner gave decision 4
(exemption ceiling) on 2026-10-02 and it is implemented in group 6 below;
decisions 1, 2, 5 and 6 were never answered, and this archive invents none.
See "Verification record" at the end of this file for measured results.

## 1. BFS — Baseline and impact coverage

- [x] 1.1 Map every consumer of `GateStatus` and `GateSeverity` —
  `src/gate/types.rs`, `src/gate/adapters.rs:1143-1152` (status-to-exit and
  status-to-severity tables), `src/gate/aggregate.rs`,
  `src/commands/gate.rs` (human render, JSON render, `results_json`
  binding), `src/gate/evidence_export.rs`, and `src/mcp/tools.rs` — to the
  new `UNBOUND` variant added in task 2.1. Any exhaustive match that is not
  in this list is a silent behaviour regression and must be added to it
  before implementation starts.
- [x] 1.2 Pin the current behaviour with a characterisation test before
  changing anything: a required check with no command today produces
  `NotApplicable` + `Info` + zero findings and reaches `pending_reviews`,
  and aggregates to `REVIEW_REQUIRED` with `blocking: [FAIL,
  REVIEW_REQUIRED]` and to `PASS` with `blocking: [FAIL]`. The second case
  is the fail-open this change closes and its characterisation test is the
  regression that must invert.
- [x] 1.3 Confirm the measured compatibility surface before touching the
  manifest schema: replay the 6 binding repositories
  (`byok-translator`, `careBridge`, `labrys`, `openpanel`,
  `therapist-commons`, `vowra`) through parse and resolve and record that
  each produces zero required checks left unbound. Any repo that does not
  hold is a new migration and must be added to the phase-1 inventory.
- [x] 1.4 Produce the phase-0 inventory from the 76 real manifests: for
  each repo, every required check that would be `UNBOUND` under the new
  rule, including the `product-code-boundary` and `placeholder-threshold`
  concerns inherited by the 41 repos on the built-in `product` /
  `rust-product` profile. Record the per-check totals (68 `tests`, 69
  `security`, 66 `build`, 61 `repository`) as the acceptance oracle for the
  blast radius. Read-only; changes nothing.
- [x] 1.5 Enumerate the schema touch points that must land together, since
  a new field without its allowlist entry is rejected as an unknown field
  in both forms: `YAML_FIELDS` in `src/gate/aigate.rs`, the native
  `gate.toml` field list, and the `ManifestError` variants in
  `src/gate/manifest.rs`.

## 2. DFS — Requirement-by-requirement implementation

- [x] 2.1 Add the `UNBOUND` status to `src/gate/types.rs` with its
  `as_str` / `from_str` arms, and the matching entries in
  `src/gate/adapters.rs`'s status-to-exit-code and status-to-severity
  tables. Evidence: round-trip unit test for the new variant plus
  unchanged-mapping assertions for the four existing variants.
- [x] 2.2 Change the `None` arm of the command match in
  `src/commands/gate.rs:228-243` to branch on `required`: a required
  unbound check records `UNBOUND` with `GateSeverity::Error`, zero
  findings, `missing_evidence: ["<id>:command"]`, and a remediation naming
  the exact `commands.<id>` key plus the `not_applicable` alternative; an
  optional unbound check records `UNBOUND` with `GateSeverity::Warning`.
  Both keep the "no execution was attempted" diagnostic so the record still
  says nothing ran. Evidence: the 1.2 characterisation tests inverted.
- [x] 2.3 Extend `src/gate/aggregate.rs`: a required `UNBOUND` lands in
  `failures` and blocks unconditionally — bypassing
  `plan.policy.review_required_blocks` and the project's `blocking:` list
  entirely — an optional `UNBOUND` is recorded with the existing
  `optional:` marker and never blocks, and a new `unbound` vector on
  `AggregateOutcome` keeps unbound ids out of `not_applicable` so a
  proven exemption is never conflated with a never-attempted check.
  Evidence: aggregation tests under `blocking: [FAIL, REVIEW_REQUIRED]`,
  `blocking: [FAIL]`, and no `blocking:` key.
- [x] 2.4 Add the `not_applicable` field to the `.ai-gate/gate.yaml` schema
  and parse it in `src/gate/aigate.rs` as concern id to non-empty reason.
  Add the same field to the native `gate.toml` form. Reject an empty or
  whitespace reason, an entry naming an unselected check, and an entry
  naming a check that also binds a command, each with a typed
  `ManifestError` naming the offending key. Apply length bounding and
  `gate::redact` credential redaction to the reason before it is persisted
  or rendered. Evidence: parse fixtures for all four cases plus the
  redaction and truncation tests.
- [x] 2.5 Add the `binding_policy` field with the closed value set
  `report | reject`, defaulting to `report`, rejecting an unknown value
  with the accepted set in the message. Under `reject`, refuse a required
  unbound check during parse/resolve before any check executes, naming
  every offending check id in one refusal. Evidence: one test per value
  plus the unknown-value refusal.
- [x] 2.6 Add the `validate` rule in `src/gate/manifest.rs` alongside the
  existing empty-command check: a required, enabled, non-in-process,
  non-exempt check with no `commands.<id>` entry is the violation. The
  in-process concern list (`source-file-size` today) is exempt, because 41
  repos inherit it through the product profiles and it is the one concern
  that runs without a command. Keep the absent case distinct from the
  existing `EmptyCheckCommand` case so their diagnostics do not merge.
  Evidence: unit tests for bound, unbound, in-process, exempt, and
  empty-string-command manifests.
- [x] 2.7 Surface the new state in output and persistence: the human render
  names unbound required checks and their remediation separately from
  proven not-applicable ids, the JSON envelope carries the `unbound` vector,
  and `bound_results_json` keeps the new result inside
  `gate_runs.results_json` so the debt is queryable from the database
  without re-running the gate. `--dry-run` stays pure and lists each
  unbound required check in the plan render. Evidence: the run-persistence
  test and the `--dry-run` purity assertion.
- [x] 2.8 Correct the workspace inventory in `driftwatchdog/README.md`,
  which claims "75 workspace `.ai-gate/gate.yaml` files" where the measured
  count is 76, and add the measured "70 declare no `commands:` block"
  figure so the documentation states the debt rather than only the
  manifest count. Evidence: the count is re-derived from the fleet, not
  copied.

## 3. BFS — Cross-surface regression and completeness

- [x] 3.1 Re-audit every surface that can read a gate result — human
  render, JSON render, persisted `gate_runs` row, evidence export, MCP
  read tools, and any report aggregation — and prove each one renders or
  counts an `UNBOUND` result distinctly from a `NOT_APPLICABLE` result and
  does not silently drop it. A surface that maps the new status to a
  passing or absent value is the same defect in a second place.
- [x] 3.2 Replay the 6 binding repositories and prove no status changes and
  no manifest edit is needed, including the heavy `sh -c` command strings
  in `therapist-commons` and `vowra` and the 29-command `openpanel`
  manifest. Prove the in-process `source-file-size` concern still runs and
  is never reported `UNBOUND`.
- [x] 3.3 Prove the rule cannot be escaped by renaming: an invented or
  single-repo check kind with no command produces `UNBOUND` under the same
  rule, and a check selected `false` produces no result and no `UNBOUND`.
  Confirm the open check-id vocabulary is unchanged by this package.
- [x] 3.4 Re-run the 1.4 inventory against the implemented binary and
  confirm the reported set matches the measured debt exactly — no extra
  repo red, no repo missed, and the 41 product-profile repos are counted
  with their two inherited concerns.
- [x] 3.5 Confirm the unchanged surfaces still hold: `driftwatch check`, the
  checker protocol, the AI review path and its missing-provider policy, the
  foreign-runtime no-op, `gate.toml` precedence over `.ai-gate/gate.yaml`,
  and the rule that a `commands` key must reference a selected check.

## 4. Verification

- [x] 4.1 Write the companion delta for `generic-gate-contract` —
  `Normalize gate results` and `Aggregate gate results` — as a second spec
  file in this change, covering the new status variant and the
  unconditional blocking of a required unbound check. This package's
  single `gate-project-configuration` delta does not modify that
  capability, and the change must not be archived without it.
- [x] 4.2 **CARRIED, NOT ANSWERED — the owner's answers to open decisions 1-6
  were never given, and this change does not invent them.** Task 4.2 asked for the
  `binding_policy` default's flip point, whether the new status blocks from the
  first release or after a shadow cycle, whether a profile default may be
  exempted, whether exemptions are capped, the fleet migration's granularity,
  and status variant versus a `provenance` field. **Only decision 4 was
  answered** (owner, 2026-10-02: cap at `min(floor(0.25 × required_checks), 5)`
  with four never-exempable spine checks), and it is implemented in group 6.
  Decision 3 has a shipped *disposition* — allowed, with a recorded reason —
  which is an implementation choice made to keep the package shippable, not an
  owner ruling. **Decisions 1, 2, 5 and 6 remain unanswered**, and nothing in
  this change, in the promoted spec, or in code implements an assumed answer to
  any of them: `binding_policy` still defaults to `report`, the required
  unbound check blocks from the first release, and `UNBOUND` is a distinct
  status. All six questions are carried verbatim **with their options** as
  `workspace-fleet-check-sweep` `design.md` §4, and as its items **0.1-0.5**
  (origin box: this one), so one pass of owner input closes them. One box of
  them, 0.6, is the re-measurement that has to precede the rest.
  **The decision set is unanswered at the time of this archive, and the archive
  says so rather than closing the question.**
- [x] 4.3 Run `cargo fmt --all -- --check`, `cargo clippy --all-targets`
  with no new findings, the focused gate test suite, `git diff --check`,
  and `openspec validate --all --strict --no-interactive`. Capture the
  outputs verbatim; do not summarise a run that was not executed.
- [x] 4.4 Record the honest verification gaps: which surfaces were not
  replayed, whether the phase-0 shadow cycle has actually run, and
  confirm that no test in this change asserts a check *passed* — the only
  success condition is a truthful red.

## 6. DFS — Amendment: the per-repository exemption cap (owner decision 2026-10-02)

Spec-first: the `gate-project-configuration` and `generic-gate-contract`
deltas were amended before any code. Rationale and counting rules are
recorded in `design.md` §3.5; the requirement is normative in
`specs/gate-project-configuration/spec.md`.

- [x] 6.1 Amend the spec before the code: add the normative requirement
  *"Cap the exemptions one project may declare"* with WHEN/THEN scenarios
  for below the cap, exactly at the cap, over the cap, a non-exempable
  concern, a small manifest, a large manifest, a project with no
  exemptions, and the dry-run audit; and amend the two
  `generic-gate-contract` requirements (`Normalize gate results`,
  `Aggregate gate results`) with the budget result's status/severity
  mapping and its blocking behaviour. Evidence:
  `openspec validate gate-unbound-check-fails --strict` passes.
- [x] 6.2 Compute the allowance in the same pure resolve pass that
  already computes `unbound` and `exemptions`:
  `exemption_cap(required) = min(required / 4, 5)` in `manifest.rs`,
  counted from `selected_without_triggers` so triggers cannot move it,
  plus `ExemptionBudget` (required, cap, declared, non-exemptable ids)
  and its four verdicts. Carried on `ResolvedGatePlan` and rendered by
  `render_plan`, so `--dry-run` names the budget before anything
  executes. Evidence: unit tests in group 7.
- [x] 6.3 Make the four spine concerns unexempable, under every spelling the
  workspace uses. An exemption naming `build`, `tests`, `security`,
  `repository`, or a workspace alias of one of them — `repository_integrity`
  for repository-integrity, used by `labrys` and `openpanel` — is a hard
  `FAIL` whatever the count: the plan-level `exemption-budget` result is added
  to the effective plan as a **required** check, so the verdict blocks
  independently of `blocking:` and of whether the exempted concern was
  itself marked `required`, and the exempted check's own per-check result
  becomes `FAIL`/`Error` with the `non-exempable-concern` rule so no
  surface records it as an acknowledged `NOT_APPLICABLE`. Matching walks an
  explicit alias table (`NON_EXEMPTABLE_ALIASES`) keyed by concern, so a
  future spelling is a deliberate edit rather than an id someone appends, and
  diagnostics keep naming the four concerns. Evidence: task 6.9.
- [x] 6.4 Map the verdicts onto existing statuses rather than adding
  new ones: at the cap → `REVIEW_REQUIRED` + `Warning` on an **optional**
  plan check (recorded, never blocking); over the cap or a spine
  violation → `FAIL` + `Error` on a required plan check (blocking).
  Existing aggregate, persistence, human/JSON render, and evidence paths
  therefore carry the verdict with no new consumer. The JSON envelope
  additionally carries the `exemption_budget` object.
- [x] 6.5 Count every declaration against the budget, including one on an
  optional check, so `required = false` is not a one-character bypass;
  and do not let the budget become a parse-time refusal, so the
  over-cap state is still queryable from a persisted run row exactly
  like the unbound state. Evidence: unit and end-to-end tests.
- [x] 6.6 Re-audit every surface that reads a manifest or a result for
  the new state: both manifest forms (`.ai-gate/gate.yaml` and
  `gate.toml`) resolve through the same budget, the plan render, the
  human render, the JSON envelope, and the persisted
  `gate_runs.results_json`. No exhaustive match needed a new arm,
  because no new status was introduced.
- [x] 6.7 Update the fixtures that the new rule supersedes, without
  weakening any assertion: five exemption fixtures in
  `tests/gate_unbound_check.rs` and one in `src/commands/gate.rs` now
  declare enough required checks to hold an allowance. Every assertion
  is unchanged; the exemptions that those tests are about are now inside
  a budget instead of outside one. No test was deleted.
- [x] 6.8 Document the rule where an operator reads it: the
  `not_applicable` bullet in `README.md` now states the allowance, the
  warning, the failure, the four never-exempable concerns, and that the spine
  is matched by concern rather than by spelling.
- [x] 6.9 **Defect fix, 2026-10-02 (independent verification).** The first
  implementation of 6.3 compared the exemption id against a flat list of the
  four ids the ruling named. A fleet scan found that repository-integrity is
  spelled `repository` in 63 manifests but `repository_integrity` in 2
  (`labrys`, `openpanel`), with no third spelling — leaving the spine
  protectable-under-a-different-name, which is the escape the cap was
  written to close. Fixed by matching an explicit alias table keyed by
  concern, so every workspace spelling of a non-exempable concern is
  protected and a future spelling is a deliberate edit. The rule was
  imprecise, not the audit. Spec (`gate-project-configuration` plus the
  `generic-gate-contract` delta), `design.md` §3.5, §5, §8, the proposal,
  and this file were amended with it. Evidence: the alias unit tests, the
  extended `a_spine_check_may_never_be_exempted` loop, and
  `the_repository_alias_is_not_exemptable_even_when_it_is_the_only_declaration`.

  **Severity, measured rather than assumed.** The hole was **latent, not
  live**, and that correction matters. Both alias repos bind *every*
  required check — `openpanel` and `labrys` each bind
  `repository_integrity: "git diff --check"` and report zero
  required-unbound checks — and exempting a *bound* check is already a
  parse-time `ExemptionConflictsWithCommand` error, confirmed by running
  the real manifests. So no manifest in the workspace could have exploited
  this today. The fix is still required: a rule that is only safe because
  of a coincidence about today's manifests is not a rule, and the first
  manifest to declare the alias selected-but-unbound would have opened it.

  **Proof against the real manifests** (copies in a scratch directory; no
  `gate.yaml` in any fleet repo was touched): with each repo's *own*
  `repository_integrity` left selected but unbound and then exempted, both
  are refused by the spine while sitting **inside** their numeric
  allowance, so nothing but the spine can be the cause — `openpanel`:
  `exemption budget: 1 of 5 allowances used` then `verdict: build, tests,
  security, repository may never be exempted: not_applicable names
  repository_integrity`; `labrys`: `1 of 2 allowances used` with the same
  verdict.

## 7. Verification — the cap, proved (group 6)

- [x] 7.1 Prove the arithmetic at the boundaries with real numbers
  (unit test `exemption_cap_matches_the_owner_formula_at_the_boundaries`,
  plus the same constants asserted in the end-to-end tests):
  **10 required → 2**; **20 → 5**; **24 → 5** (the absolute cap binds,
  since 0.25 × 24 = 6); **40 → 5** (the percentage would allow 10).
  Ends: **3 → 0**, **0 → 0**, **4 → 1**, **19 → 4**.
- [x] 7.2 Prove the counting rules: the denominator is the manifest's own
  required-check count, a trigger cannot move it, an optional check's
  exemption still spends budget, and an exempted check still counts in the
  denominator because an exemption never deselects.
- [x] 7.3 Prove the verdicts end to end
  (`tests/gate_exemption_cap.rs`, 14 tests): below the cap → no budget
  result; exactly at the cap → `REVIEW_REQUIRED`/`Warning`, run `PASS`;
  over the cap → `FAIL`/`Error`, run `FAIL` and blocked, also with
  `review_required_blocks = false`; each of the four spine ids **and the
  `repository_integrity` alias** → hard `FAIL` and never `NOT_APPLICABLE`;
  an *optional* spine exemption → still `FAIL`; 3 required checks →
  allowance 0 and one exemption over it; 40 required → allowance 5, five warn
  and six fail; no exemptions → the run is unchanged and no budget result
  exists; a project that names its own `exemption-budget` check cannot hide
  its own verdict behind the id; both the JSON envelope and the `--dry-run`
  plan carry the budget, and the dry run still persists nothing; the YAML
  form honours the same rule.
- [x] 7.4 Measure the fleet consequence, read-only, via `gate --dry-run`
  over all 76 real manifests. **Result below, and it contradicts the
  owner's stated premise** — the fleet is not made of 10-25-check repos.

**Measured fleet impact of the cap (task 7.4).** 76 manifests; 65 resolve,
11 fail to parse on the pre-existing unknown-field defect. Required-check
counts: min 3, **median 7**, max 29. Resulting allowances:

| Allowance | Repos | Which |
|---|---|---|
| 0 | 2 | `kairosa`, `retakeKit` (3 required checks each) |
| 1 | 60 | the majority (38 repos declare 7 required checks, 16 declare 4) |
| 2 | 2 | `labrys` (8), `vowra` (10) |
| 5 | 1 | `openpanel` (29) |

The owner's premise was "roughly 10–25 checks, so this yields a hole of
2–5". The measured fleet is smaller than that: **60 of 65 repos get a hole
of 1, and 2 get a hole of 0.** The formula is implemented exactly as
decided; the distribution is reported, not adjusted. What the evidence now
shows is that **the spine is the mechanism that actually closes the escape**
and the cap is a secondary bound on the non-spine remainder: with a median of
7 required checks, a typical repo could reach green with two exemptions if
the spine did not exist, and cannot reach green at all because four of its
six unbound checks are spine.

What the cap changes versus the unbound check alone:

- **No repo's verdict changes today.** 0 of 76 manifests declare
  `not_applicable` (the key did not exist before this change), so every
  budget is `Within` and no repo gains a budget finding. The red count is
  the same 59 of 65 as the unbound rule alone. This is measured, not
  assumed.
- **The escape is closed, permanently.** 0 repos can reach a green run by
  exempting their debt. All 59 repos with debt carry **spine** debt —
  unbound `build`, `tests`, `security`, or a repository spelling — which no
  manifest can ever exempt. The typical repo (7 required, 6 unbound: 4
  spine + 2 others, allowance 1) can spend its one allowance on one
  non-spine check and still ends up with **5 red checks**, down from 6
  unbound but never zero.
- **The 6 binding repos are unaffected**: 0 unbound checks each, and no
  exemption declared, so no budget result and no status change.
- Nothing in the fleet was edited. Newest `gate.yaml` mtime across the
  workspace is 2026-09-30; `git status` in every fleet repo shows no
  modified or new `.ai-gate` content from this work. The gate is
  stricter on purpose and the consequence is reported rather than
  suppressed.

**Gaps, stated honestly (cap amendment).**

- The owner's stated size premise does not match the measured fleet (see
  above). Implemented as decided; if a 1-check hole is too tight for the
  workspace's real shape, that is a second decision, not something this
  amendment may quietly re-tune.
- The spine is matched by concern through an alias table, so the two
  workspace spellings of repository-integrity are both protected (task
  6.9). What remains open is a *future* spelling: a manifest that renames a
  spine concern to something not in `NON_EXEMPTABLE_ALIASES` would be
  exempable, and closing that generally would mean a closed concern-id
  vocabulary, which this change explicitly does not adopt (proposal
  §Non-goals). The deliberate-edit requirement is the mitigation, not a fix.
- The cap is verified against fixtures and against `--dry-run` plans, not
  against a full executed gate run on a real repo: no fleet repo was
  executed, so the end-to-end *persisted* budget result is proven on
  temporary projects only.

## Verification record

Commands run from the repository root, with outputs captured verbatim.

| Command | Result |
| --- | --- |
| `cargo fmt --all -- --check` | clean, no diff |
| `cargo build` | Finished `dev` profile |
| `cargo test --workspace --all-targets` | every Rust test target green (593 lib + 190 integration); the `packaging` target fails on `installer` — reproduced identically on a pristine `HEAD` tree, so it is pre-existing and environmental |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | clean, no findings |
| `git diff --check` | clean |
| `openspec validate gate-unbound-check-fails --strict` | `Change 'gate-unbound-check-fails' is valid` |
| `openspec validate --all --strict --no-interactive` | `Totals: 32 passed, 0 failed (32 items)` |
| `node scripts/check-openspec-change-names.mjs` | `OpenSpec change names valid` |
| `python3 scripts/workspace_check.py --self-check` (workspace-governance) | `Summary: errors=0 warnings=0` |

**Verification record — cap amendment, 2026-10-02.** Re-run in full after
the amendment; outputs verbatim.

| Command | Result |
| --- | --- |
| `cargo fmt --all -- --check` | clean, no diff |
| `cargo build` | `Finished \`dev\` profile` |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | `Checking driftwatchdog v0.1.0` → `Finished`, no findings (re-checked after `cargo clean -p driftwatchdog` so it was a real compile, not a cache hit) |
| `cargo test --workspace --lib` | `test result: ok. 602 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out` |
| `cargo test --workspace --tests` | 20 targets, **794 passed, 1 failed**; the single failure is `packaging` (see below) |
| `cargo test --test packaging` | `test result: FAILED. 0 passed; 1 failed` on `packaging_bash_and_node_suites_pass`, `FAIL  installer`, `unexpected installed path: /tmp/tmp.…/driftwatchdog` |
| `openspec validate gate-unbound-check-fails --strict` | `Change 'gate-unbound-check-fails' is valid` |
| `openspec validate --all --strict --no-interactive` | `Totals: 32 passed, 0 failed (32 items)` |
| `openspec validate --changes --strict --no-interactive` | `Totals: 1 passed, 0 failed (1 items)` |
| `node scripts/check-openspec-change-names.mjs` | `OpenSpec change names valid: /home/paul/code/driftwatchdog/openspec/changes` |

**The `packaging` failure is pre-existing, not caused by this work.** It was
reproduced on a pristine `git archive HEAD` tree in a separate directory:
the same test, the same sub-suite, and the same message
(`FAIL  installer` / `unexpected installed path: …`). The change touches no
packaging input, so the failure is reported rather than fixed.

**Measured phase-0 / phase-1 inventory (tasks 1.4, 3.4).** 76 workspace
`.ai-gate/gate.yaml` files; 70 declare no `commands:` block; 6 bind commands.
Of the 65 that parse today, 59 have at least one required unbound check and 6
have none — and those 6 are exactly the 6 binding repositories
(`byok-translator`, `careBridge`, `labrys`, `openpanel`,
`therapist-commons`, `vowra`), so none of them needs a manifest edit. The
remaining 11 manifests fail to parse on a pre-existing unknown field
(`notes`, `evidence_boundary`, or `status`) that was never in `YAML_FIELDS`;
10 of them carry the same unbound debt by inspection, so the unparseable
debt is 69, matching the model-based count. Per-check required-unbound
totals over the parseable set: 59 `security`, 58 `tests`, 58 `build`,
55 `repository`, 38 `product-code-boundary`, 38 `placeholder-threshold`. The
1.4 oracle figures (68 / 69 / 66 / 61 and 41 / 41) are the model-based counts
over all 76 manifests and are reproduced exactly; the per-check figures above
are lower only because 11 manifests cannot be parsed by the binary yet.

**Gaps, stated honestly.**

- No repo was executed. Every fleet number comes from `gate --dry-run`, which
  returns before the database is opened, so no sibling repo was written to and
  no project command was run. The 6 binding repositories are therefore proven
  free of required-unbound checks but *not* re-proven green end to end.
- The phase-0 shadow cycle has **not** run. Blocking is unconditional from this
  release, so the ~69-repo red flood arrives with the release rather than
  after a silent cycle. The owner must accept that deliberately (decision 2).
- `cargo deny check` and `cargo audit` were not run: neither `cargo-deny` nor
  `cargo-audit` is installed on this host. The change adds no dependency, so
  the advisory surface is unchanged, but the license/advisory gates are
  unverified here and CI must be the confirming evidence.
- The shared governance check was run in its read-only form
  (`workspace_check.py --self-check`). The workspace-wide `--root .. --audit`
  form was deliberately **not** run because it writes `.project.json` into
  every audited repository, which is outside this change's scope; it should be
  run by the owner at completion.
- No test in this change asserts that any workspace repository passed. The only
  success conditions asserted are a truthful red (`FAIL` + `UNBOUND` +
  `blocked`), the 6 unaffected binding repositories' manifests parsing with
  zero unbound checks, and `source-file-size` continuing to run in-process.
- Three existing assertions were updated because the spec deliberately
  replaces the state they pinned, not to make anything pass:
  `tests/gate_deployable.rs` expected `REVIEW_REQUIRED` for an unbound
  required pair, and `tests/gate_product_quality.rs` /
  `tests/gate_release_evidence.rs` expected `NOT_APPLICABLE` for an *optional*
  command-free check. All three now assert the new state and additionally
  assert it is not the old one. No test was weakened or removed.

## 5. Explicit deferred work (moved, not dropped — `workspace-fleet-check-sweep`)

Every item below was fleet-scale from the start, and each is now **carried, not
done**: marked `[x]` only in the sense that this change stopped owning it, with
the text saying so. The work lives in `workspace-fleet-check-sweep`, unstarted
and unarchived, itemised per owner.

- [x] 5.1 **CARRIED, NOT DONE — binding commands for the 70 unbound
  repositories.** The phase-1 fleet migration: 70 language-specific changes
  across Node, Rust, .NET and Python. Measured starting debt over all 76
  manifests: 69 unbound `security`, 68 unbound `tests`, 66 unbound `build`,
  61 unbound `repository`, plus the 41 repositories inheriting unbound
  `product-code-boundary` and `placeholder-threshold` from the built-in
  product profiles. It stays blocked on decisions 1 and 2 (4.2 above) and is
  now `workspace-fleet-check-sweep` item **3.1**, unstarted. **Bind, do not
  exempt**: with a median of 7 required checks the cap yields an allowance of
  1 for 60 of 65 parseable repositories, and all 59 repositories carrying debt
  carry *spine* debt, so a repository that exempts instead of binding still
  ends red.
- [x] 5.2 **CARRIED, NOT DONE — a uniform `make gate` entrypoint.** Only 7 of
  81 repositories have a Makefile and driftwatchdog executes language-neutral
  `sh -c` strings, so mandating `make` would exclude the `npm run` /
  `dotnet test` / `msbuild` stacks or force a shim into 70 repositories before
  any gate can run. The partial answer already ships: every unbound check must
  name its own real command. Now `workspace-fleet-check-sweep` item **3.2**,
  unstarted.
- [x] 5.3 **CARRIED, NOT DONE — a closed concern-id vocabulary.** 34 check
  kinds are declared by exactly one repository, `accessibility` by 4, `format`
  by 3, `lint` by 2, and every one of those is unbound. Closing the vocabulary
  is a breaking manifest change for ~40 repositories; until then unknown kinds
  are handled by the same fail-closed rule, so an invented check name cannot
  escape the ruling. This is also what would close the residual spine hole —
  a future spelling of a non-exempable concern is protected today only if
  someone adds it to `NON_EXEMPTABLE_ALIASES` deliberately. Now
  `workspace-fleet-check-sweep` item **3.3**, unstarted.
- [x] 5.4 **CARRIED, NOT DONE — CI wiring that runs tests and the gate
  together.** 57 repositories run tests in CI and 0 run both. Owner:
  `workspace-governance`; an integration concern, not a driftwatchdog change.
  Now `workspace-fleet-check-sweep` item **3.4**, unstarted.
- [x] 5.5 **CARRIED, NOT DONE — a workspace-level exemption budget or coverage
  metric across repositories.** Partly superseded by the 2026-10-02 decision
  (group 6): the per-repository ceiling and the never-exempable spine are
  normative and implemented, which closes "a repo exempts its entire gate and
  goes green". What remains is the cross-repo view — how much of the
  workspace's total coverage is exempted in aggregate — which is still a
  `workspace-governance` concern. A per-repo override of the cap is
  deliberately not offered anywhere: it would restore the self-authorisation
  the cap removes. Now `workspace-fleet-check-sweep` items **3.5** and **3.6**,
  unstarted.
- [x] **This change is archived** (2026-10-13), carrying the runtime semantics
  this repository owns and nothing else. Archiving authorised it, and it
  required 4.2 and 5.1-5.5 to be transferred first, each box rewritten to say
  it is carried rather than done — no box was deleted, and no fleet-scale work
  is represented as complete. The residue lives in `workspace-fleet-check-sweep`,
  which stays open and unarchived, with decisions 1-6 still unanswered.
