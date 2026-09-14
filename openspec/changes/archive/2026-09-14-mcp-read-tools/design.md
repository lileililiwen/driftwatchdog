## Approach
- Transport: newline-delimited JSON-RPC 2.0 on stdin/stdout, matching MCP `2024-11-05` (`JSONRPCMessage` framing). One request per line; responses carry the matching `id`. `notifications/initialized` is accepted and ignored.
- Handshake: `initialize` returns `protocolVersion` (echo the client's when supported, else negotiate down to `2024-11-05`), `serverInfo: {name: "driftwatchdog", version: <Cargo>}`, `capabilities: {tools: {}}`. No prompts/resources/logging capabilities advertised.
- Tools map 1:1 onto existing builders so output is identical to the CLI:
  - `top_bugs {limit?, days?, tag?}` → `TopRow` list (same shaping as `driftwatch top`).
  - `show_bug {id}` → single fingerprint detail (same shaping as `driftwatch show`); unknown id → tool error with the same `hint:` remediation as the CLI.
  - `ai_report {limit?, days?, tag?}` → Markdown from `render_ai` (same bytes as `report --ai`); cautious wording preserved verbatim.
  - `doctor_status {}` → check list with honest severities (same shaping as `doctor`).
- Input validation: MCP `tools/call` arguments are validated with `serde`; missing/ mistyped args → `-32602` with a message naming the expected schema. Every tool input schema is closed (`additionalProperties: false`).
- Read-only guarantee: the server opens SQLite with `SQLITE_OPEN_READ_ONLY` (via `rusqlite::OpenFlags`); any code path attempting mutation fails at the driver level. No `Command` spawning anywhere in `src/mcp/`.
- Malformed-line policy: a non-JSON line yields a `-32700` parse-error response when an `id` can be recovered, otherwise the line is skipped with a stderr diagnostic; the server never exits on bad input (EOF exits 0).

## Non-goals
- No write/execute tools (see proposal); no HTTP/SSE transport; no prompt templates or sampling support.
