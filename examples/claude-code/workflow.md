# Claude Code + Driftwatch

Attach Driftwatch's read-only MCP server to Claude Code and run the
recurrence-investigation loop on every failing run.

## 1. Install the binary

Claude Code shells out to whatever is on `PATH`. Install Driftwatch
once per developer machine using the channel that matches your
environment (binary naming rule: `driftwatch` from npm, `driftwatchdog`
from Cargo / the shell installer — Claude Code calls whatever name is
in this `mcp.json`).

    # npm (recommended for JS/TS projects)
    npm i -g driftwatchdog

    # shell installer (Linux, macOS)
    curl -fsSL https://github.com/lileililiwen/driftwatchdog/releases/latest/download/install.sh | sh

    # cargo
    cargo install driftwatchdog

Verify the binary is on `PATH`:

    driftwatch --help
    driftwatch doctor

## 2. Install the MCP config

Copy `mcp.json` from this directory into the Claude Code project
configuration location. Claude Code reads MCP servers from
`.mcp.json` at the project root (committed config) and from
`~/.claude/mcp.json` (user-level config). Pick one:

    # project-level (committed; the recommended shape)
    cp examples/claude-code/mcp.json .mcp.json

    # user-level (every Claude Code session sees it)
    mkdir -p ~/.claude
    cp examples/claude-code/mcp.json ~/.claude/mcp.json

Restart Claude Code so the new server is registered. Confirm with
`claude /mcp`: the `driftwatch` server should appear with four
read-only tools (`top_bugs`, `show_bug`, `ai_report`, `doctor_status`).

If the binary is installed as `driftwatchdog` (Cargo / shell installer
path) and not `driftwatch`, change the `command` field in `mcp.json`
to match. Do not add any other args — the server speaks JSON-RPC 2.0
on stdio and ignores unrelated flags.

## 3. The investigation loop

The human drives every command that mutates state. Claude Code
observes the output, asks Driftwatch's MCP tools for context, and
helps the human form a hypothesis and write a regression test. The
following steps restate the same instructions the `report --ai`
command emits, so behaviour stays consistent with the documented
contract.

1. **Human reproduces.** The human runs the failing command through
   Driftwatch so the occurrence is recorded and fingerprinted. The
   exit line prints a `bug:<id>` token when the failure matches a
   previously seen fingerprint.
2. **Top-bugs scan.** Claude Code calls the `top_bugs` MCP tool
   (optionally with `limit` / `days` / `tag`) to list every recurring
   fingerprint ordered by occurrence count.
3. **Show-bug details.** For each row that is plausibly related,
   Claude Code calls `show_bug` with the `id` to see the canonical
   message, recent commits, and recent evidence excerpts.
4. **AI report.** Claude Code calls `ai_report` to fetch the same
   Markdown `driftwatch report --ai` prints, including recurring
   failures, current spec violations, possible relationships, and
   the investigation task.
5. **Spec review.** Claude Code reads the `Source` column entries
   from the spec violations section. It helps the human form a
   hypothesis about the relationship between the bug and the spec
   before any application behaviour is changed. **Do not modify
   specs to silence warnings.**
6. **Human adds a regression test.** The human writes (or updates) a
   test that covers the failure, then re-runs the failing command
   through Driftwatch. Claude Code confirms the fingerprint count
   for that bug stopped incrementing.
7. **Re-run.** When the test passes, Claude Code calls `top_bugs`
   again to confirm the bug is no longer in the top list (or its
   count has stopped growing).

## 4. Fallback (no MCP)

If MCP cannot be enabled, the human renders the report in a shell
and Claude Code reads the resulting file:

    # human-side
    driftwatch report --ai > drift.md

    # in the Claude Code session
    "@drift.md investigate the top recurring bug"

The two paths produce the same Markdown; MCP just avoids the manual
file handoff.

## 5. Boundaries

- The MCP server is **read-only**. There are no write or execution
  tools. The investigation loop reads state; the human still runs
  every Driftwatch command.
- The server opens `.driftwatch/state.db` with
  `SQLITE_OPEN_READ_ONLY`, so even a misconfigured tool cannot
  corrupt the local database.
- No network calls, no telemetry, no LLM API. Driftwatch stays
  local-first.
