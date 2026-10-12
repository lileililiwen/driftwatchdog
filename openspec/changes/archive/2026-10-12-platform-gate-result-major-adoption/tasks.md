# Tasks: platform-gate-result-major-adoption

Ordered BFS impact map → DFS requirement implementation → BFS regression and
completeness → verification.

## 1. BFS — Impact map and baseline

- [x] 1.1 Read the authoritative contract before writing code:
      `platform-contracts/schemas/gate-result.schema.json` (six-value `result`
      and `checks[].status` enums, unchanged `gate_id` / `pipeline` /
      `check_id` patterns, unchanged 512-character `detail` and
      `evidence_refs` caps, unchanged 256-character `subject_ref` cap,
      `additionalProperties: true` on root and check object) and the registry
      record `contracts["platform.gate-result"]` (`current_version: 1.0.0`,
      `supported_major_versions: [0, 1]`, `narrowing.state: declared`,
      `drops: [0]`). Record that major 0 is still served and that this change
      is its named precondition.
- [x] 1.2 Map every consumer of the emitted document before changing it. Inside
      this repository: none — no MCP tool, no `doctor` check, no
      `evidence-export`, no persisted-record reader. Outside it: Forge vendors
      the schema and *emits* `platform.gate-result` from its own evidence
      store; it never reads `driftwatch gate --format platform`, and its
      Driftwatchdog consumption path is `gate --format json`. Evidence:
      `forge/src/contract/mod.rs::emit_gate_result`,
      `forge/src/policy/driftwatch.rs`, `forge/contracts/registry.json`.
- [x] 1.3 Confirm what 1.0.0 did **not** relax: `check_id` is still
      `^[a-z][a-z0-9_.-]{0,63}$` and `detail` is still `maxLength: 512`. Both
      must keep refusing; neither bound may be touched.
- [x] 1.4 Record the pre-existing behaviour that must not change: the exact
      stdout of `--format human` and `--format json`, exit codes `0` / `1`,
      `GateFormatArg`'s two values for `evidence-export`, and the
      `EXIT_UNREPRESENTABLE = 2` contract for the platform format.

## 2. DFS — Adopt the 1.0.0 vocabulary

- [x] 2.1 Add `PlatformStatus::ReviewRequired` and `PlatformStatus::Unbound`
      with `review_required` / `unbound` wire strings in `as_str` and `parse`.
      Keep `Errored` defined and reachable from `parse`.
- [x] 2.2 Make `map_status` a total `fn(GateStatus) -> PlatformStatus`. Delete
      `StatusMapping` and its `Unrepresentable` payload — with every state
      representable it has no constructor, and keeping it would preserve the
      refusal vocabulary in the types after the behaviour is gone. Drop
      `StatusMapping` from the `src/gate/mod.rs` re-export list.
- [x] 2.3 Delete `Refusal::AggregateStatus` and `Refusal::CheckStatus`, the
      `REVIEW_REQUIRED_BECAUSE` / `UNBOUND_BECAUSE` constants, the
      `unrepresentable_because` helper, and the two ordered steps in `build`
      that produced them. Retarget `build`'s doc comment at the surviving
      order: root identity, then per-check shape.
- [x] 2.4 Rewrite the module documentation: the five-row mapping table with the
      1.0.0 values, why `errored` is unreachable, and why `review_required`
      and `unbound` are honest rather than coerced.
- [x] 2.5 Evidence: one unit test per mapping row; the "every status has an arm"
      test kept; the "`errored` is never produced" test kept **and**
      strengthened with an `as_str`/`parse` round-trip assertion so removing the
      variant can never be mistaken for "the platform does not have it".

## 3. DFS — Declare the contract version

- [x] 3.1 Add `pub const PLATFORM_GATE_RESULT_CONTRACT: &str =
      "platform.gate-result/1.0.0";` beside `PLATFORM_PIPELINE`.
- [x] 3.2 Add a root `contract: String` property to `PlatformGateResult`,
      initialised from that constant in `build`, and re-export the constant
      from `src/gate/mod.rs`.
