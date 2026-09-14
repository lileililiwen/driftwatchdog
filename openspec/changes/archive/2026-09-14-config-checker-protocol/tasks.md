## 1. Config validation
- [x] 1.1 Add `deny_unknown_fields` + duplicate/empty/zero-timeout validators with hint errors; add tests for each scenario.
- [x] 1.2 Jail `working_dir` to canonical project root; add escape + cwd-independence tests.
- [x] 1.3 Add committable `driftwatch.toml.example`; keep `driftwatch.toml` git-ignored.

## 2. Protocol strictness
- [x] 2.1 Allow-ignore unknown top-level fields; require `alerts`; add size/count caps; `{}`-is-error test.
- [x] 2.2 Map dry-run/parse-failure to explicit statuses with diagnostics; checker-isolation test (one malformed checker does not block others).

## 3. Change completion workflow (HANDOFF.md routine — repeat for this change)
- [x] 3.1 Run verification gates: `cargo fmt --check && cargo test && cargo clippy --all-targets --all-features -- -D warnings && openspec validate --changes --strict --no-interactive`.
- [x] 3.2 Confirm `openspec/changes/config-checker-protocol/tasks.md` has every box ticked.
- [x] 3.3 Archive with `openspec archive config-checker-protocol -y` (omit `--skip-specs`: adds `config-checker-protocol` capability).
- [x] 3.4 Stage and commit implementation + archive: `git add -A && git commit -m "Implement config-checker-protocol"`.
- [x] 3.5 Update `HANDOFF.md` (status, next action, module map) and commit: `git add HANDOFF.md && git commit -m "Update HANDOFF after config-checker-protocol"`.
- [x] 3.6 Re-run `openspec validate --changes --strict --no-interactive` to confirm the change list still resolves.
