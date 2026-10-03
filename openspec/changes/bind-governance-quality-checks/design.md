# Design: Bind the governance quality checks as real Gate concerns

## 1. Implementation boundary

The adapter lives in this repository, under `src/gate/`, beside the existing
`adapters.rs` and `aigate.rs`. It is a new concern, not a new mechanism: the
command-binding, envelope parsing, exit-code mapping and aggregation already
exist and are reused unchanged.

This change does not modify `workspace_check.py`. It reads that checker's
`--json` output, whose top-level keys are `findings` and `summary`, with `summary`
shaped `{"errors": N, "warnings": N}` and each finding carrying a `level` and a
`code`.

Language is incidental to the contract. The scenarios below are written against
the envelope, so an implementation in another language satisfies them equally —
but a Rust implementation in `src/gate/` is preferred, because the concern, the
manifest surface and the test harness all live here and a second language would
introduce a build and release surface this repository does not currently have.

## 2. Why an adapter, and not the alternatives

**Rejected: teach `workspace_check.py` to emit exit 2.** Exit 2 would mean
REVIEW_REQUIRED fleet-wide, which changes the meaning of the exit code for every
existing caller, including the four repositories whose own gates invoke it and
treat a non-zero exit as a plain failure. A shared contract is not a place to
introduce a second convention.

**Rejected: bind the raw command.** `scripts/workspace_check.py` exits 1 on the
current fleet findings, of which all but a handful are unrelated to these two
checks (`PUBLICATION_UNVERIFIED` 76, `DISCOVERABILITY_MISSING` 74,
`LICENSE_MISSING` 51, `HANDOFF_SCAFFOLD` 8, `DEPLOYMENT_PROMOTION_STALE` 7,
`DISCOVERED_UNREGISTERED` 4). The concern would report a portfolio-wide backlog
as if every repository had failed it, and the real findings would be invisible
inside the noise.

**Chosen: a code-scoped adapter.** The concern declares which finding codes it
owns. The adapter runs the checker, keeps only findings whose `code` is in that
set, and reports only those. A repository whose README claim is contradicted
fails; a repository with a missing LICENSE does not, because that is not this
concern's business.

## 3. Envelope mapping

| Matching findings | Level | Status | Exit |
|---|---|---|---|
| none | — | `PASS` | 0 |
| one or more | `WARNING` | `REVIEW_REQUIRED` | 2 |
| one or more | `ERROR` | `FAIL` | 1 |

`ERROR` dominates: a run with both an ERROR and a WARNING is `FAIL`. The mapping
is total over the two levels the governance checker emits; a third level is a
contract change and is handled as a cannot-run case, not silently treated as a
warning.

The concern reports the filtered finding count, not the checker's total. A
caller reading the evidence line must be able to tell "4 concerns matched" from
"215 findings exist, 4 of which are mine".

## 4. Scope is not exemption

The per-project code set is a **scope** decision: it declares which codes this
concern reports on. It is not a way to make a finding disappear.

- The default set ships with the concern and contains
  `QUALITY_README_CLAIM_CONTRADICTED`, `QUALITY_README_CLAIM_UNSCOPED`,
  `RULES_DRIFT_NEW` and `RULES_DRIFT_BASELINE_STALE`.
- A project MAY narrow the set for its own run.
- A project MUST NOT widen the set past the codes the concern declares, and MUST
  NOT use it to suppress a code the concern owns. A set that is empty while the
  concern is enabled is a configuration error and is reported as
  `REVIEW_REQUIRED` with a diagnostic naming the empty scope, not as `PASS`.

This is deliberately different from the exemption mechanism this repository
already ships. An exemption removes a *required* check from the gate and is
capped at `min(floor(0.25 × required), 5)`, with `build`, `tests`, `security` and
`repository` never exempable on either spelling via `NON_EXEMPTABLE_ALIASES`.
Scope and exemption answer different questions and must not be conflated.

## 5. Failure path when the checker cannot run

None of these may be `PASS`. The distinction is between "the check ran and found
something a human must look at" and "the check never ran, so nothing is known".

| Condition | Status | Exit | Why |
|---|---|---|---|
| checker file missing | `UNBOUND` | 1 | nothing is bound to run, so nothing can be claimed |
| unexpected non-zero, non-1 exit | `REVIEW_REQUIRED` | 2 | it ran and misbehaved; evidence is the exit code and stderr |
| unparseable or truncated JSON | `REVIEW_REQUIRED` | 2 | it may have found something that cannot be read |
| timeout | `REVIEW_REQUIRED` | 2 | consistent with the existing adapter policy for timeouts |
| third, unrecognised finding level | `REVIEW_REQUIRED` | 2 | the contract moved underneath us |

`UNBOUND` is reserved for "there is nothing to run". A checker that starts and
cannot finish is a review condition, and collapsing the two would let a broken
checker present as a missing one.

