# Design: `platform.gate-result` emission

## 1. Implementation boundary

| Path | Change |
|---|---|
| `src/gate/platform_result.rs` | **new.** The whole emitter: mapping types, the pure `build`, the refusal type, and the ASCII pattern validators. No IO, no clock, no database, no manifest access. |
| `src/gate/mod.rs` | Register the module and re-export its public surface. |
| `src/cli.rs` | Add `GateOutputFormat { Human, Json, Platform }` and use it for `GateArgs.format` only. `GateFormatArg` keeps its two values so `driftwatch gate evidence-export --format` is untouched. |
| `src/commands/gate.rs` | One new `match` arm, one new exit constant, and the diagnostic printer. |
| `tests/gate_platform_result.rs` | **new.** End-to-end fixtures. |

The emitter MUST NOT:

- add a dependency (no JSON-Schema validator crate, no regex crate),
- read the manifest, the database, the evidence store, or the clock,
- modify `GateStatus`, `GateSeverity`, `AggregateOutcome`, `aggregate`, the
  adapter status/exit tables, the manifest schema, or the concern vocabulary,
- write to stdout on any path, including the refusal path,
- validate its own output against the schema. It asserts the schema's
  constraints directly and refuses rather than validating.

## 2. Why this is a mapping problem and not a validation problem

The schema is vendored by the consumer. Forge validates what Driftwatchdog
emits. If Driftwatchdog also validated, a schema drift would be silently
absorbed by a second, stale copy of the rules inside the producer, and the
emitter would keep emitting a document Forge rejects. A producer that refuses
on unrepresentable *data* and trusts the schema for *structure* fails loudly
at the right place.

## 3. Status mapping (normative)

`map_status` is **total**: it returns a value for all five states and never
fails. The failure is the data, not the call. This is what makes the table
below exhaustive by construction — a sixth `GateStatus` variant cannot be
added without the compiler pointing at this function.

| Driftwatchdog `GateStatus` | Platform `result` / `checks[].status` | Rationale |
|---|---|---|
| `PASS` | `passed` | Direct. The check executed and every required evidence input was present. |
| `FAIL` | `failed` | Direct. Severity stays orthogonal and is *not* mapped: the platform has no severity field, and `Info`/`Warning`/`Error` do not change whether a check failed. |
| `NOT_APPLICABLE` | `skipped` | **Judgement, stated explicitly.** `NOT_APPLICABLE` is a *proven* absence: either a bound command ran and reported the check does not apply, or the manifest declared an `not_applicable` exemption with a written reason. "Skipped" is the closest platform word and does not assert a failure that did not happen. |
| `REVIEW_REQUIRED` | **unrepresentable** | The platform has no human-decision state. `REVIEW_REQUIRED` means *a human has not decided yet* — a command timed out, a spawn failed, an envelope was malformed or contradicted its exit code, or the exemption budget is exhausted. Mapping it to `failed` asserts an assertion failure that was not observed; mapping it to `errored` invents a tool-execution taxonomy the platform never defined for this producer; mapping it to `skipped` claims the check was skipped, which is false; mapping it to `passed` is a lie. **Refuse.** |
| `UNBOUND` | **unrepresentable** | `src/gate/types.rs:7-11` states that `UNBOUND` and `NOT_APPLICABLE` "must never share a status, a severity, or a findings list" because one is a never-attempted absence and the other a proven exemption. `skipped` is the platform word for the proven one. Emitting `skipped` for `UNBOUND` would publish exactly the conflation that invariant forbids, to a consumer that cannot see the difference. **Refuse.** |

### `errored` is never emitted

The platform's fourth value, `errored`, has no Driftwatchdog source. Every
execution-level failure Driftwatchdog observes — timeout, spawn failure,
process-group kill, malformed envelope, exit-code/envelope contradiction —
is already normalized upstream to `REVIEW_REQUIRED` by
`src/gate/adapters.rs`, and `REVIEW_REQUIRED` is itself refused. Deliberately
mapping that subset to `errored` while refusing the rest would split one
driftwatchdog state across two platform states on a boundary the schema does
not define. The gap is recorded here rather than papered over: a consumer
reading this document sees only `passed`, `failed`, and `skipped`, and a
driftwatchdog run that needs `errored` semantics arrives as a refusal naming
the exact checks instead.

