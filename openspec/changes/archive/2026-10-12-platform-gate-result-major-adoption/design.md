# Design: adopt `platform.gate-result` 1.0.0

## 1. Implementation boundary

| Path | Change |
|---|---|
| `src/gate/platform_result.rs` | **the whole change**: `PlatformStatus` (+2 values), `map_status` becomes total, `StatusMapping` retired, `Refusal` loses two variants, `build` loses two ordered steps and gains the `contract` property. Still pure: no clock, no filesystem, no database, no manifest, no dependency. |
| `src/gate/platform_result_tests.rs` | unit tests rewritten for the new mapping; the "no state escapes the table" and "`errored` is never produced" tests are kept in stronger form. |
| `src/gate/mod.rs` | re-export `PLATFORM_GATE_RESULT_CONTRACT`; drop `StatusMapping`. |
| `src/cli.rs` | doc text on `GateOutputFormat::Platform` and the `--help` example line. No enum value, no flag, no default change. |
| `src/commands/gate.rs` | doc text on `print_platform`, the `EXIT_UNREPRESENTABLE` comment, and the module header. No behavioural change. |
| `tests/gate_platform_result.rs` | end-to-end emission of every status; the retained refusal classes stay asserted through the binary. |

The emitter MUST NOT:

- add a dependency (no JSON-Schema validator crate, no regex crate),
- read the manifest, the database, the evidence store, or the clock,
- modify `GateStatus`, `GateSeverity`, `AggregateOutcome`, `aggregate`, the
  adapter status/exit tables, the manifest schema, or the concern vocabulary,
- write to stdout on any path, including the refusal path,
- validate its own output against the schema. It asserts the constraints it
  depends on and refuses rather than validating.

## 2. Why 1.0.0 is the honest home for these two statuses

The contract did not loosen its rules to accommodate Driftwatchdog; it *named*
the two claims Driftwatchdog was already making and refusing to state.

From the 1.0.0 `result` description:

| platform value | schema text | Driftwatchdog claim it matches |
|---|---|---|
| `skipped` | "a check was proven not to apply; this is an exemption, not an absence of evidence" | `NOT_APPLICABLE`: a bound command ran and reported it does not apply, or a declared exemption carried a written reason. |
| `review_required` | "a human decision is still open and no one has decided the outcome" | `REVIEW_REQUIRED`: timeout, spawn failure, malformed envelope, exit-code contradiction, exhausted exemption budget — every execution failure is already normalised to "a human must decide". |
| `unbound` | "the check was asserted and no command was ever bound to it, so nothing ran; this is a configuration defect and is never an exemption" | `UNBOUND` (`src/gate/types.rs:7-11`): asserted, nothing bound, nothing ran; explicitly the opposite claim to `NOT_APPLICABLE`. |

The schema adds the constraint that makes the mapping non-lossy rather than
merely convenient: *"A consumer MUST NOT read `skipped` and `unbound` alike,
and MUST NOT read `review_required` as a decision already taken."* That is
exactly the invariant `src/gate/types.rs` has been holding locally. Under 0.1.0
the only way to publish an `UNBOUND` run was to write `skipped` — a conflation
the schema now forbids in writing. Refusing was the correct answer to *that*
contract; it is the wrong answer to this one.

## 3. Status mapping (normative)

`map_status` is **total and infallible**: `fn map_status(GateStatus) -> PlatformStatus`.
Every [`GateStatus`] variant has an arm, so adding a sixth state is a compile
error here rather than a silent emission.

| Driftwatchdog `GateStatus` | Platform `result` / `checks[].status` | Claim |
|---|---|---|
| `PASS` | `passed` | ran and passed |
| `FAIL` | `failed` | ran and reported a failure; severity stays orthogonal and unmapped |
| `NOT_APPLICABLE` | `skipped` | proven exemption |
| `REVIEW_REQUIRED` | `review_required` | a human decision is still open |
| `UNBOUND` | `unbound` | asserted, no command bound, nothing ran |
| — | `errored` | never emitted by this runtime |

The mapping is **symmetric**: the aggregate `AggregateOutcome::status` and each
`GateResult.status` go through the same total function, which is also what the
schema now states — *"The vocabulary is shared by `result` and `checks[].status`
and each value carries the same meaning at both levels."*

### `StatusMapping` is retired, not deprecated

`StatusMapping` had exactly two payloads: `Representable(PlatformStatus)` and
`Unrepresentable(&'static str)`. With all five states representable,
`Unrepresentable` has no constructor, and the wrapper would force every caller
into a match with one live arm. Keeping it would preserve the vocabulary of
refusal in the type system after the behaviour is gone — the exact kind of stale
shape this change exists to remove. `map_status` returns `PlatformStatus`
directly.

