# Proposal: Gate project configuration

## Why

Business projects need to declare profiles, checks, project-runtime commands,
blocking policy, and local rule overrides without embedding tool implementations
or language-specific logic in Driftwatchdog.

## What Changes

- Add a validated project Gate manifest and profile resolution.
- Support conditional concern selection from changed surfaces.
- Keep project commands separate from Gate-tool commands.

## Capabilities

### New Capabilities

- `gate-project-configuration`: portable project Gate manifest and profile policy.

### Modified Capabilities

## Impact

Affects configuration loading, root discovery, doctor diagnostics, CLI errors,
examples, and future gate planning. Depends on `generic-gate-contract`.

## Non-Goals

- No tool downloads or adapter implementations.
- No mandatory OpenSpec configuration.
