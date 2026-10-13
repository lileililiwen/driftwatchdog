# Tasks: Give every gate check its own declared timeout, and show it

BFS → DFS → BFS. Phase 0 is measurement that was already done and is
recorded so a later reader can re-check it rather than trust it.

**Archived 2026-10-13 with two boxes transferred, not closed.** Every box in
the last phase that this repository could own is genuinely done and verified
below. The two that could not be owned are fleet-scale: they need a sibling
repository's owner, or a reviewed runtime change of their own. They are now
marked `CARRIED, NOT DONE` and point at `workspace-fleet-check-sweep`, which
holds them as open, unstarted items. Nothing in this file claims a manifest
was swept or a digest defect was fixed.

## Phase 0 — BFS: the state being fixed, measured

- [x] **The defect is a single hardcoded constant.** `CHECK_TIMEOUT_MS =
  60_000` (`src/commands/gate.rs:56`) is its only use
  (`src/commands/gate.rs:196`), passed as `AdapterInput::timeout_ms` for
  the `sh -c <command>` run. No manifest surface reaches it.
- [x] **Warm standalone timings measured 2026-10-04** across the live
  fleet: `cvunify` 55s (8% headroom), `labrys` 58.5s (2% headroom),
  `fotofy` UnitTests 69s, `openaccounting` 143s, `rust-ecommerce` ~10min,
  `alltools-platform` >15min. Four are killed mid-run; two are one slow
  machine from a false red.
- [x] **A timeout is already distinguishable from a failure**, so the fix
  does not have to invent a status. `run_text_adapter`
  (`src/gate/adapters.rs:936`) branches on `run.timed_out` before the
  exit-code match and returns `infra_result(AdapterError::Timeout)`,
  i.e. `REVIEW_REQUIRED` plus `{tool_id}:timeout` missing evidence; the
  exit branch maps non-zero to `FAIL`. The proposal's claim verified, not
  assumed — see `design.md` §6.

## Phase 1 — BFS: the contract this repository owns

- [x] **`ManifestCheck.timeout_ms: Option<u64>`** added, `#[serde(default)]`,
  mirroring `CheckerEntry.timeout_ms` (`src/project/config.rs:81`).
  `Option` keeps the canonical TOML (and therefore `manifest_digest`)
  byte-identical for every manifest that does not declare the key.
- [x] **`MAX_CHECK_TIMEOUT_MS = 3_600_000`**, a named public constant
  beside the field it bounds, mirroring `MAX_MAX_LINES`.
- [x] **`ManifestError::InvalidCheckTimeout { id, detail }`** with an
  actionable message for both refusals (zero, above cap), so the
  remediation is in the error rather than in a lookup.
- [x] **`timeout_ms` joins `KNOWN_FIELDS`** so a typo in the native form
  gets a `did you mean` hint.
- [x] **Both refusals run in `manifest::validate` only.** No second
  validation path is added to the YAML parser.

## Phase 2 — DFS: the field through the runtime

- [x] **`src/commands/gate.rs` resolves per check** — `CHECK_TIMEOUT_MS`
  becomes the documented default constant and each `AdapterInput` is
  built from that check's resolved timeout.
- [x] **`ResolvedGatePlan.check_timeouts: BTreeMap<String, u64>`**
  (`#[serde(default)]`), populated in `resolve` for every selected check
  with the declared value or the 60s default.
- [x] **`render_plan` prints a `check timeouts:` block**, one line per
  check, so `--dry-run` shows the effective deadline before anything runs.
- [x] **The default is unchanged at 60s.** No manifest edit is required to
  keep current behaviour, and none is made.

## Phase 3 — DFS: the business YAML form

- [x] **A per-check `timeouts:` mapping** in `.ai-gate/gate.yaml`
  converts straight onto `ManifestCheck.timeout_ms`, so the YAML and
  native forms produce an identical check and the §1 rules run once for
  both.
- [x] **`timeouts` added to `YAML_FIELDS`** (top-level list) and to the
  module header's declared-schema comment.
- [x] **`timeouts.<id>` naming an unselected check is a parse error**,
  mirroring the existing `commands.<id>` rule.

## Phase 4 — final BFS: tests, regression review, measurement

- [x] **Absent `timeout_ms` resolves to 60000** on the plan and in the
  rendered dry-run.
- [x] **A declared value is carried on the plan** and rendered.
- [x] **`timeout_ms = 0` is refused** with an actionable message naming
  the check.
- [x] **Above the cap is refused**, and the message names the cap.
- [x] **The single-owner invariant is pinned by a test**: a YAML manifest
  and the equivalent native `gate.toml` resolve to the same
  `ManifestCheck.timeout_ms` and the same rendered timeout.
- [x] **Digest stability pinned**: a manifest that does not declare
  `timeout_ms` has an unchanged `manifest_digest`, so no existing
  manifest's digest moves. Unit-tested, then verified against the live
  fleet — see the Phase 4 measurement below.
