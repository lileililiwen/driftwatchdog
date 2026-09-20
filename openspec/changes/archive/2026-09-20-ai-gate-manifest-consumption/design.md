## Context

`gate.toml` is driftwatchdog's native manifest, but bootstrapped business
projects record policy in `.ai-gate/gate.yaml` and name driftwatchdog as
the runtime. The two describe the same plan (profile, selected concerns,
blocking policy, project commands, rule-pack identity, context providers).
The conversion must reuse the existing resolve/execute/aggregate/persist
pipeline so no second Gate implementation appears in a business repo
(the bootstrap skill forbids copying shared Gate logic into projects).

## Goals / Non-Goals

**Goals:** resolve and execute `.ai-gate/gate.yaml` through the current
Gate pipeline; strict, actionable validation; deterministic identity.

**Non-Goals:** no rule-pack evaluator, no YAML for checker config, no
change to execution or aggregation semantics.

## Decisions

- **Parser:** `yaml-rust2` (maintained, MSRV-compatible with the crate's
  1.74 floor). `saphyr` 0.1 and `yaml-rust2` 0.13 require rustc 1.85 and
  are auto-excluded; `serde_yaml` is unmaintained. Manual `Value` mapping
  gives precise, actionable errors and avoids a serde dependency.
- **Single model:** the YAML document is parsed into an intermediate
  `YamlGatePolicy`, then converted to the existing `GateManifest` struct.
  All downstream `resolve`/`render_plan`/`execute_plan`/`aggregate`/persist
  code is unchanged, so behavior parity with `gate.toml` is structural.
- **Path precedence:** `manifest_path` returns the first of
  `gate.toml`, `.driftwatch/gate.toml`, `.ai-gate/gate.yaml`. A project
  keeps one manifest source; native `gate.toml` still wins.
- **Runtime guard:** `runtime` is optional. Absent or `driftwatchdog` →
  this runtime executes. Any other value → the loader returns a distinct
  `NotThisRuntime` outcome; `gate` prints a clear "not for driftwatchdog"
  line and exits 0 without persisting (it must not silently PASS a
  foreign runtime's plan, nor block on checks it does not own).
- **Blocking list mapping:** `blocking` accepts the status names
  `FAIL` and `REVIEW_REQUIRED`. `FAIL` is always enforced by the existing
  aggregator (presence or absence does not loosen it); `REVIEW_REQUIRED`
  presence sets `review_required_blocks = true`, absence sets `false`.
  An unknown status name fails resolution with a did-you-mean hint.
- **Domain profiles:** `profile` may be a built-in (`backend`, `frontend`,
  `full`, `minimal`) or a project-defined name. A project-defined profile
  is valid only when the manifest supplies the checks it selects; the
  converted manifest records the profile string and `rule_pack` value as
  plan identity (rendered in the dry-run and stored in the digest). The
  built-in profile default set is not reinterpreted.
- **Checks and commands:** `checks` maps concern id → boolean (selected /
  deselected). An optional `commands` map binds concern id → command
  string; a selected concern with no command follows the existing
  `NOT_APPLICABLE`/review path. A `commands` entry for a deselected or
  unknown concern is an error. Empty command strings fail before execution
  (existing rule).
- **Identity:** `manifest_digest` is computed from the canonical TOML
  rendering of the converted `GateManifest`, so a YAML manifest and its
  `gate.toml` equivalent share a digest and the persisted `rule_pack`
  string records provenance.

## Risks / Trade-offs

- Manual YAML mapping risks missing a schema nuance; mitigated by strict
  unknown-field rejection, a golden fixture, and parity tests against the
  equivalent `gate.toml`.
- Precedence means a project with both files silently uses `gate.toml`;
  documented explicitly and covered by a test.
- The foreign-runtime exit-0 could be read as a pass; the printed line
  names the runtime and states nothing was executed, and no `gate_runs`
  row is written, so history stays honest.
