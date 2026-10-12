# gate-platform-result-export Specification

## Purpose
`driftwatch gate` can emit a completed run as a `platform.gate-result`
document at contract major 1.0.0, so a portfolio consumer can read "may this
deploy proceed?" from one contract instead of scraping the human table or
hard-coding Driftwatchdog's own five-state vocabulary. Every gate status maps
onto the contract's six values, the document declares the version it claims,
and a run that still cannot be represented is refused rather than coerced:
nothing reaches stdout, one diagnostic naming the exact offending value goes to
stderr, and the process exits non-zero while the run itself is still executed
and still persisted.
## Requirements
### Requirement: Emit a platform gate-result document on request

`driftwatch gate` MUST offer a machine-readable output that conforms to the
`platform.gate-result` contract, selected with a new `--format` value. The new
value MUST be additive: the existing `human` and `json` outputs and their exit
codes MUST remain unchanged. The emitter MUST NOT add a JSON-Schema validation
dependency and MUST NOT be the second authority on the schema; the consumer
validates the document. It MUST be a pure function of an executed gate run and
MUST NOT read the clock, the filesystem, or the database while building.

The document MUST carry `gate_id`, `pipeline`, `evaluated_at` as an RFC 3339
date-time, and `result`; and MAY carry `subject_ref`, `checks`, and
`evidence_refs`. The driftwatchdog pipeline identity MUST be reported in
`pipeline` and the project-owned profile MUST be preserved verbatim alongside
it, so that no project-owned value is coerced into a shape it does not have.

#### Scenario: Clean pass is emitted

- **WHEN** a gate run completes with an aggregate status of `PASS` and every
  check reports `PASS`
- **THEN** the emitter writes one document to stdout whose `result` is `passed`
  and whose every `checks[].status` is `passed`

#### Scenario: Clean fail is emitted

- **WHEN** a gate run completes with an aggregate status of `FAIL`
- **THEN** the emitter writes one document to stdout whose `result` is `failed`
  and the process exits with the existing blocked exit code

#### Scenario: Not applicable is emitted as skipped

- **WHEN** a gate run completes with a check whose status is `NOT_APPLICABLE`,
  meaning the absence was proven by a bound command or by a declared exemption
  carrying a written reason
- **THEN** that check is emitted with status `skipped`

#### Scenario: The gate run is still recorded

- **WHEN** the platform format is requested
- **THEN** the gate still executes and still persists its run record exactly as
  the `human` and `json` formats do, so a refusal is a statement about emission
  and never about the run

### Requirement: Refuse rather than coerce an unrepresentable identity

The platform constrains `gate_id` to `^[A-Za-z0-9][A-Za-z0-9._:-]{0,127}$`,
`pipeline` to `^[a-z][a-z0-9_.-]{0,127}$`, and `checks[].check_id` to
`^[a-z][a-z0-9_.-]{0,63}$`. Driftwatchdog check ids come from a project
manifest and are not otherwise constrained.

The emitter MUST pass a conforming check id through unchanged and MUST refuse
any check id that does not already conform. It MUST NOT truncate, lowercase,
transliterate, or otherwise slug an id, because no such transform is a
lossless reversible map into the platform's 64-character identifier, and
merging two distinct driftwatchdog checks into one platform identifier is worse
than declining to publish. The original id MUST additionally be preserved
verbatim on the emitted check object so the identity is recoverable and
machine-checkable without a decoding rule.

Length limits MUST be evaluated in Unicode code points, as the schema defines
them, not in UTF-8 bytes.

#### Scenario: Conforming check id

- **WHEN** every check id matches the platform pattern
- **THEN** each is emitted unchanged and its driftwatchdog id is repeated
  verbatim alongside it

#### Scenario: Non-conforming check id

- **WHEN** a check id does not match the platform pattern, for example one
  beginning with an uppercase letter or a digit
- **THEN** nothing is written to stdout, stderr names the offending id and the
  pattern, states that no reversible slug exists inside the platform's length
  cap, and the exit code is non-zero

