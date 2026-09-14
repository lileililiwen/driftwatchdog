## 1. Example collateral
- [x] 1.1 Add `examples/{claude-code,opencode,aider}/mcp.json`: stdio fragment (`driftwatch` + `mcp` args), one copy destination note each.
- [x] 1.2 Add `examples/{claude-code,opencode,aider}/workflow.md`: reproduce → `top_bugs`/`show_bug` → `ai_report` → spec review → regression test loop, reusing the `report --ai` instructions verbatim; Aider includes the `report --ai > drift.md` fallback.
- [x] 1.3 Add `examples/README.md`: harness→destination map, stdio-only + read-only caveat, fallback pointer.

## 2. Consistency test
- [x] 2.1 Add packaging-style test: JSON parses, every `driftwatch <sub>` token exists in `--help`, no `run`/`check` agent steps, `mcp.json` declares the stdio server. Wire into `tests/packaging.sh` discovery (auto-picks `test_*.sh`).

## 3. Docs
- [x] 3.1 Link the examples from README (Integrations section); record in HANDOFF module map.

## 4. Change completion workflow (HANDOFF.md routine — repeat for this change)
- [x] 4.1 Run verification gates: `cargo fmt --check && cargo test && cargo clippy --all-targets --all-features -- -D warnings && openspec validate --changes --strict --no-interactive`.
- [x] 4.2 Confirm `openspec/changes/agent-examples/tasks.md` has every box ticked.
- [x] 4.3 Archive with `openspec archive agent-examples -y` (omit `--skip-specs`: adds `agent-examples` capability).
- [x] 4.4 Stage and commit implementation + archive: `git add -A && git commit -m "Implement agent-examples"`.
- [x] 4.5 Update `HANDOFF.md` (status, next action, module map) and commit: `git add HANDOFF.md && git commit -m "Update HANDOFF after agent-examples"`.
- [x] 4.6 Re-run `openspec validate --changes --strict --no-interactive` to confirm the change list still resolves.
