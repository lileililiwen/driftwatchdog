# Proposal: Adopt `platform.gate-result` major 1.0.0 in the driftwatchdog producer

## Why

`platform-contracts` bumped `platform.gate-result` from `0.1.0` to `1.0.0`
(change `gate-result-major-review-state`, commits `3cfd310`, `5e8a6ca`). The
`result` and `checks[].status` enums went from four values to six, symmetrically
at both levels:

```
passed | failed | errored | skipped | review_required | unbound
```

The registry record (`schemas/registry.json`,
`contracts["platform.gate-result"]`) is authoritative: `current_version:
"1.0.0"`, `supported_major_versions: [0, 1]`, and a `narrowing` block with
`drops: [0]`, `state: "declared"`. **Major 0 is still served.** It is dropped in
a separate change, and its own `notice` names this change as the precondition:
*"Major 0 is removed only in a separate change that moves this record to state
'enforced' once the sole producer, driftwatchdog, emits platform.gate-result/1.0.0."*

Driftwatchdog is that sole producer. `platform-gate-result-emission`
(archived `2026-10-12`) built the emitter against the 0.1.0 four-value
vocabulary and therefore had to **refuse** the two states driftwatchdog is
named for:

| Driftwatchdog `GateStatus` | 0.1.0 (live today) | 1.0.0 (this change) |
|---|---|---|
| `PASS` | `passed` | `passed` |
| `FAIL` | `failed` | `failed` |
| `NOT_APPLICABLE` | `skipped` | `skipped` |
| `REVIEW_REQUIRED` | **refused** (`Refusal::AggregateStatus` / `Refusal::CheckStatus`) | `review_required` |
| `UNBOUND` | **refused** | `unbound` |
| — | never emitted | never emitted (`errored`) |

That refusal logic is now obsolete. Worse, it was *load-bearing in the wrong
direction*: the schema at major 1 defines `unbound` with exactly the claim
Driftwatchdog's own status model makes — *"the check was asserted and no command
was ever bound to it, so nothing ran; this is a configuration defect and is never
an exemption"* — and `review_required` as *"a human decision is still open and
no one has decided the outcome"*. The contract learned the vocabulary instead of
Driftwatchdog losing information.

Verified live before this change: the format refuses on both the driftwatchdog
repository and the forge repository, exiting `2` with **zero stdout bytes**.

A second, quieter gap: the emitted document does not say which contract
version it claims. `additionalProperties: true` on the root makes a declaration
possible, and this repository already has the precedent at
`src/checker/json.rs:41` — `CHECKER_REPORT_CONTRACT = "driftwatch-checker/0.1.0"`,
emitted as a `contract` field. A producer that refuses to name its own version
is asking the consumer to guess.

## What Changes

- `src/gate/platform_result.rs`:
  - `PlatformStatus` gains `ReviewRequired` and `Unbound`; `errored` stays
    defined and stays unreachable.
  - `map_status` becomes a total `GateStatus -> PlatformStatus` function with
    no failure arm. `StatusMapping` (whose only failure value was
    `Unrepresentable`) is retired with it.
  - `Refusal::AggregateStatus` and `Refusal::CheckStatus` are **deleted**.
  - A new `contract` root property is emitted, carrying the new constant
    `PLATFORM_GATE_RESULT_CONTRACT = "platform.gate-result/1.0.0"`.
  - The seven remaining refusal classes — `gate_id`, `pipeline`, `subject_ref`,
    `evaluated_at`, `check_id`, `detail`, `evidence_ref` — are unchanged in
    behaviour, ordering, and wording.
- `src/gate/mod.rs`: re-export the new constant, drop `StatusMapping`.
- `src/cli.rs`, `src/commands/gate.rs`: doc text only; no flag, enum value,
  exit code, or emitted byte changes.
- Tests updated to the new behaviour, with added coverage for `review_required`
  and `unbound` at both the aggregate and per-check level, plus a test that
  `errored` is still never produced.

## What This Change Does Not Do

- It does not weaken, truncate, or drop any surviving refusal class. A refusal
  is still a correct outcome and still exits `2` with empty stdout.
- It does not coerce anything. `REVIEW_REQUIRED` becomes `review_required`
  because that is what it claims; `UNBOUND` becomes `unbound` for the same
  reason. Neither is mapped onto `failed`, `skipped`, or `passed`.
- It does not change `GateStatus`, `NOT_APPLICABLE`/`UNBOUND` semantics,
  `aggregate()`, the manifest, the concern vocabulary, or the adapters.
- It does not change the `--format human` and `--format json` paths: same
  bytes, same exit codes.
- It does not add a JSON-Schema dependency. The mapping stays hand-written.
- It does not touch `platform-contracts` or Forge, and it does not move the
  registry `narrowing.state` to `enforced` — that is a separate change in
  another repository.
- It does not edit, complete, archive, or re-scope the five incomplete open
  changes.

## Known residual gap this change does NOT fix (reported, not papered over)

Even with every status representable, `--format platform` still refuses on real
repositories for two reasons that have nothing to do with status. Both are
measured live and both survive major 1.0.0 unchanged:

1. **`check_id` pattern.** `^[a-z][a-z0-9_.-]{0,63}$`. Driftwatchdog check ids
   are project-manifest text validated only as non-empty
   (`src/gate/manifest.rs:1045`). `L1-configuration` does not match.
2. **`detail` max length.** 512 characters. Driftwatchdog's own
   `MAX_DIAGNOSTIC_BYTES` is 2,048. On the driftwatchdog repository,
   `declared-verification` carries a 2,049-character diagnostic and the document
   is refused for that reason alone.

1.0.0 relaxed neither bound. The honest resolution needs a decision that is out
   of scope here; see `design.md` §8 for the three options and the
   recommendation. **No fix is implemented by this change.**

## Impact Map (BFS)

| Surface | Change | Risk |
|---|---|---|
| `src/gate/platform_result.rs` | status vocabulary, mapping, `contract`, refusal set | the whole surface; pure, no IO |
| `src/gate/platform_result_tests.rs` | unit tests rewritten for the new mapping | test-only |
| `src/gate/mod.rs` | re-export list | none |
| `src/cli.rs` | doc text on `GateOutputFormat::Platform` | none; no parse change |
| `src/commands/gate.rs` | doc text on `print_platform` and the exit constant | none; no byte change |
| `tests/gate_platform_result.rs` | end-to-end emission + retained refusals | test-only |
| `openspec/specs/gate-platform-result-export/spec.md` | canonical capability spec | none |

**Consumers inside this repository.** None. No MCP tool, `doctor` check,
`evidence-export`, or persisted-record reader consumes the platform document.

**Consumers outside it.** Forge vendors the same schema and *emits*
`platform.gate-result` from its own evidence store
(`forge/src/contract/mod.rs::emit_gate_result`); it never reads
`driftwatch gate --format platform` output, and its consumption path for
Driftwatchdog is `driftwatch gate --format json` — Driftwatchdog's own
vocabulary. Forge's vendored copy is still the 0.1.0 four-value enum and is
updated by Forge's own `scripts/sync-contracts.mjs`; that is Forge's change,
not this one. Its own documents only ever use values that remain legal in 1.x.

## Package Boundary and Split Assessment

One package. The vocabulary, the refusal set, the version declaration, the CLI
doc text, and the canonical spec are not separable: shipping the version
declaration while still refusing `REVIEW_REQUIRED` would claim a 1.0.0 document
this producer cannot produce.
