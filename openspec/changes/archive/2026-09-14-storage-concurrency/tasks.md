## 1. Migrations
- [x] 1.1 Make migration runner transactional + rerunnable (`PRAGMA table_info` guard); add concurrent-open + crash-rerun tests.
- [x] 1.2 Cover `MIGRATION_0002/0003` rerun in `tests/storage.rs`.

## 2. Root + git
- [x] 2.1 Canonicalize root, remove check-then-use, explicit error past max depth.
- [x] 2.2 Bound git capture (~2s), `dirty=None` on missing/timeout; add tests.

## 3. Doctor
- [x] 3.1 Read-only DB open for checks; atomic `create_new` probe with `Drop` cleanup.
- [x] 3.2 Bounded argv-based `--version` probes; add hang + mutation tests.

## 4. Tag/occurrence/trend/export
- [x] 4.1 Escape LIKE wildcards (or exact-match join); add `auth` vs `oauth` + `%_` tests.
- [x] 4.2 Fix `upsert_for_occurrence` first/last-seen; add backdated test.
- [x] 4.3 Single-query timezone-correct trends, propagate errors (remove `unwrap_or(0)`/`continue` swallowing); export cap warning + test.

## 5. Change completion workflow (HANDOFF.md routine — repeat for this change)
- [x] 5.1 Run verification gates: `cargo fmt --check && cargo test && cargo clippy --all-targets --all-features -- -D warnings && openspec validate --changes --strict --no-interactive`.
- [x] 5.2 Confirm `openspec/changes/storage-concurrency/tasks.md` has every box ticked.
- [x] 5.3 Archive with `openspec archive storage-concurrency -y` (omit `--skip-specs`: adds `storage-concurrency` capability).
- [x] 5.4 Stage and commit implementation + archive: `git add -A && git commit -m "Implement storage-concurrency"`.
- [x] 5.5 Update `HANDOFF.md` (status, next action, module map) and commit: `git add HANDOFF.md && git commit -m "Update HANDOFF after storage-concurrency"`.
- [x] 5.6 Re-run `openspec validate --changes --strict --no-interactive` to confirm the change list still resolves.
