# Tasks: Machine-readable output for driftwatch check

## 1. BFS — Baseline and impact coverage

- [x] Inventory checker result states, alert wire shapes and the existing
  truncation rules shared with storage.
- [x] Map CLI dispatch, human renderer and any callers of the report path.
- [x] Add fixture JSON documents (ok, alerting, protocol-error, timeout,
  mixed) before implementing.
- [x] Confirm `--dry-run` persistence assertions exist for reuse.

## 2. DFS — Requirement-by-requirement implementation

- [x] Add `--format human|json` to `CheckArgs` with human default.
- [x] Implement the checker-report renderer with contract/tool/version,
  declaration order, bounded messages and empty-alerts presence.
- [x] Route all non-document output to stderr in JSON mode.
- [x] Keep execution/isolation/exit/semantics byte-for-byte equal to human
  mode (shared code path, no duplicated logic).

## 3. BFS — Cross-surface regression and completeness

- [x] Re-verify human output unchanged (snapshot), dry-run persists
  nothing in both formats, exit codes equal across formats.
- [x] Re-verify malformed-checker isolation shows as protocol-error rows
  in the document while other checkers still report.
- [x] Update README (External checkers + examples) and `check --help`
  after-help; keep completions/man output consistent.

## 4. Verification

- [x] `cargo fmt --check && cargo test && cargo clippy --all-targets
  --all-features -- -D warnings`.
- [x] `openspec validate --changes --strict --no-interactive`.
- [x] Record any environment-blocked check with its exact next action.

  No environment-blocked checks. Local toolchain passes fmt, the
  full test suite (445 lib + 32 check integration + 8 new
  `src/checker/json` unit + 8 new `tests/check` integration),
  clippy with `-D warnings`, and the strict OpenSpec validation
  for the three currently-proposed changes. No network access, no
  tool install, no Gate persistence, no change to the existing
  checker protocol or `check` exit-status semantics.
