# Tasks: `gate-dry-run-json-plan`

BFS → DFS → final BFS, per `.ai-rules/workflow.md`.

## BFS 1 — impact map (before writing code)

- [x] 1.1 Read `AGENTS.md`, `README.md`, `ROADMAP.md`, `HANDOFF.md`, the
      archived `platform-gate-result-emission` and
      `platform-gate-result-major-adoption` packages, and Forge's
      `src/gate/{mod,model,engine}.rs` (read-only, other repository).
- [x] 1.2 Locate the root cause: `src/commands/gate.rs:121-124` returns
      before the `match args.format` dispatch at `:148-158`.
- [x] 1.3 Map the compatibility surface: `render_plan` (human dry run),
      `print_json` / `print_human` / `print_platform` (real runs),
      `Gates::insert` (the one side effect a rehearsal must not reach).
- [x] 1.4 Map the platform surface: `src/gate/platform_result.rs` and
      `src/gate/platform_result_tests.rs` are **read-only inputs** to this
      change; `--format platform` must not gain or lose a single behaviour.
- [x] 1.5 Read `unbound_required_checks` (`src/gate/manifest.rs:1128`) and
      `aggregate` (`src/gate/aggregate.rs:89`) to answer, from the code and
      not from a guess, which parts of `blocked` are knowable pre-execution.
- [x] 1.6 Confirm the eight open changes are untouched and that
      `per-check-gate-timeout` (the adjacent one) does not collide: this
      change only *reads* `ResolvedGatePlan::check_timeouts`, which already
      exists on `main`; it writes no manifest field, no binding and no
      timeout.
- [x] 1.7 Capture the pre-change baseline: `--dry-run` human, `--dry-run`
      json (human text, the defect), real `--format json` and real
      `--format human`, from `cf823dc`.

## DFS — one coherent requirement at depth

### Requirement: emit a machine-readable rehearsal document

- [x] 2.1 Add `src/gate/dry_run_plan.rs`: the versioned rehearsal contract
      constant, the document-kind constant, the planned-check row
      composition, and the three-arm `print_dry_run`.
- [x] 2.2 The `Human` arm calls `manifest::render_plan(&resolved)` and
      prints it exactly as the old branch did — byte-identical output.
- [x] 2.3 The `Platform` arm prints the same plan text and returns `0`,
      with an explicit comment naming the decision (design §5). No new
      semantic is invented for that combination.
- [x] 2.4 The `Json` arm prints the rehearsal document and returns `0`.
- [x] 2.5 Replace `src/commands/gate.rs:121-124` with one delegating call and
      add the import. **Verify the file does not grow.**

### Requirement: a rehearsal must not be readable as a verdict

- [x] 2.6 Implement the recursive deny-set as a production helper
      (`verdict_like_keys`) so the rule is executable, and use it in the
      unit test rather than duplicating the list in the test.
- [x] 2.7 `would_block` is typed so that `false` does not compile: an enum
      with `Forced` / `Undetermined`, serialized as `true` / `null`.
- [x] 2.8 `blocking_prediction.kind` is `plan-forced-block` /
      `undetermined-before-execution` — never `outcome`, never `status`.
- [x] 2.9 The blocking prediction is computed from
      `ResolvedGatePlan::unbound` and
      `ResolvedGatePlan::exemption_budget.verdict()`, i.e. from resolution
      facts only, never from a child process.
- [x] 2.10 No command text, no clock value and no revision in the document
      (design §3.4).
- [x] 2.11 Add `src/gate/dry_run_plan_tests.rs` with the unit tests
      (12-test matrix, design §10 rows 1-7 and 12).

### Requirement: a rehearsal persists nothing

- [x] 2.12 `print_dry_run` returns `EXIT_OK` for every format, so the exit
      code cannot depend on the document's contents.
- [x] 2.13 Add `tests/gate_dry_run_json.rs` (end-to-end through the binary):
      round-trip, no verdict field, byte-identical human baseline, unchanged
      real documents, no `gate_runs` row, exit `0` on a blocking plan.

## Final BFS — system-wide regression review

- [x] 3.1 `cargo fmt` clean.
- [x] 3.2 `cargo test` — full suite green, no test weakened, skipped or
      deleted.
- [x] 3.3 `cargo clippy` on default targets clean.
- [x] 3.4 `cargo build --release` succeeds.
- [x] 3.5 `git diff --stat` shows `src/gate/platform_result.rs` and
      `src/gate/platform_result_tests.rs` **unmodified**.
- [x] 3.6 Re-capture `--dry-run --format human`, real `--format json` and
      real `--format human`; diff against the `cf823dc` baseline → empty.
- [x] 3.7 Source-file-size: `src/commands/gate.rs` did not grow; the two new
      modules and the new integration test are each well under 1,000 lines.
- [x] 3.8 `openspec validate --changes --strict --no-interactive` passes.
- [x] 3.9 `node scripts/check-openspec-change-names.mjs` clean;
      `git diff --check` clean.
- [x] 3.10 Confirm the eight pre-existing open changes are unmodified
      (`git status --short openspec/changes/` shows only this change).
- [x] 3.11 Record the Forge-side consequence (design §9) in HANDOFF as a
      reported, out-of-scope follow-up, since Forge's `parse_gate_document`
      requires `blocked` + `results` and this document must have neither.
- [x] 3.12 Every `tasks.md` box is checked; archive with
      `openspec archive gate-dry-run-json-plan -y` (never `--skip-specs`);
      two commits (implementation + archive, then docs).