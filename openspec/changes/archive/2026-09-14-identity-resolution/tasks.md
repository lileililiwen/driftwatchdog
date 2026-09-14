## 1. Prefix + link identity
- [x] 1.1 Add `find_by_hash_prefix_all`, enforce 1-exact-or-error in `show`/`link`; ambiguous/unknown error tests.
- [x] 1.2 Disambiguate digit-only refs (`id:` rule + help text); add misresolve regression tests.
- [x] 1.3 Add migration 4 for `UNIQUE(fingerprint_id, alert_id)` + dedupe + FK diagnostics; duplicate-link test. (XOR CHECK deliberately not applied: `driftwatch link` sets both endpoints, so XOR would reject every real link row; OR semantics preserved + documented in `schema.rs`.)

## 2. Correlation freshness + caps
- [x] 2.1 Always clear-then-write per fingerprint (incl. empty pass list / empty alerts); stale-row tests.
- [x] 2.2 Newest-first candidate cap with true global bound; remove no-op branch; cap test.

## 3. Change completion workflow (HANDOFF.md routine — repeat for this change)
- [x] 3.1 Run verification gates: `cargo fmt --check && cargo test && cargo clippy --all-targets --all-features -- -D warnings && openspec validate --changes --strict --no-interactive`.
- [x] 3.2 Confirm `openspec/changes/identity-resolution/tasks.md` has every box ticked.
- [x] 3.3 Archive with `openspec archive identity-resolution -y` (omit `--skip-specs`: adds `identity-resolution` capability).
- [x] 3.4 Stage and commit implementation + archive: `git add -A && git commit -m "Implement identity-resolution"`.
- [x] 3.5 Update `HANDOFF.md` (status, next action, module map) and commit: `git add HANDOFF.md && git commit -m "Update HANDOFF after identity-resolution"`.
- [x] 3.6 Re-run `openspec validate --changes --strict --no-interactive` to confirm the change list still resolves.
