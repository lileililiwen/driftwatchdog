# agent-examples Specification

## Purpose
TBD - created by archiving change agent-examples. Update Purpose after archive.
## Requirements
### Requirement: Per-harness MCP examples
Each of `examples/claude-code/`, `examples/opencode/`, `examples/aider/` MUST contain a valid MCP client-config fragment attaching `driftwatch mcp` over stdio.

#### Scenario: Config parses and points at the server
- **WHEN** the consistency test reads any `mcp.json`
- **THEN** it parses as JSON and declares a stdio server whose command is `driftwatch` with `mcp` in its arguments

#### Scenario: Aider fallback documented
- **WHEN** a user reads `examples/aider/workflow.md`
- **THEN** it documents the `driftwatch report --ai > drift.md` attach-to-chat fallback alongside the MCP path

### Requirement: Workflow contract parity
Every `workflow.md` MUST restate the `report --ai` investigation instructions and MUST reference only real, read-only subcommands.

#### Scenario: Stale subcommand reference
- **WHEN** a `workflow.md` names a subcommand absent from `driftwatch --help`
- **THEN** the consistency test fails naming the file and token

#### Scenario: Execution step sneaks in
- **WHEN** a `workflow.md` instructs the agent to invoke `driftwatch run` or `driftwatch check`
- **THEN** the consistency test fails (agents read via MCP/report; execution stays human-driven)

### Requirement: Shared index
`examples/README.md` MUST map each harness to its copy destination and state the stdio-only, read-only caveat.

#### Scenario: New user onboarding
- **WHEN** a user reads `examples/README.md`
- **THEN** they learn which file to copy where for their harness and that no network or write access is involved