- [x] 3.3 Do not invent a second shape: the field is named `contract` and the
      value is `name/major.minor.patch`, matching
      `src/checker/json.rs:41` / `:88` / `:202`.
- [x] 3.4 Evidence: a unit test pinning the field on a built document and a
      serialisation test pinning the JSON key; an end-to-end assertion that the
      emitted stdout carries it.

## 4. DFS — Retain the seven remaining refusal classes

- [x] 4.1 Confirm each surviving variant's error string, pattern constant, and
      bound is byte-identical to the archived change: `GateId`, `Pipeline`,
      `SubjectRef`, `EvaluatedAt`, `CheckId`, `Detail`, `EvidenceRef`.
- [x] 4.2 Keep every existing unit test for those seven, unchanged in
      expectation: conforming ids pass, non-conforming ids refuse and name the
      pattern, a 64-character id passes and a 65-character one refuses, a
      512-character diagnostic is emitted and a 513-character one refuses,
      lengths are counted in code points and not bytes, an empty and an
      over-length evidence key both refuse, evidence refs are distinct and in
      first-seen order, and redaction still runs before the measurement.
- [x] 4.3 Update the ordering tests to the surviving order: root identity is
      evaluated before per-check shape, and there is no longer a status stage
      to be evaluated first.
- [x] 4.4 Evidence: every retained class still refuses, with a unit test each
      and an end-to-end test through the binary asserting empty stdout, a
      non-zero exit code, and a diagnostic naming the exact value.

## 5. BFS — Regression and completeness review

- [x] 5.1 Update the four end-to-end status tests from refusal to emission:
      a required `UNBOUND` check, an optional `UNBOUND` check, a pending review,
      and a blocking pending review. Assert the emitted `result` /
      `checks[].status` values and the exit codes.
- [x] 5.2 Prove the change is additive: `--format human` and `--format json`
      produce byte-identical stdout and the same exit codes. Evidence: the
      pre-existing `src/commands/gate.rs` tests and
      `tests/gate_unbound_check.rs`, all unchanged and passing.
- [x] 5.3 Prove nothing else moved: `git diff --stat` touches only
      `src/gate/platform_result.rs`, `src/gate/platform_result_tests.rs`,
      `src/gate/mod.rs`, `src/cli.rs` (doc text), `src/commands/gate.rs` (doc
      text), `tests/gate_platform_result.rs`, and the OpenSpec package. No
      `Cargo.toml` / `Cargo.lock` change, no new dependency.
- [x] 5.4 Prove `GateStatus`, `NOT_APPLICABLE`/`UNBOUND` semantics,
      `aggregate()`, the adapter tables, the manifest, and the concern
      vocabulary are untouched.
- [x] 5.5 Confirm none of the five incomplete open changes
      (`per-check-gate-timeout`, `bind-declared-verification-command`,
      `declare-spec-purpose-placeholder-code`, `bind-governance-quality-checks`,
      `gate-unbound-check-fails`) was edited, completed, archived, or
      re-scoped, and that this change conflicts with none of them.
- [x] 5.6 Keep both `platform_result.rs` files under the 1,000-line
      source-file-size boundary.
- [x] 5.7 **Measure the residual gap and report it.** Build the release binary
      and run `gate --format platform` against this repository, `forge`, and at
      least two other local repositories; record exit code, stdout byte count,
      and the exact reason. Report the result in `HANDOFF.md` and in the
      orchestrator report. Do **not** fix it here.

## 6. Verification

- [x] 6.1 `cargo fmt`
- [x] 6.2 `cargo test`
- [x] 6.3 `cargo clippy` (default targets)
- [x] 6.4 `openspec validate --changes --strict --no-interactive`
- [x] 6.5 `git diff --check`
- [x] 6.6 `node scripts/check-openspec-change-names.mjs`

The full `driftwatch` Gate runtime, `cargo clippy --all-targets --all-features
-- -D warnings`, heavy-feature workspace tests, and container/integration
suites are deliberately **not** run here; they are the orchestrator's.

## Verification record

Run on 2026-10-12. Results are reported in `HANDOFF.md` and the change is
archived only after every box above is genuinely done.
