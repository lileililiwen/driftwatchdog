# Tasks: platform-gate-result-emission

Ordered BFS impact map → DFS requirement implementation → BFS regression and
completeness → verification.

## 1. BFS — Impact map and baseline

- [x] 1.1 Read the authoritative schema
  (`platform-contracts/schemas/gate-result.schema.json`) before writing any
  code, and record every constraint the emitter must assert: the four-value
  `result` and `checks[].status` enums, the `gate_id` / `pipeline` /
  `check_id` patterns and their length caps, the `evaluated_at` RFC 3339
  format, the 512-character `detail` and `evidence_refs` caps, the 256
  character `subject_ref` cap, and `additionalProperties: true` on both the
  root and the check object.
- [x] 1.2 Map every consumer of the new output value before implementing it:
  `GateArgs.format` in `src/cli.rs`, the `match args.format` in
  `src/commands/gate.rs`, `GateEvidenceExportArgs.format` (which must NOT gain
  the value), the human render, the JSON render, and `gate_runs` persistence.
  Confirm no reader of the persisted record, no MCP tool, and no doctor check
  consumes a format selector.
- [x] 1.3 Enumerate every exhaustive `match` over `GateStatus` that the new
  mapping must not disturb: `src/gate/types.rs`, `src/gate/aggregate.rs`,
  `src/gate/adapters.rs` status-to-exit and status-to-severity tables,
  `src/gate/evidence_export.rs`, and `src/commands/gate.rs`. The emitter adds
  a reader, never a new state, so none of these need a new arm.
- [x] 1.4 Record the pre-existing behaviour the change must not alter: the
  exact stdout of `--format human` and `--format json`, and the `0` / `1`
  exit codes. Evidence: the existing `src/commands/gate.rs` tests plus
  `tests/gate_unbound_check.rs`, which assert the human and JSON surfaces.

## 2. DFS — The mapping module

- [x] 2.1 Create `src/gate/platform_result.rs` with the platform status
  vocabulary and the **total** `map_status` function: `PASS` → `passed`,
  `FAIL` → `failed`, `NOT_APPLICABLE` → `skipped`, `REVIEW_REQUIRED` and
  `UNBOUND` → `Unrepresentable`. The function returns a value for all five
  states and never fails, so a future sixth state cannot be added without the
  compiler pointing here. Evidence: one unit test per row plus a
  no-state-escapes-the-table test.
- [x] 2.2 Implement the four ASCII pattern validators by hand (no regex
  dependency): `gate_id` `^[A-Za-z0-9][A-Za-z0-9._:-]{0,127}$`, `pipeline`
  `^[a-z][a-z0-9_.-]{0,127}$`, `check_id` `^[a-z][a-z0-9_.-]{0,63}$`, and the
  per-field length caps counted in **characters**, not bytes, because JSON
  Schema `maxLength` counts Unicode code points and `str::len()` counts UTF-8
  bytes. Evidence: accepted/rejected unit tests on both sides of every
  boundary, including a leading digit, a leading uppercase letter, a
  non-ASCII letter, an underscore, and a 64- versus 65-character id.
- [x] 2.3 Define the typed `Refusal` enum with one variant per refusal case
  and an ordered evaluation in `build` so the diagnostic is deterministic:
  aggregate status, check statuses, `gate_id`, `pipeline`, `subject_ref`,
  `evaluated_at`, `check_id`, `detail`, `evidence_ref`. Statuses first, so a
  substantive refusal is never masked by a cosmetic one.
- [x] 2.4 Implement `build` as a **pure** function of a `PlatformRun` input
  struct: no clock, no filesystem, no database, no manifest. It either returns
  a complete `PlatformGateResult` or a `Refusal`; it can never return a
  partial or coerced document. Evidence: every refusal unit test asserts the
  error and the absence of any document.

## 3. DFS — Field mapping decisions

- [x] 3.1 Map `gate_id` to the run's manifest digest, `evaluated_at` to the
  run's `taken_at`, `subject_ref` to the run's `revision`, and `result` to the
  aggregate status. Apply the pattern and format checks to each rather than
  trusting them.
- [x] 3.2 Map `pipeline` to the constant `driftwatchdog-gate` and carry the
  project-owned profile verbatim in the root `driftwatch_profile` additional
  property. Rationale recorded in `design.md` §4: a constant is a documented
  fact about the producer and never needs a refusal; a profile is
  project-owned text of unbounded shape that the platform pattern cannot
  express.
