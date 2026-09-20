# Proposal: Consume business-project Gate policy from .ai-gate/gate.yaml

## Why

Bootstrapped business repositories declare their Gate policy in
`.ai-gate/gate.yaml` with `runtime: driftwatchdog`, a domain profile
(`browser-extension`, `web-saas`, ...), a rule-pack version, a selected
check map, and a blocking status list. Driftwatchdog is the named shared
Gate Runtime but can only read `gate.toml` with four built-in profiles,
so every real business project's declared policy is currently
unexecutable. The CLI must consume the convention it is referenced by.

## What Changes

- Resolve `<root>/.ai-gate/gate.yaml` as a third Gate manifest source
  (after `gate.toml` and `.driftwatch/gate.toml`).
- Parse the declared schema (`version`, `runtime`, `profile`,
  `rule_pack`, `checks`, `blocking`, plus optional `commands`,
  `project_commands`, `contexts`) into the existing Gate manifest model
  with strict unknown-field rejection and actionable errors.
- Honor `runtime`: when it names another tool, driftwatchdog reports the
  policy is not for it and gates nothing.
- Support project-defined domain profiles via an optional `[profiles.<name>]`
  table in `gate.toml`; YAML manifests carry their profile and
  `rule_pack` as plan identity.
- Map `checks` selections and `commands` bindings onto the existing
  execution path (declared command → project-runtime adapter; no command
  → explicit `NOT_APPLICABLE`/review).

## Capabilities

### New Capabilities

### Modified Capabilities

- `gate-project-configuration`: additional manifest source, domain
  profiles, and blocking-list mapping.

## Impact

Affects `src/gate/manifest.rs` (schema + resolution), a new
`src/gate/aigate.rs` (YAML model + conversion), `manifest_path`/`load`
callers (`commands/gate.rs`, doctor gate history), Cargo dependencies
(one maintained YAML parser), README Gate documentation, and tests.
Execution, aggregation, persistence, and `driftwatch check` semantics are
unchanged. No network access is added.

## Non-Goals

- No bundled rule-pack implementations (the checks map remains the
  executable selection).
- No YAML support in `driftwatch.toml` (checker config stays TOML).
- No changed blocking semantics; `FAIL` always blocks as today.