### Aggregate vs. check level

The aggregate `AggregateOutcome::status` maps through the same table and is
emitted as the root `result`. `aggregate()` can currently only return `PASS`,
`FAIL`, or `REVIEW_REQUIRED` (`src/gate/aggregate.rs:167-204`), so a clean
aggregate `NOT_APPLICABLE` is unreachable in practice; the table still
defines it (`skipped`) so the mapping stays total.

**Required is stricter than the stated floor.** The floor is "refuse when any
*required* check or the aggregate is `REVIEW_REQUIRED` or `UNBOUND`". This
implementation refuses when **any** check is in those states, required or
optional. An optional check lands in `pending_reviews` as `optional:<id>` and
never blocks, but coercing it to `skipped` would publish "skipped" for a check
that was actually reviewed and left pending. Optionality changes whether a
status blocks; it does not change what the status *claims*. Refusing more
than the floor is safe; coercing less would not be.

### Optional checks and check-level `failed`

Every normalized result is emitted as one `checks[]` entry, including optional
ones. The platform has no `required` flag, so an optional check that failed
appears as `failed` while the root `result` may be `passed`. That is not a
contradiction: the schema states the root `result` value controls whether a
deploy may proceed, and `aggregate()` already decided it. The document is
honest about both facts rather than hiding the optional check.

## 4. Field mapping (normative)

| Platform field | Source | Constraint enforced |
|---|---|---|
| `gate_id` | `ResolvedGatePlan::manifest_digest` (`sha256:` + 16 hex) | `^[A-Za-z0-9][A-Za-z0-9._:-]{0,127}$` |
| `pipeline` | constant `driftwatchdog-gate` | `^[a-z][a-z0-9_.-]{0,127}$` |
| `evaluated_at` | the run's `taken_at` (`chrono::Utc::now().to_rfc3339()`) | parseable as RFC 3339 |
| `result` | `AggregateOutcome::status` via §3 | enum |
| `subject_ref` | the run's `revision` | ≤ 256 characters |
| `checks[].check_id` | `GateResult.gate_id` **unchanged** | `^[a-z][a-z0-9_.-]{0,63}$` |
| `checks[].status` | `GateResult.status` via §3 | enum |
| `checks[].detail` | `GateResult.diagnostic` after `redact_secrets` | ≤ 512 characters |
| `evidence_refs` | the distinct `GateResult.evidence[].key` values, first-seen order | 1..=512 characters each |
| `driftwatch_profile` (additional) | `ResolvedGatePlan.profile`, verbatim | none |
| `driftwatch_gate_id` (additional, per check) | `GateResult.gate_id`, verbatim | none |

`tenant_id` has no Driftwatchdog source and is never emitted.

### `gate_id` is the manifest digest

The digest is Driftwatchdog's own gate-run identity, already persisted on every
`gate_runs` row, already `sha256:<hex>` (`src/gate/manifest.rs:986-992`), and
already what `driftwatch doctor` prints. It is a *fact about the run* and it
conforms. The pattern check is still applied: if the producer ever changes the
digest format, the emitter refuses instead of emitting a non-conforming id.

### `pipeline` is a constant, and the profile is preserved losslessly

`pipeline` is **not** the manifest profile. A profile is project-owned text of
unbounded shape — `src/gate/manifest.rs` validates it is non-empty and
resolvable, nothing more — so using it as `pipeline` would refuse on a
perfectly ordinary project whose profile is `Mobile_Product` or `web app`.
`pipeline` describes the pipeline that produced the document, and for this
producer that is always `driftwatchdog-gate`.

The profile is not lost. It is carried verbatim in the root
`driftwatch_profile` additional property, which the schema permits
(`additionalProperties: true`) and constrains not at all. This is the right
trade: a constant is a documented fact, not a guess; a constant never needs to
be refused; and the information that would justify a refusal is preserved
exactly.

`rule_pack_version` is **not** carried. Its default is the workspace-wide
`workspace-governance@1` and it adds no per-project information to a document
already keyed by the manifest digest.

