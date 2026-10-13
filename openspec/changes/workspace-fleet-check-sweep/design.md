# Design: Fleet-scale residue of three repository-level Gate changes

Change: `workspace-fleet-check-sweep`. Planning and record only; no code, no
spec delta. The reasoning below is carried forward from the `design.md` of each
origin so a future reader does not have to reopen three archives to learn why
the work is shaped the way it is.

## 1. Carried forward from `per-check-gate-timeout`

**Why the field is `Option<u64>` and the default is unchanged at 60s.**
`ResolvedGatePlan.manifest_digest` is computed over
`toml::to_string_pretty(manifest)`, and the TOML serializer omits `None`. A
defaulted `u64` would emit `timeout_ms = 60000` into every canonical document
and move the digest of all 76 existing manifests for a field none of them
declared. `Option` is what keeps "absent" and "declared to be the default"
different things. A test pins it, and it was re-verified at archive time. **A
timeout sweep must therefore never be treated as a formatting change**: adding
a declared `timeout_ms` to a manifest *does* move that manifest's digest, by
design.

**Why raising the global default was rejected.** Two different defects hid
behind the one constant: four repositories that cannot pass (`fotofy` UnitTests
69s, `openaccounting` 143s, `rust-ecommerce` ~10min, `alltools-platform`
>15min), and two that are one slow machine from a false red (`cvunify` 55s, 8%
headroom; `labrys` 58.5s, 2%). A single larger constant makes the second defect
worse in relative terms and guarantees neither repo margin. A timeout that is
wrong for every check is not a default, it is a missing field.

**Why the sweep is per-repository owner work, and why it cannot be centralised.**
The field shipped; what each repository needs is a number *measured on its own
machine*, on its own warm cache, against its own test suite. `MAX_CHECK_TIMEOUT_MS`
bounds the value at 1h but says nothing about which value is right. A sweep tool
could write a placeholder; a placeholder deadline is a false claim about a
repository's coverage in exactly the way the original constant was. **There is
no correct automated value, so there is no correct automated sweep.**

**Why the digest instability stayed in this change.** The YAML converter stamps
`source = "manifest"` on every check it builds (`src/gate/aigate.rs`) while the
native `gate.toml` form leaves `source` absent (`src/gate/manifest.rs`). The two
forms therefore have never produced byte-identical canonical manifests, so their
`manifest_digest` values differ for identical declarations. It surfaced *here*,
but it is a digest-stability question, not a timeout question, and fixing it
would move digests for real manifests — the exact invisible consequence the
`Option` design above exists to prevent. It is carried, not closed.

## 2. Carried forward from `bind-declared-verification-command`

**Why the binding is generated per repository and never templated.**
`profile_defaults` (`src/gate/manifest.rs`) selects concern *ids* and nothing
else; it never invents a command, by design. The value to bind is the string
each repository wrote into its own `.project.json`, so the writer must read the
declaration from the same repository. A template cannot do that and must not
pretend to.

**Why the generator refuses rather than invents.** Absent `.project.json`,
unparseable JSON, absent `verification`, blank or non-string `command` — each is
a named refusal, and the run continues to the next repository. It never
synthesises a command, never falls back to a profile default, and never reuses
another repository's command, because a generated command asserts a verification
the repository never declared. It also refuses to overwrite a *differing*
hand-written binding and refuses to write into a repository dirty in the file
it must change. Those guards are why the residue below is **inert**, not lost:
nothing was clobbered to get the fleet bound.

**Why the fence shipped `False`, and what has changed since.** The rule
(`QUALITY_VERIFICATION_COMMAND_UNBOUND`, ERROR, below the fence) is fully
implemented and tested, gated on one committed constant
`VERIFICATION_BINDING_AUDIT_ENABLED`. It shipped closed rather than reddening 70
repositories for reasons the change did not own. The measured position after the
bindings landed has since changed, which is why activation is *not* the +70
decision it was at the time of writing:

| fence | audit errors | `QUALITY_VERIFICATION_COMMAND_UNBOUND` findings |
| --- | --- | --- |
| `False` (shipped) | 205 | **0** |
| `True` | 210 | **5** — `chronicleaf`, `devloom`, `kairovia`, `lileililiwen-site` (`lileililiwen.github.io`), `ziban` |

The five are the repositories with **no `.ai-gate/gate.yaml` at all**
(`devloom` carries a native `gate.toml`). So flipping the fence is now a +5
decision about five manifest-less repositories, not a +70 decision. It remains a
fleet decision: it belongs to the operator and to those five owners.

