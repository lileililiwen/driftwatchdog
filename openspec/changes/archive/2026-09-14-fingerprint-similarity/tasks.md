## 1. Normalizer fixes
- [x] 1.1 Fix duration/port-vs-linecol/temp-path/whitespace rules with fixture tests (each audit example becomes a test).
- [x] 1.2 Fallible `compile`, pattern-keyed cache, remove `Box::leak`; invalid-pattern test.
- [x] 1.3 Update golden fingerprints; add Python/YAML indentation fixture.

## 2. Similarity recalibration
- [x] 2.1 Placeholder-aware + leaf `score_file`; multi-token IDF `score_symbol`; short-token allowlist + expanded stopwords.
- [x] 2.2 Implement or remove `score_tag` weight; re-tune threshold on labeled fixtures; document weights.
- [x] 2.3 Add true-positive (abs path) + false-positive (generic `error`) regression tests.

## 3. Change completion workflow (HANDOFF.md routine — repeat for this change)
- [x] 3.1 Run verification gates: `cargo fmt --check && cargo test && cargo clippy --all-targets --all-features -- -D warnings && openspec validate --changes --strict --no-interactive`.
- [x] 3.2 Confirm `openspec/changes/fingerprint-similarity/tasks.md` has every box ticked.
- [x] 3.3 Archive with `openspec archive fingerprint-similarity -y` (omit `--skip-specs`: adds `fingerprint-similarity` capability).
- [x] 3.4 Stage and commit implementation + archive: `git add -A && git commit -m "Implement fingerprint-similarity"`.
- [x] 3.5 Update `HANDOFF.md` (status, next action, module map) and commit: `git add HANDOFF.md && git commit -m "Update HANDOFF after fingerprint-similarity"`.
- [x] 3.6 Re-run `openspec validate --changes --strict --no-interactive` to confirm the change list still resolves.
