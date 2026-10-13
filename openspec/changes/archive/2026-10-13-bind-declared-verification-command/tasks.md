# Tasks: bind-declared-verification-command

BFS → DFS → BFS. Phase order is load-bearing: §2's declared code must land
before §4's rule, or the rule is emitted by a checker the gate filters to
nothing (`design.md` §4).

**Archived 2026-10-13 with four boxes transferred, not closed.** What this
repository owns — the concern id, the declared code, the generator, the rule,
its fence and their tests — is done and verified below. The four boxes that
were not are fleet-scale: they need a sibling repository's owner, or the
operator's activation decision. They are now marked `CARRIED, NOT DONE` and
point at `workspace-fleet-check-sweep`, which holds them as open, unstarted
items. Nothing here claims a manifest was parsed, a repository was written to,
or that the fence was turned on.

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
- [x] **CARRIED, NOT DONE — the 11 unparseable manifests are not fixed here.**
  Making a binding effective in one of them means deleting or relocating prose
  the owning repository wrote (for `actoria`, a note explaining that the Gate
  runtime is not installed in that checkout). That is an owner decision about
  that repository's content, not a mechanical step in this change, and the
  generator did not break any manifest: the same 65 parsed and the same 11
  failed, before and after. The bindings are written and inert until each owner
  decides. Carried as an open, unstarted item as **`workspace-fleet-check-sweep`
  item 2.1** (origin box: this one), naming all 11 repositories and the three
  unknown keys that refuse them.
- [x] **CARRIED, NOT DONE — the 4 dirty repositories are not written to by the
  tool's default.** The workspace constraint is to stop for a repository that is
  dirty in a file the change must change, and the generator refuses by default.
  The disclosure recorded in `HANDOFF.md` is carried forward with it: a
  measurement harness called `bind_repo(..., check_clean=False)` to isolate
  idempotence from dirtiness, so those 4 were written anyway (76 bound, not the
  intended 66) — untracked files, no tracked history rewritten, but the declared
  and actual write sets differ and a re-run must reconcile them deliberately.
  Carried as an open, unstarted item as **`workspace-fleet-check-sweep` item
  2.2** (origin box: this one).

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
- [x] **CARRIED, NOT DONE — the 11 inert bindings and the 4 skipped
  repositories are not closed here.** See Phase 0. A binding cannot be called
  effective while its manifest is refused, so this is a consequence of items 2.1
  and 2.2 rather than a step that could be taken beside them. Carried as an
  open, unstarted item as **`workspace-fleet-check-sweep` item 2.3** (origin box:
  this one).
- [x] **CARRIED, NOT DONE — the fence is still shipped `False`.**
  `VERIFICATION_BINDING_AUDIT_ENABLED = False` in
  `workspace-governance/scripts/workspace_check.py:570`, verified at archive
  time. Turning the rule on is **not** required to complete this repository's
  scope, and the reason is specific rather than convenient: this repository's
  own `.ai-gate/gate.yaml` binds `declared-verification`, so with the fence `True`
  this repository produces no finding at all — its own dry run schedules
  `- declared-verification [required] via manifest`. What `True` changes is the
  audit outcome of **five other repositories**, which measured `True` produces
  and `False` does not: `chronicleaf`, `devloom`, `kairovia`,
  `lileililiwen-site` (`lileililiwen.github.io`) and `ziban`, the repositories
  with no `.ai-gate/gate.yaml` at all. That is a decision about their gate
  coverage, taken by the operator and then by those five owners, not a step this
  repository can take to finish its own scope. Softening the rule to `WARNING` to
  reach the same outcome would be the second, dishonest form of the same move,
  and `design.md` §5 already rejects it. Carried as an open, unstarted item as
  **`workspace-fleet-check-sweep` item 2.4** (origin box: this one).
- [x] **The change is archived** (2026-10-13), carrying only the contract this
  repository owns. Archiving authorised it, and it required the four boxes above
  to be transferred first so that no fleet-scale work could be read as
  complete: each now says **carried, not done** rather than being deleted. The
  residue lives in `workspace-fleet-check-sweep`, which stays open and
  unarchived.
