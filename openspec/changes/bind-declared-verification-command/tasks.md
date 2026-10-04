# Tasks: bind-declared-verification-command

BFS → DFS → BFS. Phase order is load-bearing: §2's declared code must land
before §4's rule, or the rule is emitted by a checker the gate filters to
nothing (`design.md` §4).

## Phase 0 — BFS: measure, and correct the checkpoint

- [x] **The checkpoint's manifest count is wrong; the real count is 76, not 83.**
  Measured by enumerating `.ai-gate/gate.yaml` across the 81 git repositories:
  76 exist. `devloom` carries a native `gate.toml`; `chronicleaf`, `kairovia`,
  `lileililiwen.github.io` and `ziban` carry no Gate manifest in any form. The
  81 figure is the git-repository count and is correct as such.
- [x] **The checkpoint's binding count is wrong; the real count is 6, not 7.**
  `byok-translator`, `careBridge`, `labrys`, `openpanel`, `therapist-commons`,
  `vowra`. Note `careBridge` also uses `project_commands:`, and `openpanel`
  matches `commands` only in a comment — both were checked by hand, not by grep.
- [x] **The real target set is 70, not 74.** 76 manifests − 6 that already bind
  = 70, and all 70 declare a `verification.command`. The checkpoint's 74 does
  not reconcile against either count and is superseded.
- [x] **All 81 repositories declare a non-empty `verification.command`**,
  confirmed by parsing each `.project.json` rather than grepping it.
- [x] **11 of the 76 manifests do not parse.** `actoria`, `argoscope`,
  `crossalheart`, `dharmatlas`, `hookit`, `kairion`, `workspace-governance`
  (`notes`), `citylens`, `openTeacherKit` (`status`), `campusMesh`,
  `trailCrew` (`evidence_boundary`). Measured with `driftwatchdog gate
  --dry-run`, not by eye. All 11 fall inside the 70.
- [x] **4 of the 70 are dirty in the file the change must write** and are
  excluded: `crossalheart`, `dharmatlas`, `hookit`, `trailCrew` each report
  `?? .ai-gate/gate.yaml`. Write set is 66.
- [ ] **UNRESOLVED — the 11 unparseable manifests are not fixed here.** Making a
  binding effective in one of them means deleting or relocating prose the owning
  repository wrote (for `actoria`, a note explaining that the Gate runtime is not
  installed in that checkout). That is an owner decision about that
  repository's content, not a mechanical step in this change. Left unchecked on
  purpose; the bindings are written and inert until it is decided.
- [ ] **UNRESOLVED — the 4 dirty repositories are not written to.** The workspace
  constraint is to stop for a repository that is dirty in a file the change must
  change. Named above; the owner can clear them and re-run the generator.

## Phase 1 — BFS: the contract this repository owns

- [x] **`declared-verification` added as a stable concern id** in
  `src/gate/concerns.rs`, as pure data, with no adapter of its own: it is in no
  closed vocabulary, so `src/commands/gate.rs:212` routes it to the existing
  `project-runtime` text adapter (exit 0 → `PASS`, non-zero → `FAIL`).
- [x] **The id joins the non-exemptable spine**, with one spelling and no
  alias, in `NON_EXEMPTABLE_CONCERNS` and `NON_EXEMPTABLE_ALIASES`, so no
  repository can exempt the ruling with a written reason.
- [x] **The alias-table tests are updated to the new count (5 → 6)** and the new
  id is asserted. This widens the spine; it weakens nothing.
- [x] **A test pins the dispatch**: the concern id is in no closed vocabulary,
  so it is served by the text adapter and cannot be captured by an envelope
  adapter that would `REVIEW_REQUIRED` on ordinary test output.
- [x] **`QUALITY_VERIFICATION_COMMAND_UNBOUND` added to
  `GOVERNANCE_QUALITY_DECLARED_CODES`** so the concern can see the finding the
  moment the sibling checker emits it.

## Phase 2 — DFS: the generator

- [x] **`workspace-governance/scripts/bind_verification_command.py` exists** and
  reads `verification.command` from each repository's own `.project.json`.
- [x] **It refuses rather than invents**, with a named refusal per condition:
  absent `.project.json`, unparseable JSON, absent `verification`, absent/blank
  /non-string `command`. It never synthesises, never defaults from a profile,
  never reuses another repository's command.
- [x] **It writes both halves**: `checks.declared-verification: true` *and*
  `commands.declared-verification`, because a `commands.<id>` naming an
  unselected check is a parse error and would make the binding inert.
- [x] **It is idempotent**: a second run over an already-bound repository finds
  the exact command present and writes nothing. Proven by a test that compares
  the file bytes before and after the second run.
- [x] **It refuses to overwrite a differing binding**, reporting a conflict and
  leaving the file untouched, so a hand-written command is never clobbered.
- [x] **Dry run by default**; `--apply` is required to write.
- [x] **No `rm`.** Deletion uses `Path.unlink(missing_ok=True)` and
  `shutil.rmtree(..., ignore_errors=True)`.
- [x] **Unit tests cover**: fresh insert, second-run no-op, refusal on absent
  and on unparseable `verification.command`, refusal on a blank command, refusal
  on a differing existing binding, and a command that must be written verbatim
  including its shell operators.

## Phase 3 — DFS: the drift rule

- [x] **`QUALITY_VERIFICATION_COMMAND_UNBOUND` (ERROR) is emitted** by
  `workspace_check.py` for a repository that declares a command and binds none.
- [x] **`VERIFICATION_BINDING_AUDIT_ENABLED`, a committed repo-relative constant
  mirroring `SPEC_PURPOSE_AUDIT_ENABLED`** — never an environment variable, no
  skip flag, no exemption registry, one mechanism.
- [x] **The level is fixed at ERROR below the fence** and is not softened to
  WARNING to achieve the hold.
- [x] **A test proves that flipping the constant changes the result**, in both
  directions, so the fence is not decorative.
- [x] **The rule reads the same definition of "bound" the generator writes**, so
  a repository the generator bound is not reported by the rule.

## Phase 4 — final BFS: regression review and measurement

- [x] **`cargo test` passes** with the exact before/after count recorded in
  `HANDOFF.md` (baseline 874 passed / 0 failed, 28 suites).
- [x] **`cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D
  warnings`, `git diff --check` are clean.**
- [x] **`openspec validate --changes --strict` passes.**
- [x] **The fleet is bound and the consequence measured**: the generator is run
  with `--apply` over the 66 write-set repositories, and the green / red /
  failed-to-start split across the 70 is measured from a real run and reported
  with the red repositories named. No failing command is suppressed, skipped or
  softened.
- [x] **At least three repositories' gate output is captured before and after**,
  verbatim, including this repository's own aggregate `FAIL` with 7 `UNBOUND`
  checks, which is the state this change exists to fix.
- [x] **The fleet working trees are left uncommitted** for the owner to review.
  This repository carries the two commits; the 66 generated bindings are written
  to their repositories' working trees and are **not** committed, so no
  repository's history is rewritten and no pre-existing uncommitted work is
  swept into a commit.
- [ ] **UNRESOLVED — the 11 inert bindings and the 4 skipped repositories are not
  closed here.** See Phase 0. Both are owner decisions and both are named.
- [ ] **UNRESOLVED — the fence is shipped `False`.** Turning the rule on is the
  owner's decision, and this change deliberately leaves it to them rather than
  reddening the fleet unasked. See `design.md` §5.
- [ ] **UNRESOLVED — the change is not archived.** The residual items above are
  open, and `AGENTS.md` forbids representing a change with open final-BFS tasks
  as complete.