### `errored` stays defined and stays unreachable

`errored` is part of the 1.x contract and must remain in
`PlatformStatus::as_str`/`parse`, because a consumer of this type must be able
to name it. It is never *produced*, because this runtime has no source for it:
every execution-level failure Driftwatchdog observes — timeout, spawn failure,
process-group kill, malformed envelope, exit-code/envelope contradiction — is
already normalised to `REVIEW_REQUIRED` upstream in `src/gate/adapters.rs`, and
`REVIEW_REQUIRED` now maps honestly to `review_required`. Splitting that subset
across two platform values on a boundary the schema does not define would be a
new invention, not an adoption.

The proof is a test, not a comment: the table test iterates **every**
`GateStatus` and asserts none maps to `PlatformStatus::Errored`. That test is
retained from the archived change and strengthened by a second assertion that
`as_str`/`parse` still round-trip `errored`, so removing the variant can never
be mistaken for "the platform does not have it".

### Aggregate reachability

`aggregate()` can return only `PASS`, `FAIL`, or `REVIEW_REQUIRED`
(`src/gate/aggregate.rs:167-204`); a required `UNBOUND` check makes the run
`FAIL`, and an *optional* unbound check leaves the aggregate `PASS` with
`outcome.unbound` populated. Both are reachable and both are emitted honestly:
a required unbound run is a root `failed` document whose `checks[]` entry says
`unbound`, and an optional unbound run is a root `passed` document with one
`unbound` entry. The aggregate `unbound` row is defined so the mapping stays
total and the emitter never has a value it cannot name.

This resolves the archived change's "optional check lands under `review_required`
and still refuses" argument: optionality still changes whether a status blocks
and never changes what it claims — it just no longer has to be expressed by
refusing, because the platform now has the vocabulary for the claim.

## 4. Field mapping (normative)

| Platform field | Source | Constraint enforced |
|---|---|---|
| `contract` (additional) | constant `PLATFORM_GATE_RESULT_CONTRACT` | none; fixed by the producer |
| `gate_id` | `ResolvedGatePlan::manifest_digest` (`sha256:` + 16 hex) | `^[A-Za-z0-9][A-Za-z0-9._:-]{0,127}$` |
| `pipeline` | constant `driftwatchdog-gate` | `^[a-z][a-z0-9_.-]{0,127}$` |
| `evaluated_at` | the run's `taken_at` | parseable as RFC 3339 |
| `result` | `AggregateOutcome::status` via §3 | enum |
| `subject_ref` | the run's `revision` | ≤ 256 characters |
| `checks[].check_id` | `GateResult.gate_id` **unchanged** | `^[a-z][a-z0-9_.-]{0,63}$` |
| `checks[].status` | `GateResult.status` via §3 | enum |
| `checks[].detail` | `GateResult.diagnostic` after `redact_secrets` | ≤ 512 characters |
| `evidence_refs` | distinct `GateResult.evidence[].key`, first-seen order | 1..=512 characters each |
| `driftwatch_profile` (additional) | `ResolvedGatePlan::profile`, verbatim | none |
| `driftwatch_gate_id` (additional, per check) | `GateResult.gate_id`, verbatim | none |

`tenant_id` has no Driftwatchdog source and is never emitted.

### The version-declaration shape

The declaration is **one additional root property named `contract`**, carrying the
constant `PLATFORM_GATE_RESULT_CONTRACT = "platform.gate-result/1.0.0"`.

That shape is not invented here. It is the in-repo precedent at
`src/checker/json.rs:41`, which declares
`pub const CHECKER_REPORT_CONTRACT: &str = "driftwatch-checker/0.1.0";` and emits
it as a root `contract` field (`src/checker/json.rs:88`, assigned at `:202`).
Two producers in one repository should emit the same shape for the same
concept; a second spelling (`schema_version`, `$schema`, `contract_version`)
would give every future consumer a branch.

Why it is needed at all: the registry serves majors 0 and 1 **simultaneously**
right now, so a document carrying only `passed`/`failed`/`skipped` is ambiguous
between a 0.1.0 producer and a 1.0.0 producer that happened to have no
review-required or unbound check. A consumer cannot tell which. The declaration
removes the ambiguity from the producer side while the narrowing is `declared`
rather than `enforced`.

It is an *additional* property, which `additionalProperties: true` on the root
explicitly permits, so it cannot invalidate a 0.x-validating consumer for the
fields it already reads. `tenant_id`-style strictness is not applied to it: the
schema constrains its own properties and says nothing about the rest, and the
emitter does not second-guess that.