- [x] **Negative control 1 — the default path**: no `timeout_ms` → 60000
  resolved, and `AdapterInput.timeout_ms` is built from the plan. The
  real 60s wall clock is not spent in a unit test; the wiring is proven
  end-to-end by control 2 through the same code path. Stated explicitly
  rather than faked.
- [x] **Negative control 2 — the field is honored**: `timeout_ms = 1000`
  against `sleep 5` really runs, really is killed, really reports
  `REVIEW_REQUIRED` with `project-runtime:timeout` missing evidence. An
  ignored field would pass instead, so this fails if the wiring breaks.
- [x] **`cargo test` passes**: baseline **886 passed / 0 failed / 28
  suites**; after this change **908 passed / 0 failed / 29 suites**
  (+22 tests, +1 suite: 9 in `manifest.rs`, 5 in `aigate.rs`, 3 in
  `commands/gate.rs`, 5 in the new `tests/gate_check_timeout.rs`). No
  pre-existing test was weakened, skipped, or filtered, and no
  `#[allow(...)]` was added.
- [x] **`cargo fmt --check`, `cargo clippy --all-targets --all-features
  -- -D warnings`, `git diff --check` are clean.**
- [x] **`openspec validate --changes --strict` passes.**
- [x] **The 76 existing manifests are verified unchanged, not assumed.**
  Measured, not argued: `driftwatch gate --dry-run` was run over every
  fleet Gate manifest with the pre-change binary and with the
  post-change binary, and the two digest lists were diffed. Result:
  **all 77 manifests (76 `.ai-gate/gate.yaml` + `devloom`'s native
  `gate.toml`) produce byte-identical `manifest_digest` values**, so
  adding the field moved no existing manifest and no manifest needs an
  edit to keep its behaviour. All 77 also still parse — none regressed.
- [x] **The timeout is visible on the real fleet surface, not only in
  tests.** `alltools-platform` (the >15min repository) now renders
  `- declared-verification: 60000ms` in its dry-run plan — the exact
  check that was being killed mid-run, with the exact deadline that
  killed it, readable before anything executes.
- [x] **The negative controls were confirmed to have teeth by sabotage.**
  Reverting the runner's per-check lookup to the hardcoded default made
  `a_declared_timeout_kills_the_command_before_it_finishes` and
  `the_business_yaml_form_honors_a_declared_timeout` fail with
  "Unexpected success" — the gate reported `PASS` because `sleep 5`
  completed under the 60s default, which is precisely the failure the
  control exists to catch. The wiring was restored and the suite re-run
  green. Recorded because a control that has never been seen to fail is
  not known to be a control.
- [x] **CARRIED, NOT DONE — the two manifest forms label a check's
  `source` differently, and this change still does not fix it.** The YAML
  converter stamps `source = "manifest"` on every check it builds
  (`src/gate/aigate.rs:387,397`); the native form leaves `source` absent
  (`ManifestCheck.source: Option<String>`, `src/gate/manifest.rs:733`). The
  two forms therefore never produced byte-identical canonical manifests, so
  their `manifest_digest` values differ for the same declarations. This is
  pre-existing and outside this change's scope (it is a digest-stability
  question, not a timeout question), and it is the reason the single-owner
  test compares the rendered *timeout* block rather than the whole plan.
  **Re-measured at archive time and still reproducible**, on two temporary
  projects declaring the same `build` + `tests` with the same commands and
  the same `timeout_ms`: both forms resolved to identical deadlines
  (`build: 60000ms`, `tests: 900000ms`) and to **different** digests —
  `sha256:c694b16555d225fe` for `.ai-gate/gate.yaml` against
  `sha256:ac4f2706e671645a` for `gate.toml`. The finding is real and is
  carried as an open, unstarted item: **`workspace-fleet-check-sweep`
  item 1.1** (origin box: this one), with the requirement it proposes and the
  digest migration its fix implies recorded there. Not fixed here, not
  claimed fixed here.
- [x] **CARRIED, NOT DONE — the 76 manifests are not swept to declare their
  own timeouts.** This change adds the field; which timeout a repository
  needs is that repository's call, measured on its own machine. The four
  repositories that cannot pass under 60s stay unproven until their owners
  declare a value. Carried as an open, unstarted item — per repository,
  each named and each measured — as **`workspace-fleet-check-sweep` items
  1.2a-1.2f** (origin box: this one), with the reason a sweep cannot be
  automated recorded in that change's `design.md` §1. No sweep is claimed
  here and no manifest was edited here.
- [x] **The change is archived** (2026-10-13), carrying only the work that is
  this repository's and is done. Archiving authorised it, and it required
  the two boxes above to be transferred first so that no fleet-scale work
  could be read as complete: both now say **carried, not done** rather than
  deleting them. The residue lives in `workspace-fleet-check-sweep`, which
  stays open and unarchived.
