# Proposal: Agent examples (Claude Code / OpenCode / Aider)

## Why
v0.6 promises copy-paste integration for the three agent harnesses the README audience actually uses, but today there is no tested example for any of them. Untested docs drift: config keys rename, subcommand names change, and the example silently breaks. Each harness needs two things: how to attach driftwatch as an MCP server (or fall back to `report --ai` Markdown), and the workflow contract (investigate recurrence, review specs, add regression coverage, never edit specs to silence warnings).

## What Changes
- `examples/claude-code/`, `examples/opencode/`, `examples/aider/` — one directory per harness, each with an MCP client-config fragment pointing at `driftwatch mcp` over stdio plus a `workflow.md` describing the driftwatch loop in that harness's idiom.
- `examples/README.md` — shared index: which file to copy where, stdio-only caveat, and the `report --ai > drift.md` fallback for harnesses without MCP support.
- Consistency test (`tests/packaging/test_agent_examples.sh` style): every JSON fragment parses; every `driftwatch <sub>` token in `workflow.md` names a real subcommand (checked against `driftwatch --help`); every example references the read-only tool list, never `run`/`check` execution.
- README section linking the examples.

## Capabilities
### New Capabilities
- `agent-examples`: tested copy-paste MCP + workflow examples for Claude Code, OpenCode, and Aider.
### Modified Capabilities
- None.

## Impact
Affects: `examples/` (new), `tests/packaging/test_agent_examples.sh` (new), `README.md`, `ROADMAP.md`, `HANDOFF.md`, `openspec/specs/agent-examples/spec.md` (new via archive).

## Dependencies
- Depends on `mcp-read-tools` (examples attach to the MCP server); implement after it archives.

## Non-goals
- No harness plugins, skills, or auto-fix behavior: examples are config + prose. They MUST NOT instruct the agent to modify specs to silence warnings or to execute unchecked commands.
- No new product features or CLI changes.