The value is stored as a `String` field initialised from the constant, so the
single source of truth stays the constant and `Deserialize` keeps working on
the document type.

## 5. Retained refusal classes (normative, exhaustive)

Two variants are **deleted**: `Refusal::AggregateStatus` and
`Refusal::CheckStatus`, along with their `REVIEW_REQUIRED_BECAUSE` /
`UNBOUND_BECAUSE` constants and the `StatusMapping::Unrepresentable` path they
existed to carry. No status can refuse any more.

Seven classes remain, in this fixed order. `build` returns the **first**
refusal, so the diagnostic is deterministic:

1. `GateId` — `gate_id` fails `^[A-Za-z0-9][A-Za-z0-9._:-]{0,127}$`.
2. `Pipeline` — `pipeline` fails `^[a-z][a-z0-9_.-]{0,127}$`.
3. `SubjectRef` — `subject_ref` exceeds 256 characters.
4. `EvaluatedAt` — `evaluated_at` is not a valid RFC 3339 date-time.
5. `CheckId` — a `checks[].check_id` fails `^[a-z][a-z0-9_.-]{0,63}$`.
6. `Detail` — a `checks[].detail` exceeds 512 characters.
7. `EvidenceRef` — an evidence key is empty or exceeds 512 characters.

Order rationale: root identity, then per-check shape — a document whose own
identity cannot even be read should be told that before it is told which of its
checks is odd. Statuses no longer participate, so the previous "statuses first,
so a substantive refusal is never masked by a cosmetic one" clause is gone; the
remaining order preserves the same root-before-leaf property.

**None of the seven is weakened.** Every bound, pattern, and wording from the
archived change is byte-identical, because none of them was a status rule. Every
one of them still: leaves stdout empty, writes one diagnostic on stderr naming
the exact value and the exact constraint, and exits `2` (`EXIT_UNREPRESENTABLE`,
reachable only through `--format platform`).

The gate run is still executed and still persisted before the format is chosen,
exactly as it is for `human` and `json`. A refusal is a statement about
*emission*, not about the run.

### Exact stderr wording

```
driftwatch gate: refusing to emit a platform gate-result document: <reason>
driftwatch gate: nothing was written to stdout; use --format json for driftwatchdog's own machine-readable status document.
```

`<reason>` per surviving case (unchanged from the archived change):

| Case | Reason |
|---|---|
| `gate_id` | `` gate_id `sha256/abc` does not match the platform pattern ^[A-Za-z0-9][A-Za-z0-9._:-]{0,127}$ `` |
| `pipeline` | `` pipeline `Bad Pipeline` does not match the platform pattern ^[a-z][a-z0-9_.-]{0,127}$ `` |
| `subject_ref` | `subject_ref is 300 characters; the platform allows at most 256 and driftwatchdog does not truncate it` |
| `evaluated_at` | `` evaluated_at `2026-10-12 10:00:00` is not an RFC 3339 date-time `` |
| `check_id` | `` check id `L1-configuration` does not match the platform pattern ^[a-z][a-z0-9_.-]{0,63}$; no reversible slug exists inside 64 characters, so driftwatchdog refuses instead of coercing it `` |
| `detail` | `` check `tests` has a 1,024-character diagnostic; the platform allows at most 512 for detail and driftwatchdog does not truncate a diagnostic to fit `` |
| `evidence_refs` | `` evidence reference key `` (empty) `` cannot be represented: the platform requires 1 to 512 characters `` |

## 6. Compatibility boundary

`--format human` and `--format json` are byte-identical and keep exit codes
`0`/`1`. `GateOutputFormat` keeps its three values and its default. `GateFormatArg`
is untouched, so `driftwatch gate evidence-export --format` still accepts
exactly `human|json`. The `human` early-return paths (`no gate.toml`, foreign
runtime) are unchanged.

| Code | Constant | Reachable from |
|---|---|---|
| 0 | `EXIT_OK` | all formats |
| 1 | `EXIT_BLOCKED` | all formats, `outcome.blocked` |
| 2 | `EXIT_UNREPRESENTABLE` | **only** `--format platform`, on one of the seven retained refusals |

Source-level note: `StatusMapping` disappears from the crate's public
re-export list. It has no consumer inside this repository, and it existed only
to express a status being unrepresentable.

## 7. Storage, security, and process boundaries

- **Storage.** None. The emitter reads no table and writes no table.
- **Security.** No network call, no subprocess, no telemetry, no LLM. Redaction
  is applied to `detail` and to evidence keys exactly as before.
