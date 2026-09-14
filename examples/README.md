# Driftwatch agent examples

Copy-paste MCP client configs and investigation workflows for the
agent harnesses the README audience actually uses. Each directory
is a self-contained drop-in for one harness.

## What's here

| Directory        | Harness       | MCP config (`mcp.json`)              | Workflow                       |
| ---------------- | ------------- | ------------------------------------ | ------------------------------ |
| `claude-code/`   | Claude Code   | stdio MCP server `driftwatch`        | Recurrence-investigation loop. |
| `opencode/`      | OpenCode      | stdio MCP server `driftwatch`        | Recurrence-investigation loop. |
| `aider/`         | Aider         | stdio MCP server (optional attach)   | Recurrence-investigation loop with the `report --ai > drift.md` fallback. |

All three workflows restate the same investigation instructions the
`driftwatch report --ai` command emits, so behaviour stays
consistent with the documented contract regardless of which harness
the agent is running inside.

## Copy destinations

| Harness     | Project-level path          | User-level path (alternative) |
| ----------- | --------------------------- | ----------------------------- |
| Claude Code | `.mcp.json` (project root)  | `~/.claude/mcp.json`          |
| OpenCode    | `opencode.json` (project root) | `~/.config/opencode/mcp.json` |
| Aider       | (see `aider/workflow.md` §3) | `~/.aider/mcp/driftwatch.json` (MCP path; otherwise the `report --ai` fallback applies) |

The project-level files are safe to commit: the configs contain
nothing but the public binary name and a single `mcp` arg.

## Caveats

- **stdio-only.** Driftwatch's MCP transport is newline-delimited
  JSON-RPC 2.0 on stdio. There is no HTTP, SSE, or WebSocket
  variant. The `mcp.json` fragments in this directory all use
  `"type": "stdio"`.
- **Read-only.** The server advertises exactly four tools:
  `top_bugs`, `show_bug`, `ai_report`, `doctor_status`. None of
  them write state, spawn child processes, or perform network
  calls. The database is opened with `SQLITE_OPEN_READ_ONLY`, so
  a misconfigured tool cannot corrupt local state.
- **Binary naming rule.** Installations from npm expose the
  binary as `driftwatch`; installations from Cargo and the shell
  installer expose it as `driftwatchdog`. The `mcp.json` snippets
  in this directory assume `driftwatch`; change the `command`
  field if your install path is different.
- **No auto-fix.** The investigation loop ends with a regression
  test, not a code change. Agents MUST NOT edit specs to silence
  warnings, and MUST NOT call `driftwatch run` or `driftwatch
  check` as part of the loop — those are human-driven commands.

## Verifying the examples

The bash packaging suite ships a consistency test
(`tests/packaging/test_agent_examples.sh`) that asserts:

1. Every `mcp.json` parses as JSON and declares a stdio server
   whose command is `driftwatch` with `mcp` in its arguments.
2. Every `workflow.md` names only subcommands that exist in
   `driftwatch --help`.
3. No `workflow.md` instructs the agent to call `driftwatch run`
   or `driftwatch check`.

Run it locally with:

    sh tests/packaging.sh
