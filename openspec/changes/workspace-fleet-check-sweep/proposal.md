# Proposal: Record the fleet-scale residue of three repository-level Gate changes as one scoped, unstarted item

## Why

Three changes sat in this repository's queue with open boxes. Every open box in
all three was the same shape of work: **it cannot be done here.** Each one needs
a decision or an edit in a *different* repository, by *that* repository's owner.

Archiving them with the boxes open would leave `openspec/specs/` claiming
coverage this workspace never produced. Checking the boxes would be a lie. Both
are worse than a remainder, so the split is:

- **Archive each original carrying only the work that is genuinely this
  repository's and genuinely done** — the runtime contract, its enforcement, and
  its tests.
- **Move the fleet-scale residue here**, itemised, unstarted, and traceable back
  to the change and box it came from.

This change is that record. It is deliberately the *last* open change in the
queue rather than three vague half-changes, because every item below has the
same precondition and the same shape: a human owner, in a sibling repository,
deciding something about their own project.

## What this change is

A single, scoped, **unstarted** queue item per distinct piece of fleet work,
each carrying its origin. It specifies **no behaviour**. It claims **no**
coverage. Nothing here changes driftwatchdog's runtime.

| Origin change (archived 2026-10-13) | Residue carried here |
| --- | --- |
| `per-check-gate-timeout` | the manifest-form `source`-label digest instability; the 76-manifest timeout sweep (4 unprovable repositories, 2 with no margin) |
| `bind-declared-verification-command` | 11 unparseable manifests; 4 dirty repositories; 11 inert bindings; activating the shipped-`False` fence |
| `gate-unbound-check-fails` | owner decisions 1-6; the 70-repository binding migration; a uniform `make gate`; a closed concern-id vocabulary; CI wiring for 57 repositories; a workspace-level exemption budget |

## The precondition, stated once and applying to every box

**No box in this change can be checked by work inside driftwatchdog.** Every
item needs at least one of:

1. **An owner decision** recorded from the operator — open decisions 1-6 in
   `design.md` are unanswered, and each one gates the work behind it.
2. **A sibling repository's owner** acting in that repository, because the item
   is a decision about *their* project: their verification command, their
   timeout on their machine, their prose in their own manifest.
3. **A change in `workspace-governance`**, which owns the fleet tooling
   (`workspace_check.py`, the binders) and is not this repository.

An item is complete when a *named* owner does the thing, and the evidence is
that repository's own gate run or manifest diff — not a statement here.

## Why one change and not three

The three residues share a precondition, an owner set, and an enforcement
consequence: each one decides whether a repository's gate runs its own declared
verification. Split across three changes they would read as three independent
backlogs and would be worked in an order that cannot be honest — the bindings
cannot land before the unparseable manifests parse, and the enforcement flip
cannot land before the migration. One change makes the dependency visible:

```text
owner decisions 1-6  ──▶  unparseable manifests + dirty repos  ──▶  the 70-repo migration
                                                                             │
                                          timeout sweep (independent) ──────┤
                                          fence activation ─────────────────┤
                                                                             ▼
                                                  enforcement flip + CI wiring + fleet budget
```

## The one spec delta, and why it is not manufactured

This change carries **exactly one** delta, in
`specs/gate-project-configuration/spec.md`: *Both manifest forms declare one
canonical manifest*. It exists because one carried item — **1.1**, the
manifest-form `source`-label digest instability — is genuinely **this
repository's own work**, and a requirement is the only reviewable form a
behavioural target can have. The requirement states what the two forms must
agree on and says, in its own text, that the runtime does not satisfy it yet: an
unarchived delta is a proposal, and this one must not be promoted until 1.1 is
implemented.

The other fifteen items propose **no** behaviour, because they are not this
repository's behaviour: they are edits to other repositories' manifests,
activation decisions, and owner rulings. Manufacturing deltas for them would put
sentences in the canonical spec that nothing here implements.

**Recorded constraint:** the pinned OpenSpec CLI (v1.6.0) refuses any change
directory with no delta at all — `CHANGE_NO_DELTAS` is an unconditional error,
with no config escape. A delta-less change therefore cannot exist here at all.
The delta above is the one piece of genuine spec content the residue contains;
if it were removed, this change could not be validated and the record would have
to live outside OpenSpec, where `openspec list` would no longer show it.

## Non-goals

- **Not a runtime change.** No driftwatchdog code, manifest field, check status,
  or adapter is modified by this change. The single delta is a *proposal* for
  carried item 1.1, not a claim about shipped behaviour.
- **Not a claim of coverage.** Every count below is a *measurement of the debt*,
  dated and sourced. Re-measure before acting; the fleet moves.
- **Not authority to edit another repository.** driftwatchdog records the work.
  The owning repositories perform it.
- **Not an archive.** This change stays open until the residue is actually done,
  which is bounded by owner action rather than by effort here.