### Lengths are counted in characters, not bytes

JSON Schema `minLength`/`maxLength` count **Unicode code points**. Rust
`str::len()` counts UTF-8 **bytes**. Every length check in this module uses
`chars().count()`. Using `len()` would silently refuse documents that conform
(e.g. a 200-character ASCII-plus-accented `subject_ref` of 240 bytes) and
silently accept documents that do not. This is the one place where a
plausible-looking Rust idiom is wrong.

### Pattern checks are hand-written ASCII scans

`pattern` in JSON Schema is ECMA-262 regular-expression matching. These four
patterns are simple anchored ASCII character classes, so they are implemented
as direct byte scans over `[a-z]`, `[A-Z]`, `[0-9]`, `_`, `.`, `:`, `-`. This
avoids adding a regex dependency for four expressions, and the equivalence is
pinned by unit tests that assert accepted and rejected values on both sides.

The patterns are also explicitly ASCII-only. `L1-configuration` fails on the
leading `L`; `résumé` fails on `é`; `c0ffee` passes; `0abc` fails on the
leading digit.

### `checks[].check_id`: why there is no slug

The task is to map an arbitrary manifest-declared check id into
`^[a-z][a-z0-9_.-]{0,63}$` without losing information.

- **Truncation is out.** It is lossy and silent. Two ids sharing a 64-character
  prefix would become one check.
- **Lowercasing is out.** It is not injective: `L1-configuration` and
  `l1-configuration` are distinct Driftwatchdog checks with distinct
  commands, and both map to `l1-configuration`.
- **Transliterating is out.** Same collision class, and worse: `ß` and `ss`
  would merge.
- **Encoding every byte into `[a-z0-9]`** is injective in principle, but the
  alphabet is 36 symbols for a 256-symbol byte space. Six bytes need ten
  characters, so any id longer than ~38 bytes overflows the 64-character cap,
  and a real id like `governance-quality` (19 bytes) would become 38
  characters of unreadable gibberish that no consumer can display.
- **A fixed escape convention** (for example `_2d` for `-`) is injective, but
  `d`, `2`, and `_2d` then become ambiguous as literal text: `a_b` could mean
  `a_b` or `a-b`, and the consumer cannot tell without a full parser. Any
  scheme that requires a parser to be reversible is a scheme that will be
  mis-parsed by the first consumer that reads `check_id` as an opaque string.

**Conclusion: no lossless, reversible slug exists inside the platform's
64-character `check_id` for arbitrary Driftwatchdog check ids.** The emitter
therefore emits the id **unchanged** and refuses any id that does not already
conform. Because the mapping is the identity, it is trivially and *totally*
reversible: `check_id` is the Driftwatchdog gate id, and
`driftwatch_gate_id` repeats it verbatim so the round trip is machine-checkable
without a decoding rule.

This is the brief's suggested alternative made explicit: the original name is
preserved losslessly (because it is never transformed), the decision is
recorded on the object, and the reversibility is total by construction.

The consequence is honest: a project whose manifest declares
`id = "L1-configuration"` cannot emit a platform document. That is the correct
answer — the alternative is a consumer-visible collision.

**Note on an upstream bound.** `src/commands/gate.rs:318` truncates a check id
to 128 characters with an ellipsis before it reaches `GateResult`. A truncated
id contains `…`, fails the platform pattern, and is refused. This emitter does
not repair or undo that bound; it reports it.

### `detail` too long is a refusal, not a `null`

`detail` allows `["string", "null"]`, so `null` is available as an escape
hatch, and driftwatchdog's own `MAX_DIAGNOSTIC_BYTES` is 2,048 — four times
the platform's 512. Emitting `null` for an over-long diagnostic would be
lossy *and* ambiguous: a consumer could not distinguish "this check produced
no diagnostic" from "the diagnostic was too long to represent". Emitting a
truncated diagnostic is forbidden outright.

So the emitter applies `redact_secrets` (a pure redaction pass that never
truncates and never lengthens), measures the result in characters, and refuses
above 512. The refusal names the check and the exact length.

