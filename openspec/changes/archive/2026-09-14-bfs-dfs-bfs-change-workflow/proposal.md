# Proposal: BFS-DFS-BFS change workflow

## Why

Future Driftwatchdog changes will span CLI, storage, external tools, evidence,
and optional context providers. A one-pass task list can miss callers and
boundaries before implementation. The repository needs an explicit
breadth-first, depth-first, breadth-first workflow for every change.

## What Changes

- Define proposal.md as the initial BFS impact map.
- Define design.md as the complete boundary and architecture contract.
- Require tasks.md to contain BFS analysis, DFS implementation, and BFS
  regression/completion groups.
- Add the workflow to repository guidance and handoff expectations.

## Capabilities

### New Capabilities

- `bfs-dfs-bfs-change-workflow`: structured change planning and completion order.

### Modified Capabilities

## Impact

Affects `AGENTS.md`, `HANDOFF.md`, OpenSpec authoring practice, and every future
change package. No product runtime behavior changes.

## Non-Goals

- No Gate Runtime implementation.
- No OpenSpec-specific coupling in Driftwatchdog core.
- No automatic task completion or archive operation.
