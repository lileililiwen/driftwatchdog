## 1. Truncation safety
- [x] 1.1 Add shared `truncate_char_boundary` helper + unit tests (ASCII, CJK, emoji at cut point, limit 0).
- [x] 1.2 Replace `&s[..200]` in `src/commands/check.rs:296-303`, `&joined[..79]` in `src/commands/list.rs:90`, `&summary[..59]` in `src/commands/top.rs:33-34`; grep for remaining `&s[..` sites.
- [x] 1.3 Add integration tests driving `check`/`list`/`top` with CJK/emoji output (no panic, valid UTF-8).

## 2. UTF-8-safe capture
- [x] 2.1 Change `src/runtime/runner.rs:179` + `src/checker/runner.rs:194` to accumulate `Vec<u8>` and decode once.
- [x] 2.2 Add chunk-split test (emoji straddling 8KiB boundary).

## 3. Signal status
- [x] 3.1 Map `code()==None` to killed/signalled in runtime + checker runner; construct `Status::Unknown` in `src/commands/check.rs`.
- [x] 3.2 Add tests: SIGKILLed child shows killed status, distinct from spawn failure.

## 4. Timeout + group kill
- [x] 4.1 Add `timeout_ms` to run path (CLI flag), implement group kill (unix `setsid`/`process_group`, windows fallback).
- [x] 4.2 Checker runner reuses group kill; add hung-child and grandchild-holds-pipe tests.

## 5. I/O error surfacing
- [x] 5.1 Propagate `sink.write_all` / `recv` / `drain_bounded` errors into diagnostics; remove `unwrap_or_default` silent paths.
- [x] 5.2 Add test asserting a failing sink yields a diagnostic, not empty capture.

## 6. Change completion workflow (HANDOFF.md routine — repeat for this change)
- [x] 6.1 Run verification gates: `cargo fmt --check && cargo test && cargo clippy --all-targets --all-features -- -D warnings && openspec validate --changes --strict --no-interactive`.
- [x] 6.2 Confirm `openspec/changes/crash-hardening/tasks.md` has every box ticked.
- [ ] 6.3 Archive with `openspec archive crash-hardening -y` (omit `--skip-specs`: adds `crash-hardening` capability).
- [ ] 6.4 Stage and commit implementation + archive: `git add -A && git commit -m "Implement crash-hardening"`.
- [ ] 6.5 Update `HANDOFF.md` (status, next action, module map) and commit: `git add HANDOFF.md && git commit -m "Update HANDOFF after crash-hardening"`.
- [ ] 6.6 Re-run `openspec validate --changes --strict --no-interactive` to confirm the change list still resolves.
