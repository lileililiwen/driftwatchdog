# Proposal: Local Gate CLI and runtime-memory integration

## Why

Gate checks must run locally for every relevant change before archive, not only
after a GitHub push. Driftwatchdog should expose a stable CLI, persist Gate
history alongside runtime failures, and keep CI as a second verification layer.

## What Changes

- Add a high-level `gate` command and preserve `check` compatibility.
- Persist Gate plans/results and connect them to existing reports and fingerprints.
- Add local workflow guidance and machine-readable output.
- Keep CI templates as repeatable remote verification, not the first execution.

## Capabilities

### New Capabilities

- `gate-cli-and-memory-integration`: local Gate execution, history, and reporting.

### Modified Capabilities

- `export-and-doctor`: Gate readiness and result diagnostics.
- `correlation-and-ai-context`: optional Gate evidence in reports.

## Impact

Affects CLI, storage migrations, commands, reports, exports, doctor, MCP read
surface, README, HANDOFF, and CI templates. Depends on all prior Gate packages.

## Non-Goals

- No requirement that every project use OpenSpec.
- No remote service or mandatory CI provider.
- No automatic code modification or automatic spec archive.