- **Determinism.** `checks[]` follows `results` order; `evidence_refs` is
  first-seen with duplicates removed; `contract` is a constant. No map
  iteration, no clock, no randomness inside `build`.
- **Process.** `build` is a pure function of its inputs.

## 8. Residual gap: `detail` and `check_id` still refuse on real repositories

Adopting 1.0.0 fixes the *status* refusals and nothing else. Measured live
after this change (see `HANDOFF.md` for the full table):

- `check_id` must match `^[a-z][a-z0-9_.-]{0,63}$`. Manifest ids are validated
  only as non-empty, and real ids such as `L1-configuration` do not match.
- `detail` allows 512 characters; Driftwatchdog's own diagnostic bound is 2,048,
  and a routine run on this repository produces a 2,049-character
  `declared-verification` diagnostic.

Major 1.0.0 relaxed **neither** bound. Three honest resolutions:

| Option | What it does | Cost / risk |
|---|---|---|
| **(a)** move the long text to an `additionalProperties` sibling | emit `detail` only when it fits; carry the full diagnostic in a `driftwatch_detail` sibling on the check object (the root already carries `driftwatch_profile`, and the check object already carries `driftwatch_gate_id`, so the precedent is established) | stays inside the contract, no cross-repo coordination, no new schema version. It does mean a conforming consumer that reads only `detail` sees a shorter document — that is a loss of *content* on a surface we chose, not a merge of two *claims*, which is the line the status rules drew. It needs the "what does `detail` mean when absent vs. present-and-short" question answered. |
| **(b)** change `detail.maxLength` in `platform-contracts` | raise the bound to at least 2,048 (Driftwatchdog's `MAX_DIAGNOSTIC_BYTES`) | a major-version change in another repository, a second negotiation, and a bound that will drift again the moment Driftwatchdog raises its own diagnostic cap. It also decides the limit for *every* producer, on the strength of one producer's default. |
| **(c)** accept the refusal | leave it | the format never emits on any repository in the fleet. It stays correct and stays useless. |

**Recommendation: (a)**, with `detail` defined as *the first 512 characters of
the diagnostic when it is longer, or nothing at all* — no. Precisely: emit
`detail` **verbatim** whenever it fits, **omit it** when it does not, and carry
the unshortened text in `driftwatch_detail` on the same check object. Never
truncate into `detail` itself, because a silently shortened diagnostic reads as
a complete one.

This is a **decision, not an implementation**: it changes the meaning of a
contract field for every consumer, so it needs the portfolio owner's sign-off
before any code moves. **This change implements none of (a), (b), or (c).** It
reports the gap and keeps the refusal.

## 9. Rejected alternatives

| Alternative | Why rejected |
|---|---|
| Map `REVIEW_REQUIRED` → `errored` | `errored` means "a check could not be evaluated to a verdict". Every such case is already a `REVIEW_REQUIRED`, and 1.0.0 now names that state directly. Splitting one state across two platform values invents a taxonomy the schema does not define. |
| Map `REVIEW_REQUIRED` → `failed` | Asserts a verification failure that was not observed. |
| Map `UNBOUND` → `skipped` | The schema now forbids it in writing: *"A consumer MUST NOT read `skipped` and `unbound` alike."* |
| Keep `StatusMapping` with only `Representable` | Preserves the refusal vocabulary in the types after the behaviour is gone. |
| Emit `review_required` / `unbound` only at check level | The schema states the vocabulary is shared and symmetric; emitting the same claim in two spellings would be a second contract. |
| Relax `check_id` or `detail` so a real document gets through | Explicitly out of scope, and it would be a producer-side edit to another repository's contract. Reported instead (§8). |
| Delete the `errored` variant because it is unreachable | A consumer of this type must be able to *name* every value of the contract. It is defined, round-trippable, and never constructed; the tests hold both halves of that line. |
| Emit the version as `$schema` or `schema_version` | A second spelling for a concept `src/checker/json.rs` already spells `contract`. |

## 10. Verification

```text
cargo fmt
cargo test
cargo clippy
openspec validate --changes --strict --no-interactive
git diff --check
```

Plus one empirical pass outside the suite: build the release binary and run
`driftwatch gate --format platform` against this repository, `forge`, and at
least two other local repositories under `/home/paul/code/`, recording exit
code, stdout byte count, and the exact refusal reason for each. That pass is
reported, not asserted — the residual gap in §8 is a finding, not a test
failure.

The full `driftwatch` Gate runtime, `--all-targets --all-features` clippy with
`-D warnings`, workspace test breadth, and container/integration suites are the
orchestrator's and are not run here.
