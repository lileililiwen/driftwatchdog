# gate-platform-result-export Specification

## Purpose
TBD - created by archiving change platform-gate-result-emission. Update Purpose after archive.
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

### Requirement: Refuse rather than coerce an unrepresentable status

The platform status vocabulary is `passed`, `failed`, `errored`, and `skipped`.
It has no state for a decision that is still open and no state for a check that
was never attempted. The emitter MUST map `PASS` to `passed`, `FAIL` to
`failed`, and `NOT_APPLICABLE` to `skipped`, and MUST treat `REVIEW_REQUIRED`
and `UNBOUND` as unrepresentable in both the aggregate and per-check positions.

When any required check, any optional check, or the aggregate carries an
unrepresentable status, the emitter MUST write nothing to stdout, MUST write a
typed diagnostic naming the exact unrepresentable state and the exact check ids
to stderr, and MUST exit non-zero. It MUST NOT emit a partial, coerced, or
lowered document, and it MUST NOT map `UNBOUND` onto `skipped`, which would
merge a never-attempted check with a proven exemption that the status model
holds to be opposite claims.

The platform's `errored` value MUST NOT be emitted: the platform defines no
execution-failure taxonomy for this producer, and every execution failure
driftwatchdog observes is already a `REVIEW_REQUIRED` result, which is itself
refused. This gap MUST be documented rather than filled by inventing a
mapping.

#### Scenario: Aggregate requires review

- **WHEN** the aggregate status is `REVIEW_REQUIRED`
- **THEN** nothing is written to stdout, stderr names `REVIEW_REQUIRED` and
  states that the platform has no human-decision state, and the exit code is
  non-zero

#### Scenario: A check is unbound

- **WHEN** any check reports `UNBOUND`
- **THEN** nothing is written to stdout, stderr names `UNBOUND` and the exact
  check ids, and the exit code is non-zero

#### Scenario: An optional check is under review

- **WHEN** an optional check reports `REVIEW_REQUIRED` and therefore does not
  block
- **THEN** the emitter still refuses, because optionality changes whether a
  status blocks and never changes what that status claims

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
deterministic: unrepresentable statuses first, then the root identity, then the
per-check shape. A substantive refusal MUST never be masked by a cosmetic one.

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