**Why the 11 inert bindings cannot be forced.** Their manifests carry a top-level
`notes:`, `status:` or `evidence_boundary:` key that is outside `YAML_FIELDS`, so
the manifest is refused before any check resolves. A binding written into one is
*necessary and insufficient* — recorded, and inert. Resolving the unknown field
means deleting or relocating prose the owning repository wrote (for `actoria`,
a note explaining that the Gate runtime is not installed in that checkout). That
is an owner's decision about their own content. Deleting the note from here would
make the workspace look tidier and the repository's manifest a lie.

## 3. Carried forward from `gate-unbound-check-fails`

**Runtime semantics and fleet migration are different packages, and the
migration is the larger one.** The archived change made the runtime honest: a
required check with no command records `UNBOUND`/`Error` and blocks
independently of the project-controlled `blocking:` list, a project may declare
a written `not_applicable` exemption, and the exemption count is capped at
`min(floor(0.25 × required_checks), 5)` with `build`, `tests`, `security` and
`repository` (and the workspace's `repository_integrity` spelling) never
exempable. What it deliberately did **not** do is move any repository from
unbound to bound. That is 70 language-specific, per-repository migrations
(Node, Rust, .NET, Python) and it is where the review effort belongs.

**Why the enforcement flip is gated on owner decisions rather than on code.**
`binding_policy` ships `report` as the default for an honest reason: parse-time
rejection writes **no run row at all**, which destroys the very inventory the
migration needs. The flip date is a policy call about the fleet, not about the
runtime, and it must be decided rather than discovered by a red flood.

**Why the spine, not the cap, is what closes the escape — and why that shapes the
migration.** Measured over the 76 real manifests: required-check counts run min
3, **median 7**, max 29, not the 10-25 the owner's ruling assumed. The resulting
allowances are 1 for 60 of the 65 parseable repositories and 0 for 2. All 59
repositories carrying required-unbound debt carry **spine** debt, so 0 of them
can reach green by exempting. A migration that exempts a repo's debt is
therefore not a shortcut: with the spine in place a typical repo still ends with
5 red checks after spending its one allowance. **Bind, do not exempt** — that is
the measured shape of the work below.

**Why a future spelling is the residual hole in the spine.** The spine is matched
by concern through `NON_EXEMPTABLE_ALIASES`, so both workspace spellings of
repository-integrity are protected, and a new spelling is a deliberate edit
rather than a silent append. Closing that generally would mean a **closed
concern-id vocabulary**, which is item 3.3 below and is a breaking manifest
change for ~40 repositories. The deliberate-edit requirement is the mitigation,
not a fix, and it is recorded as one.

## 4. Questions for the operator — decisions 1-6, ANSWERED 2026-10-13

The operator delegated these to the implementing agent on 2026-10-13 and each is
answered below with its reasoning. Decision 4 was answered earlier, on
2026-10-02, and is already implemented. Decision 2's answer creates real work,
recorded as task 3.7. The options are retained verbatim below so the reasoning
can be audited against what was actually on the table.

These were **previously unanswered**. They are carried verbatim from
`gate-unbound-check-fails/design.md` §8 with their options, so one pass of owner
input can close all of them. **No answer is invented here, and nothing below is
implemented on an assumed answer.** One pass of operator input closes §4 and
unblocks groups 0 and 3.

### Decision 1 — when does `binding_policy` default to `reject`?

*Why it is open:* `report` ships as the default because parse-time rejection
writes no run row and would blind the inventory.

**ANSWERED 2026-10-13: (a)** — flip `binding_policy` to `reject` at migration completion. Flipping earlier accepts red the fleet has not finished migrating for; per-repository flipping leaves the default incoherent mid-migration.
- **(a)** Flip when the fleet migration is complete — the point at which no
  manifest needs `report` any more.
- **(b)** Flip at a named milestone or date, accepting red before the migration
  finishes.
- **(c)** Flip per repository, as each one goes green.

### Decision 2 — is `UNBOUND` blocking from day one, or shadowed for one cycle?

*Why it is open:* blocking is already implemented from the first release and 59
of the 65 parseable manifests went red with it. The question is whether that is
accepted deliberately or phased.

**ANSWERED 2026-10-13: (b)** — one non-blocking shadow cycle, reviewed from persisted run rows, then block. Blocking already turned 59 of 65 parseable manifests red (2026-10-02), charging that as breakage instead of as evidence. (c) still reds every repository whose spine is unbound, which is most of them. The work this creates is task 3.7.
- **(a)** Accept the immediate red (what ships today) and migrate under it.
- **(b)** Ship a shadow cycle: record `UNBOUND` without blocking for one full
  workspace cycle, review the inventory from persisted run rows, then block.
- **(c)** Block only on spine-concern `UNBOUND` in phase 1, all checks after.

### Decision 3 — may `not_applicable` exempt a check a profile requires by default?

*Why it is open:* the shipped disposition **allows** it, with a recorded reason,
because the spec accepts a `not_applicable` entry for any selected concern and a
profile default is a selected concern. That is an implementation choice made to
keep the package shippable; the owner has not ruled on it.

**ANSWERED 2026-10-13: (a)** — ratify the shipped disposition: allowed with a recorded reason. (b) would force the 41 product-profile repositories to re-declare every inherited concern. Decision 3.4 already forbids merging an exempting and an unbound concern into one status, which is what (c) exists to prevent.
- **(a)** Keep allowing it (shipped disposition), with the reason recorded.
- **(b)** A profile-inherited concern may be exempted only by an explicit
  per-check override in the manifest — stricter, and it forces the 41
  product-profile repositories to re-declare their inherited concerns.
- **(c)** A profile-inherited concern may never be exempted.

*Trade-off:* exemption honesty versus manifest verbosity.

### Decision 4 — is there a ceiling on exemptions?

**Answered by the owner on 2026-10-02 and already implemented.** Carried here for
completeness of the set, not as open work: the ceiling is
`min(floor(0.25 × required_checks), 5)` per repository, with `build`, `tests`,
`security` and `repository` (and the `repository_integrity` alias) never
exempable, a `WARN` at the ceiling and a blocking `FAIL` above it. Group 3.6
(workspace-level aggregate budget) remains open and is separate.

### Decision 5 — one workspace change, or one change per repository?

*Why it is open:* the runtime assumes per-repository changes. This very change is
the counter-example — one change with per-repository boxes — and it is a
review-process decision, not a technical one.

**ANSWERED 2026-10-13: (b)** — one workspace change with per-repository boxes, this change's own shape. 81 repositories of largely mechanical binding work does not review as 81 changes, and this change is already the counter-example to the archived per-repository assumption.
- **(a)** Per-repository changes, as the archived design assumed.
- **(b)** One workspace change with per-repository sub-tasks (this change's
  shape), reviewed as a whole.
- **(c)** Per-repository changes for the code-heavy stacks, one grouped change
  for the mechanical ones.

### Decision 6 — distinct `UNBOUND` status, or a `provenance` field on `NOT_APPLICABLE`?

*Why it is open:* the status variant ships, and is the stronger machine signal.
The field is the fallback if downstream status consumers cannot take a new value.

**ANSWERED 2026-10-13: (a)** — keep the distinct `UNBOUND` status that ships. The compatibility motive behind (b) is already met by decision 3.4; and (b) is not a drop-in change, since a consumer that ignores the new field would silently lose the distinction.
- **(a)** Distinct `UNBOUND` status (shipped; `NOT_APPLICABLE` keeps meaning
  "evaluated and inapplicable").
- **(b)** A `provenance` field on `NOT_APPLICABLE` instead, for compatibility with
  downstream consumers that key on a closed status set. This is not a drop-in
  change: aggregation and persistence behaviour differ, and any consumer that
  does not read the field silently ignores it.

## 5. Ownership boundary

| Item group | Who can complete it | What driftwatchdog may do |
| --- | --- | --- |
| 1 (timeout sweep, digest defect) | each repository's own owner; the digest defect is a driftwatchdog runtime change | design and record; fix the digest defect in this repository only when separately authorized |
| 2 (manifests, bindings, fence) | each repository's owner for 1-3; the operator for the fence | record only; the fence lives in `workspace-governance` |
| 3 (decisions, migration, `make gate`, vocabulary, CI, budget) | the operator for decisions 1-6 and the migration granularity; `workspace-governance` for CI and the workspace budget | record only |

The rule that governs this table: **a fact about another repository is measured
here and acted on there.** Nothing in this change asserts that a sibling
repository is bound, green, or exempt.

## 6. The one delta, and the constraint that forces the question

This change carries exactly one spec delta, *Both manifest forms declare one
canonical manifest*, and it maps to carried item **1.1** — the only residue item
that is driftwatchdog's own behaviour rather than another repository's content.
The requirement is written as a target and says so in its own text, because an
unarchived delta is a proposal and this one is not yet true of the runtime.

The constraint is worth recording so the next reader does not think the delta
was optional taste. The pinned CLI (v1.6.0) rejects a change directory with no
delta — `CHANGE_NO_DELTAS`, unconditional, no configuration escape — so the
real question was never "should this have a delta?" but "is there any delta that
is not manufactured?". There is one. The other fifteen items are edits to other
repositories' manifests, activation decisions and owner rulings, and a
requirement written for any of them would be a sentence in the canonical spec
that no implementation here backs. If item 1.1 is ever completed and promoted,
the delta should be archived with it; until then it must not be.