## Approach
- Prefix lookup returns all matches; `==1` resolves, `==0` not-found error with `try list/top` hint, `>1` ambiguous error listing up to 5 candidates. Add `find_by_hash_prefix_all` and keep the old single-row fn as a thin wrapper only where safe.
- Link parsing: `bug:<id>` keeps current forms; bare all-digit input requires `id:<n>` for numeric, else treated as hash prefix (documented breaking clarification + migration note in help). Alternatively: try hash-prefix first, fall back to id — decide at implementation and document.
- Migration `0004`: `UNIQUE(fingerprint_id, alert_id)`, fix `CHECK` to XOR (`(a IS NULL) != (b IS NULL)` style per SQLite), backfill dedupe.
- Correlate: always call `replace_for_fingerprint` (even with empty pass list) so stale rows clear; empty-alerts run clears all heuristic rows but preserves manual links.
- Candidates: order alerts newest-first before cap; enforce single global `MAX_PAIRS` with per-bug fairness pass; remove dead second-cap branch.

## Non-goals
- No change to similarity scoring formula (covered by `fingerprint-similarity`).
