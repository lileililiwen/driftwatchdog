# Aider + Driftwatch

Aider has no native MCP stdio attach in every version, so the
recommended path is the portable `report --ai` fallback. The MCP
config in this directory is still provided for Aider versions or
wrappers that do support stdio MCP servers; verify against your
local Aider docs first.

## 1. Install the binary

Aider shells out to whatever is on `PATH` via `--editor` or as part
of a script. Install Driftwatch once per developer machine using the
channel that matches your environment (binary naming rule:
`driftwatch` from npm, `driftwatchdog` from Cargo / the shell
installer).

    # npm (recommended)
    npm i -g driftwatchdog

    # shell installer (Linux, macOS)
    curl -fsSL https://github.com/lileililiwen/driftwatchdog/releases/latest/download/install.sh | sh

    # cargo
    cargo install driftwatchdog

Verify the binary is on `PATH`:

    driftwatch --help
    driftwatch doctor

## 2. Recommended path: `report --ai` fallback

The portable integration does not require MCP. The human renders
the Markdown report in a shell and Aider reads the file as
context.

    # human-side, after a failing run
    driftwatch report --ai > drift.md

    # in Aider, add the file as context before asking the question
    aider --read drift.md
    # or, in a chat already running, add the file with /read

The Markdown is identical to what the MCP `ai_report` tool returns,
including recurring failures, current spec violations, possible
relationships, and the investigation task.

## 3. Optional path: MCP stdio (if your Aider supports it)

If you are running an Aider build that exposes an MCP stdio slot
(some third-party wrappers do), copy `mcp.json` into the Aider MCP
configuration directory and reload. The four read-only tools
(`top_bugs`, `show_bug`, `ai_report`, `doctor_status`) attach the
same builders the CLI uses.

    mkdir -p ~/.aider/mcp
    cp examples/aider/mcp.json ~/.aider/mcp/driftwatch.json

If the binary is installed as `driftwatchdog` (Cargo / shell installer
path) and not `driftwatch`, change the `command` field in the JSON
to match. Do not add any other args — the server speaks JSON-RPC 2.0
on stdio and ignores unrelated flags.

## 4. The investigation loop

The human drives every command that mutates state. Aider observes
the output, asks Driftwatch's MCP tools (or reads the rendered
report) for context, and helps the human form a hypothesis and
write a regression test. The following steps restate the same
instructions the `report --ai` command emits, so behaviour stays
consistent with the documented contract.

1. **Human reproduces.** The human runs the failing command through
   Driftwatch so the occurrence is recorded and fingerprinted. The
   exit line prints a `bug:<id>` token when the failure matches a
   previously seen fingerprint.
2. **Top-bugs scan.** If MCP is enabled, Aider calls the `top_bugs`
   tool (optionally with `limit` / `days` / `tag`) to list every
   recurring fingerprint ordered by occurrence count. Otherwise,
   Aider reads the `Recurring failures` table in `drift.md`.
3. **Show-bug details.** For each row that is plausibly related,
   Aider calls `show_bug` (MCP) with the `id` to see the canonical
   message, recent commits, and recent evidence excerpts.
   Otherwise, the human runs `driftwatch show <bug-id>` in a shell
   and adds the output to the chat.
4. **AI report.** The human re-renders `driftwatch report --ai >
   drift.md` whenever the underlying state has changed and Aider
   re-reads the file.
5. **Spec review.** Aider reads the `Source` column entries from
   the spec violations section. It helps the human form a
   hypothesis about the relationship between the bug and the spec
   before any application behaviour is changed. **Do not modify
   specs to silence warnings.**
6. **Human adds a regression test.** The human writes (or updates)
   a test that covers the failure, then re-runs the failing
   command through Driftwatch. Aider confirms the fingerprint count
   for that bug stopped incrementing.
7. **Re-run.** When the test passes, Aider calls `top_bugs` again
   to confirm the bug is no longer in the top list (or its count
   has stopped growing). Without MCP, the human re-renders
   `drift.md` and Aider re-reads it.

## 5. Boundaries

- The MCP server is **read-only**. There are no write or execution
  tools. The investigation loop reads state; the human still runs
  every Driftwatch command.
- The server opens `.driftwatch/state.db` with
  `SQLITE_OPEN_READ_ONLY`, so even a misconfigured tool cannot
  corrupt the local database.
- No network calls, no telemetry, no LLM API. Driftwatch stays
  local-first.
