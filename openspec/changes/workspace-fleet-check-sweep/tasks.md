# Tasks: workspace-fleet-check-sweep

## Phase 1 — BFS: the precondition, and who owns each item

Every box in phases 2 and 3 is **unstarted**, and every one names the change and
box it came from. None of them can be checked by work inside driftwatchdog: each
needs an owner decision from the operator, an action by a sibling repository's
owner, or a change in `workspace-governance`. See `proposal.md` (precondition) and
`design.md` §4 (open questions 1-6).

Counts are **measured debt on the date stated**, not current truth. Re-measure
before acting: the fleet moves, and a stale count that is acted on is the same
failure as an unchecked box.

### Group 0 — Precondition: the owner's answers

Nothing in group 3 starts before this group closes. Decisions 1, 2, 5 and 6 are
unanswered; decision 3 has a shipped disposition but no owner ruling; decision 4
is answered and implemented.

- [x] **0.1 Record the owner's answer to open decision 1** — **(a) at
  migration completion.** `binding_policy` defaults to `reject` only once no
  manifest in the fleet still needs `report`; flipping earlier accepts red the
  fleet has not finished migrating for, and per-repository flipping (c) leaves
  the default incoherent across the workspace mid-migration. Origin:
  `gate-unbound-check-fails` task 4.2 / `design.md` §8 decision 1. Decided by the
  operator on 2026-10-13.
- [x] **0.2 Record the owner's answer to open decision 2** — **(b) one
  non-blocking shadow cycle, reviewed from persisted run rows, then block.**
  Blocking already ships and already turned 59 of the 65 parseable manifests red
  (2026-10-02), which is a migration cost being paid as breakage rather than as
  evidence. A shadow cycle turns that same run history into the inventory the
  migration needs, and the block then lands on a fleet that is already
  green-or-explained. **(c) was rejected**: blocking on spine concerns first
  still reds the repositories whose spine is unbound, which is most of them.
  (a) — the shipped behaviour — is what the shadow cycle replaces. Origin:
  `gate-unbound-check-fails` task 4.2 / §8 decision 2. Decided 2026-10-13.
- [x] **0.3 Record the owner's answer to open decision 3** — **(a) allowed with
  a recorded reason**, ratifying the shipped disposition. (b) was rejected: it
  forces the 41 product-profile repositories to re-declare every inherited
  concern in each manifest, trading exemption honesty for manifest verbosity
  across the fleet. Decision 3.4 already guarantees an exempting and an unbound
  concern can never be merged into one status, so the conflation (c) exists to
  prevent is already prevented. Origin: `gate-unbound-check-fails` task 4.2 /
  §8 decision 3 and §8.1.3. Decided 2026-10-13.
- [x] **0.4 Record the owner's answer to open decision 5** — **(b) one
  workspace change with per-repository boxes**, this change's own shape. The
  archived design assumed per-repository changes and this change is itself the
  counter-example: 81 repositories of largely mechanical binding work does not
  review as 81 changes. Origin: `gate-unbound-check-fails` task 4.2 / §8
  decision 5. Decided 2026-10-13.
- [x] **0.5 Record the owner's answer to open decision 6** — **(a) the distinct
  `UNBOUND` status**, which ships. The compatibility motive behind (b) — a
  downstream consumer that keys on a closed status set — is already met by
  decision 3.4, which guarantees `NOT_APPLICABLE` and `UNBOUND` can never be
  merged into one status. (b) is also not a drop-in change: aggregation and
  persistence behaviour differ, and a consumer that ignores the field would
  silently lose the distinction. Origin: `gate-unbound-check-fails` task 4.2 /
  §8 decision 6. Decided 2026-10-13.
- [ ] **0.6 Re-measure the fleet before group 3 starts** — manifest count,
  parse failures, binding count, unbound debt per concern — from the files, not
  from these numbers. The counts in this file are dated.

---

## Phase 2 — DFS: the carried work, per owner

