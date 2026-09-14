## 1. Protocol core
- [x] 1.1 Add `driftwatch mcp` subcommand: stdin line loop, JSON-RPC dispatch (`initialize`, `notifications/initialized`, `tools/list`, `tools/call`), `-32601`/`-32602`/`-32700` errors, EOF exits 0, never panics on malformed input.
- [x] 1.2 Open the database read-only (`rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY`) from the MCP path; add a test asserting mutation fails at the driver level.

## 2. Tools
- [x] 2.1 Implement `top_bugs` + `show_bug` reusing the `top`/`show` builders; `show_bug` surfaces the CLI `hint:` remediation on unknown ids.
- [x] 2.2 Implement `ai_report` reusing `render_ai` (byte-identical to `report --ai`); cautious wording preserved verbatim.
- [x] 2.3 Implement `doctor_status` reusing the doctor report builder with honest severities.
- [x] 2.4 Unit/integration coverage: handshake, `tools/list` schema closure, every tool happy path + error path, parity test (`ai_report` bytes vs CLI), malformed-line policy.

## 3. Docs
- [x] 3.1 Document `driftwatch mcp` in README (stdio only, read-only tool list, client-agnostic example) and extend `--help`; record module in HANDOFF module map.

## 4. Change completion workflow (HANDOFF.md routine — repeat for this change)
- [x] 4.1 Run verification gates: `cargo fmt --check && cargo test && cargo clippy --all-targets --all-features -- -D warnings && openspec validate --changes --strict --no-interactive`.
- [x] 4.2 Confirm `openspec/changes/mcp-read-tools/tasks.md` has every box ticked.
- [x] 4.3 Archive with `openspec archive mcp-read-tools -y` (omit `--skip-specs`: adds `mcp-read-tools` capability).
- [x] 4.4 Stage and commit implementation + archive: `git add -A && git commit -m "Implement mcp-read-tools"`.
- [x] 4.5 Update `HANDOFF.md` (status, next action, module map) and commit: `git add HANDOFF.md && git commit -m "Update HANDOFF after mcp-read-tools"`.
- [x] 4.6 Re-run `openspec validate --changes --strict --no-interactive` to confirm the change list still resolves.