### Requirement: Refuse rather than truncate an over-length field

The platform bounds `subject_ref` at 256 characters, `checks[].detail` at 512,
and each `evidence_refs` entry at 1 to 512 characters. Driftwatchdog's own
diagnostic bound is 2,048 characters, so these limits are reachable in normal
operation.

The emitter MUST NOT truncate, elide, or null out any field to fit a platform
limit. A field that cannot be represented within its limit MUST cause a refusal
naming the field and the offending value. Redaction MUST still be applied to
the values that are emitted, because redaction removes content while bounding
invents it.

#### Scenario: Over-length diagnostic

- **WHEN** a check's diagnostic exceeds the platform's detail limit
- **THEN** nothing is written to stdout, stderr names the check and the exact
  length, and the exit code is non-zero

#### Scenario: Evidence reference key

- **WHEN** an evidence reference key is empty or exceeds the platform's limit
- **THEN** the emitter refuses rather than dropping, shortening, or
  reconstructing the reference

#### Scenario: Subject reference

- **WHEN** the gated revision exceeds the platform's `subject_ref` limit
- **THEN** the emitter refuses rather than shortening the revision

### Requirement: Refuse an unevaluable document

The emitter MUST evaluate every platform constraint it depends on before
writing any byte, and MUST evaluate them in a fixed order so the diagnostic is
deterministic: the root identity first, then the per-check shape. A substantive
refusal MUST never be masked by a cosmetic one.

No status refuses, so the evaluation order is, exhaustively:

1. `GateId` — `gate_id` fails `^[A-Za-z0-9][A-Za-z0-9._:-]{0,127}$`.
2. `Pipeline` — `pipeline` fails `^[a-z][a-z0-9_.-]{0,127}$`.
3. `SubjectRef` — `subject_ref` exceeds 256 characters.
4. `EvaluatedAt` — `evaluated_at` is not a valid RFC 3339 date-time.
5. `CheckId` — a `checks[].check_id` fails `^[a-z][a-z0-9_.-]{0,63}$`.
6. `Detail` — a `checks[].detail` exceeds 512 characters.
7. `EvidenceRef` — an evidence key is empty or exceeds 512 characters.

These seven classes are retained unchanged. The emitter MUST NOT weaken a
pattern or a length bound, MUST NOT truncate, elide, or null out a value to fit,
and MUST NOT drop a field to make a document emit.

A refusal MUST leave stdout empty. The document MUST be built to completion in
memory before it is printed, so no consumer can ever observe a partial or
lowered `platform.gate-result` document.

#### Scenario: Deterministic refusal

- **WHEN** a run violates more than one platform constraint
- **THEN** the reported refusal is the same one on every run, following the
  documented evaluation order

#### Scenario: No partial output

- **WHEN** the emitter refuses
- **THEN** stdout is empty, the diagnostic is on stderr, and the exit code is
  non-zero and distinct from the existing pass and blocked codes

#### Scenario: Root identity is reported before per-check shape

- **WHEN** a run has both an unreadable root field and a non-conforming check
  id
- **THEN** the root field is the reported refusal

### Requirement: Declare the contract version in the emitted document

The emitted `platform.gate-result` document MUST declare which version of the
contract it claims, through one root additional property named `contract`
whose value is `platform.gate-result/1.0.0`. The declaration MUST come from a
single named constant in the emitter rather than a string literal at the
emission site, so that the declared version and the mapping that depends on it
cannot drift apart.

The declaration is REQUIRED because the registry currently serves contract
major 0 and major 1 simultaneously: a document that carries only `passed`,
`failed`, and `skipped` is ambiguous between a 0.1.0 producer and a 1.0.0
producer whose run happened to contain no open decision and no unbound check.
The producer, not the consumer, resolves that ambiguity.

The shape MUST match the repository's existing declaration precedent — a root
`contract` field carrying a `name/major.minor.patch` constant. The emitter MUST
NOT introduce a second spelling such as `schema_version`, `$schema`, or
`contract_version`.