## 6. Ordering constraint against `gate-unbound-check-fails`

`gate-unbound-check-fails` task 5.1 is "Binding commands for the 70 unbound
repositories", and task 5.2 records that only 7 of 81 repositories have a bound
command. A concern cannot bind in a repository that has no binding surface at
all.

This change therefore lands the concern and its mapping **here**, in this
repository's own gate, and states the dependency for the fleet rather than
inheriting it silently. The 70 repositories gain the concern when 5.1 lands, not
before. Until then the concern is live for this repository and inert elsewhere,
and any report must say which is which — a concern that is enabled in the profile
but unbound in a project is `UNBOUND`, which is a real result and is reported as
one.

## 7. Blast radius

> **Superseded 2026-10-03 by `workspace-governance` `833ca9c`.** The figures
> below were measured while the claim checker reported 4 contradicted
> repositories. Those 4 were **false positives** — the READMEs were accurate and
> the evidence rules were wrong — and `833ca9c` corrected the check. The fleet
> audit is now `errors=211 warnings=18` with **zero** contradicted findings. The
> reasoning that follows still holds and is the reason this section exists; only
> the counts have moved. The corrected table is at the end.

The intended default configuration surfaces a small, named slice of the fleet
audit, and almost none of the rest:

- `QUALITY_README_CLAIM_CONTRADICTED` — was 4: `argoset:31`, `care-bridge:17`,
  `retake-kit:42`, `devloom:9`. All four were false positives and are gone.
- `QUALITY_README_CLAIM_UNSCOPED` — 1 aggregate WARNING naming 3 statements
  (`opendockify`, `platform-contracts`, `chronicleaf`).
- `RULES_DRIFT_NEW` — 0. All 140 clause-level gaps are baselined.
- `RULES_DRIFT_BASELINE_STALE` — 0. No baseline entry has become redundant.

The other **211** errors are out of scope for this concern and stay reported by
the governance audit alone. That is the whole point: a concern scoped to four
codes must not absorb a portfolio backlog, and the numbers must keep saying so
after the codes go quiet. A concern that reports zero because its subject is
clean, and a concern that reports zero because it is not wired, must be
distinguishable — which is what the failure path in §5 and `tasks.md` 4.6 are
for.

**Corrected live measurement (`833ca9c` and later), 0 of 211 errors and 1 of 18
warnings surfaced.** `driftwatchdog`'s own result is therefore `PASS` (0), which
is the correct outcome for a repository with no contradicted claim, not a
weakened check.


## 8. Decision ledger

| # | Decision | Options | Choice | Rationale |
|---|---|---|---|---|
| D1 | Where the adapter lives | this repo · `workspace-governance` · both | **this repo** | The concern, the manifest surface and the envelope contract live here. A second language in `workspace-governance` would add a build and release surface this repository does not have. |
| D2 | How findings map to the envelope | raw exit code · code-scoped adapter · teach the checker | **code-scoped adapter** | The raw exit code surfaces 211 unrelated findings; teaching the checker changes the contract for every existing caller. |
| D3 | Whether either concern is exempable | exempable · not exempable | **not exempable** | Both own an ERROR-severity code. An exemption removes a required check; here the check is required precisely because its finding is a defect. The existing cap of `min(floor(0.25 × required), 5)` and the `NON_EXEMPTABLE_ALIASES` rule stay untouched. |
| D4 | Whether `RULES_DRIFT_BASELINE_STALE` blocks | block · warn only | **block as `REVIEW_REQUIRED`** | It is not a repository defect, but it is not nothing either: a stale baseline means the ratchet is measuring against a snapshot that no longer matches the fleet. `REVIEW_REQUIRED` is already blocking via `.ai-gate/gate.yaml`, so the concern actually binds and does not become a notice. |
| D5 | Whether a project may narrow the code set | no · yes, with a floor | **yes, but an empty set is a `REVIEW_REQUIRED` configuration error** | Scope is a legitimate per-project choice, but a scope that reports nothing while the concern is enabled is indistinguishable from a silent disable, which is the failure class D18 names. |
| D6 | How a cannot-run checker is classified | one status · two | **two** | `UNBOUND` means nothing ran; `REVIEW_REQUIRED` means something ran and could not finish. Merging them would let a broken checker read as a missing one. |

## 9. Verification oracle

- One test per cell of the envelope mapping table, including a mixed
  ERROR-and-WARNING run proving ERROR dominates.
- One test per row of the failure-path table, asserting the exact status and exit
  code, and asserting that **no** cannot-run case yields `PASS`.
- One test proving the adapter reports the filtered count rather than the
  checker's total, so a 215-finding run scoped to 4 findings reports 4.
- One test proving an empty code set is a configuration error, not a `PASS`.
- One test proving the concern is `UNBOUND` in a project with no bound command.
- A repository-level proof that the concern is enabled in this repository's own
  `.ai-gate/gate.yaml` and appears in its run output.