- [x] 3.3 Emit `checks[].check_id` **unchanged** and refuse any id that does
  not already conform. `design.md` §4 records why no lossless reversible slug
  exists inside 64 characters, and why truncation, lowercasing,
  transliteration, and fixed-escape encoding each lose or merge information.
  Because the mapping is the identity it is totally reversible; the original
  id is additionally repeated verbatim in the check object's
  `driftwatch_gate_id` additional property so the round trip is machine
  checkable.
- [x] 3.4 Map `detail` from `GateResult.diagnostic` through `redact_secrets`
  only. Never through `bound_text`, which truncates with an ellipsis — the
  silent loss this change exists to prevent. Refuse above 512 characters and
  name the check and the length.
- [x] 3.5 Map `evidence_refs` to the distinct evidence keys in first-seen
  order. Refuse an empty key or a key over 512 characters. Do not invent a
  composite `key@digest` string: the schema types the entry as an opaque
  string, and the key already resolves to the artifact in the run record.
- [x] 3.6 Refuse when **any** check is `REVIEW_REQUIRED` or `UNBOUND`, not
  only required ones. An optional pending review coerced to `skipped` would
  publish the same false claim as a required one.

## 4. DFS — CLI wiring

- [x] 4.1 Add `GateOutputFormat { Human, Json, Platform }` in `src/cli.rs`
  and use it for `GateArgs.format` only. Leave `GateFormatArg` untouched so
  `driftwatch gate evidence-export --format` keeps exactly two values and
  cannot be given `--format platform`.
- [x] 4.2 Add one `match` arm in `src/commands/gate.rs` that builds the
  document and, on success, prints it with `serde_json::to_string_pretty` and
  returns `EXIT_OK` / `EXIT_BLOCKED` exactly as the other two arms do.
- [x] 4.3 Add `EXIT_UNREPRESENTABLE: i32 = 2`, reachable only through
  `--format platform`. On refusal write nothing to stdout and write the
  two-line diagnostic from `design.md` §5 to stderr.
- [x] 4.4 Register the module in `src/gate/mod.rs` and re-export its public
  surface.
- [x] 4.5 Update the three in-repo `GateArgs` constructors in the
  `src/commands/gate.rs` tests to the new enum type. Mechanical; no
  behavioural change.

## 5. BFS — Regression and completeness review

- [x] 5.1 Prove the change is additive: `--format human` and `--format json`
  produce byte-identical stdout and the same exit codes as before. Evidence:
  the pre-existing `src/commands/gate.rs` tests and `tests/gate_unbound_check.rs`,
  all unchanged and passing.
- [x] 5.2 Prove the gate pipeline is not restructured: `execute_plan`,
  `aggregate`, the adapter tables, the manifest, and the concern vocabulary
  are untouched. Evidence: `git diff` shows changes only in the four listed
  paths plus the new module and the new test file.
- [x] 5.3 Prove no new dependency was added. Evidence: `git diff -- Cargo.toml
  Cargo.lock` is empty; the module uses only `serde`, `serde_json`, `chrono`,
  and the existing crate-internal modules.
- [x] 5.4 Prove no Forge or `platform-contracts` file was touched. Evidence:
  `git status` in `platform-contracts` is clean and this change adds no
  dependency on either.
- [x] 5.5 Confirm none of the five incomplete open changes
  (`per-check-gate-timeout`, `bind-declared-verification-command`,
  `declare-spec-purpose-placeholder-code`, `bind-governance-quality-checks`,
  `gate-unbound-check-fails`) was edited, completed, archived, or re-scoped,
  and that this change conflicts with none of them. It only reads
  `AggregateOutcome` and `GateResult`; it writes no manifest field, no
  command binding, no timeout, and no check status.
- [x] 5.6 Check every refusal case has a unit test, and that every refusal
  test asserts both that no document is produced and that the failure is
  typed, and that the end-to-end suite asserts stdout is empty with a
  non-zero exit code for the refusal paths reachable from the CLI.

## 6. Verification

- [x] 6.1 `cargo fmt`
- [x] 6.2 `cargo test`
- [x] 6.3 `cargo clippy` (default targets)
- [x] 6.4 `openspec validate --changes --strict --no-interactive`
- [x] 6.5 `git diff --check`
- [x] 6.6 `node scripts/check-openspec-change-names.mjs`

The full `driftwatch` Gate runtime, `cargo clippy --all-targets --all-features
-- -D warnings`, heavy-feature workspace tests, and container/integration
suites are deliberately **not** run here; they are the orchestrator's.

## Verification record

Run on 2026-10-12. Results are reported in `HANDOFF.md` and the change is
archived only after every box above is genuinely done.