# Proposal: MCP read tools

## Why
Coding agents working in a driftwatch-monitored project today must shell out to the CLI and parse human-oriented Markdown. That is fragile: agents re-implement argument handling, misparse output, and have no typed contract for the read surface. The Model Context Protocol (MCP) over stdio gives agents a typed, discoverable interface with zero network exposure. Driftwatch state is already local SQLite plus deterministic report builders, so a read-only tool layer maps directly onto existing library functions (`top`, `show`, `report --ai`, `doctor`) without new storage or services.

## What Changes
- New `driftwatch mcp` subcommand serving a minimal MCP server on stdio (JSON-RPC 2.0, newline-delimited): `initialize`, `notifications/initialized`, `tools/list`, `tools/call`.
- Four read-only tools: `top_bugs`, `show_bug`, `ai_report`, `doctor_status`. Each reuses the existing command builders and export DTOs (`SCHEMA_VERSION`) so the MCP surface can never drift from CLI output.
- Strict failure behavior: unknown method → `-32601`, bad params → `-32602`, malformed input never panics; the server opens the database read-only.
- Docs: README section + `--help` for `mcp`; unit/integration coverage for protocol handling and every tool.

## Capabilities
### New Capabilities
- `mcp-read-tools`: stdio MCP server with typed read-only tools over local driftwatch state.
### Modified Capabilities
- None.

## Impact
Affects: `src/cli.rs`, `src/main.rs`, `src/mcp/` (new: `server.rs`, `tools.rs`), `src/commands/meta.rs` (help text only if needed), `tests/mcp.rs` (new), `README.md`, `ROADMAP.md`, `HANDOFF.md`, `openspec/specs/mcp-read-tools/spec.md` (new via archive).

## Non-goals
- No HTTP/SSE/streamable transports: stdio only, preserving the offline, non-telemetric default.
- No write or execute tools: `run`, `check`, `link`, `unlink`, `gc` stay CLI-only. An agent tool that executes arbitrary child commands would be a sandbox escape vector.
- No new dependencies: `serde`/`serde_json` already cover the protocol.
