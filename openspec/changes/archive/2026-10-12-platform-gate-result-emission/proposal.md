# Proposal: Emit a `platform.gate-result` document from `driftwatch gate`

## Why

The workspace already has an authoritative wire contract for a Gate outcome:
`platform-contracts/schemas/gate-result.schema.json`. Nothing produces it.

Every machine-readable surface Driftwatchdog owns today is private to
Driftwatchdog:

| Surface | Shape | Portable? |
|---|---|---|
| human render | table | no |
| `--format json` | `{status, blocked, failures, pending_reviews, not_applicable, unbound, exemption_budget, results}` | no — the key set is Driftwatchdog's own |
| `gate_runs.results_json` | `drwatch-gate-run/1` | no |
| `gate evidence-export` | workspace-governance `release_evidence` vocabulary | vocabulary is Workspace Governance's, not the platform's |

So a portfolio consumer that wants to ask "may this deploy proceed?" has no
contract to code against. It either scrapes the human table or hard-codes
Driftwatchdog's five-state vocabulary — which is exactly what the platform
contract exists to prevent.

The blocking problem is **not** writing JSON. It is that the two vocabularies
do not have the same shape, and the honest answer for most runs is *refusal*:

| Driftwatchdog `GateStatus` | platform `result` / `checks[].status` | Same claim? |
|---|---|---|
| `PASS` | `passed` | yes |
| `FAIL` | `failed` | yes |
| `NOT_APPLICABLE` | `skipped` | judgement, documented |
| `REVIEW_REQUIRED` | — | **no**. "A human has not decided yet" has no platform state. |
| `UNBOUND` | — | **no**. `src/gate/types.rs:7-11` says `UNBOUND` "must never share a status" with `NOT_APPLICABLE`. Coercing it to `skipped` violates Driftwatchdog's own invariant. |

A run whose aggregate is `REVIEW_REQUIRED` or `UNBOUND`, or that contains any
check in those states, therefore **cannot** produce a conforming document. The
platform has four states and the honest mapping uses three of them;
`errored` is never emitted, because every Driftwatchdog execution failure
(timeout, spawn failure, malformed envelope, contradiction) is already
normalized to `REVIEW_REQUIRED`, and `REVIEW_REQUIRED` is itself refused.

The second, quieter problem is identity. Driftwatchdog check ids come from a
project manifest and are validated only as *non-empty*
(`src/gate/manifest.rs:1045`). The platform requires
`^[a-z][a-z0-9_.-]{0,63}$`. `L1-configuration`, `MyCheck`, and
`l1-configuration` all fail, and the first two are not even distinct from the
third under any lowercasing slug. That is a third refusal class, not a
truncation.

## What Changes

- Add `--format platform` to `driftwatch gate` as a new, additive output
  value. `--format human` and `--format json` are untouched and keep their
  bytes and exit codes.
- Add `src/gate/platform_result.rs`: a pure, dependency-free mapping from a
  completed gate run (`AggregateOutcome` + `Vec<GateResult>` + run identity)
  to a `platform.gate-result` document, or a typed refusal.
- Refuse — emit nothing on stdout, write one typed diagnostic on stderr, exit
  non-zero — whenever the run cannot be represented without losing or
  inventing a claim.
- Add unit tests for every mapping row and every refusal class, plus an
  end-to-end fixture set that asserts stdout is empty and the exit code is
  non-zero for each refusal.

## What This Change Does Not Do

- It does not touch Forge, and it does not modify the `platform-contracts`
  repository or its schema.
- It does not add a JSON-Schema validator dependency. Forge already vendors
  the schema; Driftwatchdog's job is to emit an honest document or refuse, not
  to re-validate its own output.
- It does not restructure the gate pipeline, the aggregate policy, the
  manifest, the concern vocabulary, or the five `GateStatus` states.
- It does not migrate `gate evidence-export`; that subcommand keeps its own
  two-value `--format`.
- It does not edit, complete, archive, or re-scope any of the five incomplete
  open changes.

## Impact Map (BFS)

| Surface | Change | Risk |
|---|---|---|
| `src/gate/platform_result.rs` (new) | mapping + refusal types | none; pure, no IO |
| `src/gate/mod.rs` | module registration + re-exports | none |
| `src/cli.rs` | new `GateOutputFormat` enum for `GateArgs.format` | additive; `GateFormatArg` untouched so `evidence-export` is unchanged |
| `src/commands/gate.rs` | one new `match` arm + exit code 2 | additive; existing arms byte-identical |
| `src/commands/gate.rs` tests | `gate_args` helper uses the new enum | mechanical |
| `tests/gate_platform_result.rs` (new) | end-to-end emission and refusal | none |
| `openspec/specs/gate-platform-result-export/spec.md` (new) | canonical capability spec | none |

**Consumers.** None inside this repository. `driftwatch check`, the MCP
read-only tools, `driftwatch doctor`, and `gate evidence-export` do not read the
new format.

**Non-consumers of the new exit code.** `EXIT_UNREPRESENTABLE = 2` is new and
only reachable through `--format platform`. No existing path can reach it.

## Package Boundary and Split Assessment

One package. The mapping table, the refusal set, the CLI flag, and the
canonical spec are not separable: emitting the document without the refusal
path would ship a producer that silently coerces, which is the failure this
change exists to prevent.