`bound_text` is deliberately **not** used on any field the platform bounds:
it truncates with an ellipsis, which is exactly the silent loss being avoided.
Redaction is applied where it is a security control; bounding is not applied
where it is a lie.

### `evidence_refs` are keys, not composite strings

The schema types `evidence_refs` as opaque strings with no sub-structure.
Driftwatchdog's `EvidenceRef` has `key`, `digest`, `media_type`, `byte_size`,
and `preview`. Only the key is emitted.

This is not lossy in the sense that matters: the key **is** the artifact
reference into the bounded evidence store, and a consumer holding a key can
resolve the artifact — including its digest — from the run record. Inventing a
composite `"<key>@<digest>"` string would create an undocumented mini-format
that no consumer can parse without a second agreement. An empty key (which the
schema's `minLength: 1` forbids) and a key over 512 characters are both
refused.

Distinct keys are emitted once each, in first-seen order across `results`, so
the array is deterministic.

## 5. Refusal cases (normative, exhaustive)

`build` evaluates in this fixed order and returns the **first** refusal, so the
diagnostic is deterministic:

1. `AggregateStatus` — the aggregate status is `REVIEW_REQUIRED` or `UNBOUND`.
2. `CheckStatus` — one or more checks are `REVIEW_REQUIRED` or `UNBOUND`. All
   offending ids are collected, deduplicated, and sorted.
3. `GateId` — `gate_id` fails `^[A-Za-z0-9][A-Za-z0-9._:-]{0,127}$`.
4. `Pipeline` — `pipeline` fails `^[a-z][a-z0-9_.-]{0,127}$`.
5. `SubjectRef` — `subject_ref` exceeds 256 characters.
6. `EvaluatedAt` — `evaluated_at` is not a valid RFC 3339 date-time.
7. `CheckId` — a `checks[].check_id` fails `^[a-z][a-z0-9_.-]{0,63}$`.
8. `Detail` — a `checks[].detail` exceeds 512 characters.
9. `EvidenceRef` — an evidence key is empty or exceeds 512 characters.

Order rationale: statuses first, because they are the substantive reason a
run cannot be published; then root identity; then per-check shape. A status
refusal is never masked by a cosmetic length refusal.

Every refusal takes the same shape:

- **stdout is empty.** The document is built to completion in memory before a
  single byte is written, so there is no partial or coerced output.
- **stderr carries one diagnostic** naming the exact unrepresentable state or
  value and, for check-scoped refusals, the exact check ids.
- **the exit code is `2`** (`EXIT_UNREPRESENTABLE`), which is reachable only
  through `--format platform`.

The gate run is still executed and still persisted before the format is
chosen, exactly as it is for `human` and `json`. A refusal is a statement
about *emission*, not about the run: the local record is complete and the
process exit reflects a producer that declined to publish, not a gate that did
not happen.

### Exact stderr wording

```
driftwatch gate: refusing to emit a platform gate-result document: <reason>
driftwatch gate: nothing was written to stdout; use --format json for driftwatchdog's own machine-readable status document.
```

`<reason>` per case:

| Case | Reason |
|---|---|
| aggregate status | `the aggregate status REVIEW_REQUIRED has no platform.gate-result equivalent (platform result is one of passed, failed, errored, skipped); driftwatchdog's REVIEW_REQUIRED means a human decision is still open and the platform has no human-decision state` |
| aggregate `UNBOUND` | `the aggregate status UNBOUND has no platform.gate-result equivalent (platform result is one of passed, failed, errored, skipped); coercion to skipped would merge a never-attempted check with a proven exemption, which driftwatchdog forbids` |
| check status | `checks with status REVIEW_REQUIRED have no platform.gate-result equivalent and are not coerced: slow, ux-review` (same `UNBOUND` clause for `UNBOUND`) |
| `gate_id` | `` gate_id `sha256/abc` does not match the platform pattern ^[A-Za-z0-9][A-Za-z0-9._:-]{0,127}$ `` |
| `pipeline` | `` pipeline `Bad Pipeline` does not match the platform pattern ^[a-z][a-z0-9_.-]{0,127}$ `` |
| `subject_ref` | `subject_ref is 300 characters; the platform allows at most 256 and driftwatchdog does not truncate it` |
| `evaluated_at` | `` evaluated_at `2026-10-12 10:00:00` is not an RFC 3339 date-time `` |
| `check_id` | `` check id `L1-configuration` does not match the platform pattern ^[a-z][a-z0-9_.-]{0,63}$; no reversible slug exists inside 64 characters, so driftwatchdog refuses instead of coercing it `` |
| `detail` | `check `tests` has a 1,024-character diagnostic; the platform allows at most 512 for detail and driftwatchdog does not truncate a diagnostic to fit` |
| `evidence_refs` | `` evidence reference key `` (empty) `` cannot be represented: the platform requires 1 to 512 characters `` |

The reasons name the value, the constraint, and the reason driftwatchdog
refuses rather than coercing, because a consumer reading only the stderr line
must be able to act on it.

## 6. CLI and compatibility boundary

`GateArgs.format` changes type from `GateFormatArg` to a new
`GateOutputFormat` that adds `Platform`. This is a source-level change to a
`pub` struct in `src/cli.rs`; it is contained, and the three in-repo
constructors of `GateArgs` (`src/commands/gate.rs` tests) are updated.

`GateFormatArg` is left exactly as it is and is still the type of
`GateEvidenceExportArgs.format`, so `driftwatch gate evidence-export --format`
keeps two values and cannot be given `--format platform`.

`GateOutputFormat::Human` and `GateOutputFormat::Json` map onto the identical
`print_human` / `print_json` calls. The `match` in `gate()` gains one arm.
Existing stdout bytes and exit codes are unchanged, including the two
early-return paths (`no gate.toml`, foreign runtime) which print human text
under `--format platform` exactly as they do under `--format json`.

Exit codes:

| Code | Constant | Reachable from |
|---|---|---|
| 0 | `EXIT_OK` | all formats |
| 1 | `EXIT_BLOCKED` | all formats, `outcome.blocked` |
| 2 | `EXIT_UNREPRESENTABLE` | **only** `--format platform`, on refusal |

## 7. Storage, security, and process boundaries

- **Storage.** None. The emitter reads no table and writes no table. The run
  it describes was already persisted by the unchanged code path.
- **Security.** No network call, no subprocess, no telemetry, no LLM. Redaction
  is applied to `detail` and to evidence keys before they reach the document,
  using the existing `gate::redact` pass. No secret is newly introduced; no
  existing redaction is skipped.
- **Determinism.** `checks[]` follows `results` order. `evidence_refs` is
  first-seen order with duplicates removed. No map iteration, no clock, no
  randomness inside `build`.
- **Process.** The emitter is a pure function of its inputs. It cannot observe
  the filesystem, so a unit test and a production run take the same branch.

## 8. Rejected alternatives

| Alternative | Why rejected |
|---|---|
| Depend on a JSON-Schema validator crate | The consumer owns validation. A second copy of the rules inside the producer would mask schema drift and keep emitting what the consumer rejects. |
| Map `REVIEW_REQUIRED` → `errored` | `errored` is undefined for this producer, and every execution failure is already a `REVIEW_REQUIRED`. Splitting one state across two platform states is a new invention. |
| Map `REVIEW_REQUIRED` → `failed` | Asserts a verification failure that was not observed. |
| Map `UNBOUND` → `skipped` | Directly violates the `src/gate/types.rs:7-11` invariant. |
| Lowercase or slug check ids | Not injective; silently merges two distinct checks. |
| Truncate long fields to fit | Silent loss, explicitly forbidden. |
| Emit `detail: null` when over-length | Ambiguous between "no diagnostic" and "diagnostic too long". |
| Reuse `GateFormatArg` for all three values | Would silently widen `evidence-export --format` beyond this change's scope. |
| Fix the manifest to reject non-conforming check ids | That is a manifest-schema change owned by another concern. Out of scope here. |

## 9. Verification

```text
cargo fmt
cargo test
cargo clippy
openspec validate --changes --strict --no-interactive
git diff --check
```

The full `driftwatch` Gate runtime, `--all-targets --all-features` clippy with
`-D warnings`, workspace test breadth, and container/integration suites are the
orchestrator's and are not run here.