The declaration MUST be an additional property, which the schema permits on the
root, and MUST NOT change, rename, or remove any contract field.

#### Scenario: The declaration is present on every document

- **WHEN** the platform format is requested for any run the emitter can
  represent
- **THEN** the document's root `contract` is `platform.gate-result/1.0.0`

#### Scenario: The declared version is a single constant

- **WHEN** the emitter is inspected
- **THEN** the emitted value and the mapping it describes come from one named
  constant, not from a literal repeated at the emission site

### Requirement: Map every gate status onto the contract's major 1 vocabulary

The contract's `result` and `checks[].status` share one six-value vocabulary —
`passed`, `failed`, `errored`, `skipped`, `review_required`, `unbound` — and the
emitter MUST map every Driftwatchdog [`GateStatus`] onto it at both levels: the
aggregate `result` and each per-check `status`, through the same mapping.

| Driftwatchdog `GateStatus` | platform value | claim |
|---|---|---|
| `PASS` | `passed` | ran and passed |
| `FAIL` | `failed` | ran and reported a failure |
| `NOT_APPLICABLE` | `skipped` | a proven exemption |
| `REVIEW_REQUIRED` | `review_required` | a human decision is still open |
| `UNBOUND` | `unbound` | asserted, no command bound, nothing ran |
| — | `errored` | never emitted by this runtime |

The mapping MUST be total and MUST NOT have a failure case: every `GateStatus`
variant has an arm, so a future state is a compile error at the mapping rather
than a silent emission or a refusal.

The mapping MUST NOT coerce. `REVIEW_REQUIRED` MUST NOT become `failed`,
`errored`, or `skipped`, and `UNBOUND` MUST NOT become `skipped`: the contract
states that a consumer MUST NOT read `skipped` and `unbound` alike, and MUST
NOT read `review_required` as a decision already taken, which is the same
distinction `NOT_APPLICABLE` and `UNBOUND` already make inside Driftwatchdog.

`errored` MUST remain part of the producer's platform vocabulary — it is a
value of the contract and a consumer of this type must be able to name it — and
it MUST remain unreachable: every execution failure this runtime observes
(timeout, spawn failure, process-group kill, malformed envelope, exit-code
contradiction) is already normalized to `REVIEW_REQUIRED`, which now has an
honest value of its own.

Optionality MUST NOT change what a status claims. An optional check under review
and an optional unbound check are emitted with `review_required` and `unbound`
respectively, under whichever root `result` the aggregate decided; the emitter
MUST NOT refuse them for being optional, and MUST NOT lower them.

#### Scenario: An open human decision is emitted as review_required

- **WHEN** the aggregate status is `REVIEW_REQUIRED`, because a required check
  has no verdict yet and the manifest blocks on review
- **THEN** the emitter writes a document whose `result` is `review_required`
  and exits with the existing blocked exit code

#### Scenario: A pending review is emitted at check level

- **WHEN** an optional check reports `REVIEW_REQUIRED` and therefore does not
  block
- **THEN** the document is written, its root `result` reflects the aggregate,
  and that check's `status` is `review_required`

#### Scenario: An unbound check is emitted as unbound

- **WHEN** a check reports `UNBOUND`, meaning it was asserted and no command
  was ever bound to it
- **THEN** the document is written and that check's `status` is `unbound`, so a
  consumer counting checks sees every check the gate asserted

#### Scenario: Unbound is never merged with skipped

- **WHEN** a run contains both a `NOT_APPLICABLE` check and an `UNBOUND` check
- **THEN** the two are emitted with different statuses — `skipped` and
  `unbound` — and neither is refused, coerced, or omitted

#### Scenario: Errored is never produced

- **WHEN** any `GateStatus` is mapped
- **THEN** no status maps to `errored`, while `errored` remains a named,
  round-trippable value of the producer's platform vocabulary

#### Scenario: Every status has a mapping arm

- **WHEN** the mapping is inspected
- **THEN** it is a total function of `GateStatus` with no failure arm, and it
  produces the same value at the aggregate and per-check levels