Each box below is one owner's action in one repository. They are grouped by the
change that recorded them, and each is unstarted.

### Group 1 — Carried from `per-check-gate-timeout` (archived 2026-10-13)

- [ ] **1.1 Fix the manifest-form `source`-label digest instability.** The YAML
  converter stamps `source = "manifest"` on every check it builds; the native
  form leaves `source` absent, so the two forms have never produced
  byte-identical canonical manifests and their `manifest_digest` values differ
  for identical declarations. Pre-existing, found by that change, not fixed by
  it, and **deliberately not fixed there**: the change's own digest-stability
  guarantee depends on the canonical TOML staying byte-identical, so this is a
  separate, reviewed runtime change with its own migration story for existing
  digests — **and implementing it moves the digest of every `.ai-gate/gate.yaml`
  manifest**, because the label it drops is one the converter wrote into all of
  them. That migration is part of this item, not a follow-up. Owner:
  driftwatchdog, separately authorized. This is the one item the change's single
  delta (`Both manifest forms declare one canonical manifest`) proposes, and the
  delta must not be promoted before the code lands. Origin: `per-check-gate-timeout`
  Phase 4, *"UNRESOLVED — the two manifest forms label a check's `source`
  differently, and this change did not fix it."*
- [ ] **1.2 Sweep the 76 workspace manifests to declare their own timeouts.**
  The field shipped; each repository's value is that repository's call, measured
  on its own machine. Origin: `per-check-gate-timeout` Phase 4, *"UNRESOLVED —
  the 76 manifests are not swept to declare their own timeouts."* Sub-items,
  per repository and per owner:
  - [ ] **1.2a** `fotofy` UnitTests, measured 69s — declare a value above 69s.
  - [ ] **1.2b** `openaccounting`, measured 143s — declare a value above 143s.
  - [ ] **1.2c** `rust-ecommerce`, measured ~10min — declare a value above it.
  - [ ] **1.2d** `alltools-platform`, measured >15min — declare a value above
    it. (Its declared verification command is separately wrong: it declares
    `cargo test --workspace` in a repository with no `Cargo.toml` at its root.
    That is the repository's to fix.)
  - [ ] **1.2e** `cvunify` (55s, 8% headroom) and `labrys` (58.5s, 2% headroom)
    — declare a value with real margin. Both pass today only because the
    measuring machine was warm; neither is proven.
  - [ ] **1.2f** The remaining manifests, so the sweep is a sweep and not four
    exceptions. Each value is the repository's own measurement.
  - **Not closeable by a generator writing a placeholder.** A placeholder
    deadline is a false claim about a repository's coverage, in exactly the way
  the single hardcoded constant was (`design.md` §1).

---

### Group 2 — Carried from `bind-declared-verification-command` (archived 2026-10-13)

- [ ] **2.1 Resolve the 11 unparseable manifests.** Each carries a top-level key
  outside `YAML_FIELDS` and is refused before any check resolves:
  `notes:` in `actoria`, `argoscope`, `crossalheart`, `dharmatlas`, `hookit`,
  `kairion`, `workspace-governance`; `status:` in `citylens` and
  `openTeacherKit`; `evidence_boundary:` in `campusMesh` and `trailCrew`. Their
  generated `declared-verification` bindings are **written and inert** until the
  field is resolved. Resolving it means deleting or relocating prose the owning
  repository wrote — for `actoria`, a note explaining that the Gate runtime is
  not installed in that checkout — which is that owner's decision about their
  own content. Origin: `bind-declared-verification-command` Phase 0, *"UNRESOLVED
  — the 11 unparseable manifests are not fixed here."*
- [ ] **2.2 Write to the 4 dirty repositories once their owners clear them.**
  `crossalheart`, `dharmatlas`, `hookit`, `trailCrew` each reported
  `?? .ai-gate/gate.yaml` at write time. The generator's default refuses to write
  into a repository dirty in a file it must change; that refusal is the
  documented behaviour, not an oversight. **Disclosure carried forward:** a
  measurement harness in the original change called the generator with the
  clean-tree guard disabled to isolate idempotence from dirtiness, so those 4
  were written anyway (76 bound, not 66). The files are untracked, so no
  tracked history was rewritten, but the declared write set and the actual write
  set differ and a re-run must reconcile them deliberately. Origin:
  `bind-declared-verification-command` Phase 0, *"UNRESOLVED — the 4 dirty
  repositories are not written to."*
- [ ] **2.3 Close the 11 inert bindings.** The bindings exist and parse nothing;
  they are effective only once 2.1 lands. Tracked separately from 2.1 because
  closing them is a *consequence*, not a step: a binding cannot be called
  effective while its manifest is refused. Origin:
  `bind-declared-verification-command` Phase 4, *"UNRESOLVED — the 11 inert
  bindings and the 4 skipped repositories are not closed here."*
- [ ] **2.4 Turn the fence on.** `VERIFICATION_BINDING_AUDIT_ENABLED` ships
  `False` in `workspace-governance/scripts/workspace_check.py` — one committed
  repo-relative constant, never an environment variable, no skip flag, no
  exemption registry; the rule's level is fixed at ERROR below the fence and is
  not softened to achieve the hold. Measured after the bindings landed, `True`
  produces **5** findings, not 70: `chronicleaf`, `devloom`, `kairovia`,
  `lileililiwen-site` (`lileililiwen.github.io`), `ziban` — the repositories
  with no `.ai-gate/gate.yaml` at all. So this is a decision about **five**
  manifest-less repositories, each of which must first acquire a manifest.
  Owner: the operator, then those five repositories. Origin:
  `bind-declared-verification-command` Phase 4, *"UNRESOLVED — the fence is
  shipped `False`."*

---

### Group 3 — Carried from `gate-unbound-check-fails` (archived 2026-10-13)

The runtime semantics this group depends on are implemented and archived: a
required unbound check records `UNBOUND`/`Error` and blocks independently of the
project's `blocking:` list; `not_applicable` exemptions carry a written reason
and are capped per repository; `build`, `tests`, `security` and `repository`
(and the `repository_integrity` spelling) are never exempable. **This group is
the migration that makes those semantics bearable, and none of it is driftwatchdog
work.**

- [x] **3.0 Satisfy group 0 first.** Decisions 1, 2, 3, 5 and 6 were answered by
  the operator on 2026-10-13 and are recorded in tasks 0.1-0.5. Decision 2
  selected a shadow cycle, so **3.7 is the work that answer creates** and it must
  land before 3.1 begins flipping repositories to `reject`. Origin:
  `gate-unbound-check-fails` tasks 4.2 / 5.1.
- [ ] **3.7 Implement the shadow cycle decision 2 selected.** `UNBOUND` already
  blocks; decision 2 (b) requires it to stop blocking for exactly one full
  workspace cycle while still being **persisted on every run row**, so the
  inventory is derived from real run history rather than from a second scan.
  Add the mode, make the run row record `UNBOUND` without folding it into
  `FAIL`/`REVIEW_REQUIRED`/`NOT_APPLICABLE`, keep it out of the aggregate
  verdict while shadowing, add a test that a shadowed `UNBOUND` is persisted and
  does not block, and a test that the shadow cannot be re-entered silently.
  Shadowing must be a declared, dated window — never a silent default that
  outlives the migration.
- [ ] **3.1 Bind, exempt or relax the 70 unbound repositories.** 70
  language-specific, per-repository migrations across Node, Rust, .NET and
  Python. Measured starting debt over all 76 manifests: 69 unbound `security`,
  68 unbound `tests`, 66 unbound `build`, 61 unbound `repository`, plus the 41
  repositories on the built-in `product`/`rust-product` profiles inheriting
  unbound `product-code-boundary` and `placeholder-threshold`. **Bind rather
  than exempt**: with a median of 7 required checks the cap is an allowance of
  1 for 60 of 65 parseable repositories, and because all 59 repositories
  carrying debt carry *spine* debt, a repository that exempts instead of binding
  still ends red (`design.md` §3). Origin: `gate-unbound-check-fails` task 5.1.
- [ ] **3.2 Decide the repo entrypoint** — whether to standardise on `make gate`.
  Only 7 of 81 repositories have a Makefile and driftwatchdog executes
  language-neutral `sh -c` strings, so mandating `make` would exclude
  `npm run` / `dotnet test` / `msbuild` stacks or force a shim into 70
  repositories before any gate can run. Whatever is decided, each unbound check
  must still name its own real command, which the `UNBOUND` remediation already
  forces. Origin: `gate-unbound-check-fails` task 5.2.
- [ ] **3.3 Decide whether to close the concern-id vocabulary.** 34 check kinds
  are declared by exactly one repository; `accessibility` by 4, `format` by 3,
  `lint` by 2 — and every one of those is unbound. Closing the vocabulary is a
  breaking manifest change for ~40 repositories. Until then, unknown kinds are
  handled by the same fail-closed `UNBOUND` rule, so a repository cannot invent
  a check name to escape the ruling. This is also what would close the residual
  spine hole: a future spelling of a non-exempable concern is currently
  protected only if someone adds it to `NON_EXEMPTABLE_ALIASES` deliberately.
  Origin: `gate-unbound-check-fails` task 5.3.
- [ ] **3.4 Wire CI to run tests and the gate together.** 57 repositories run
  tests in CI and **0** run both. Owner: `workspace-governance`; it is an
  integration concern, not a driftwatchdog change. A gate that cannot go green
  is not yet wired anywhere. Origin: `gate-unbound-check-fails` task 5.4.
- [ ] **3.5 Record the workspace-level exemption coverage metric.** How much of
  the workspace's *total* coverage is exempted in aggregate, across
  repositories. The per-repository ceiling and the never-exempable spine shipped
  and are normative; the cross-repo view is what remains. A per-repo override of
  the cap is deliberately not offered anywhere: it would restore the
  self-authorisation the cap removes. Owner: `workspace-governance`. Origin:
  `gate-unbound-check-fails` task 5.5.
- [ ] **3.6 (Decision 4 follow-on, answered but not finished)** the workspace
  aggregate budget above is the open remainder of an answered decision; the
  per-repository ceiling itself is done. Origin: `gate-unbound-check-fails`
  task 5.5 and `design.md` §8 decision 4.

---

## Phase 3 — final BFS: regression review and verification before closing

No box in phases 1-2 is closed on this repository's word. This phase is the
review that decides whether the change may be closed at all, and it is what the
local gates must show before anyone considers archiving.

### Group 4 — Closing this change

- [ ] **4.1 Close this change only when the boxes above are done**, each on its
  own owner's evidence — a sibling repository's manifest diff or its own gate
  run — and each recorded with its date. A box is not closed by this repository
  asserting it is closed.
- [ ] **4.2 Promote the delta only when 1.1 is implemented.** This change ships
  one delta — *Both manifest forms declare one canonical manifest* — and it is a
  proposal for carried item 1.1. Every other item here specifies no behaviour and
  therefore proposes nothing. `openspec archive` on this change must not run
  while 1.1 is open, or the canonical spec would claim a property the runtime
  does not yet have.
- [ ] **4.3 Re-measure the fleet and re-run the local gates before closing.**
  The counts in phases 1-2 are dated, so a closing review re-derives them from
  the files: manifest count, parse failures, binding count, unbound debt per
  concern, and the `check timeouts` rendering for any repository whose owner
  declared one. Then run this repository's own gate:
  `cargo fmt --check && cargo test && cargo clippy --all-targets --all-features
  -- -D warnings && openspec validate --all --strict --no-interactive`, plus
  `node scripts/check-openspec-change-names.mjs` and `git diff --check`. CI
  repeats these and is not the first check. A closure claim without that
  re-measurement is the failure this change exists to prevent.