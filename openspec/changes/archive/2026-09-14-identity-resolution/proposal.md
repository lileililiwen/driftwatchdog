# Proposal: Identity resolution (hash prefixes, links, stale correlations)

## Why
Audit found identity bugs: `find_by_hash_prefix` (`src/repo/bugs.rs:184-187`) returns `rows[0]` for `len>=8` even when ambiguous (silent wrong-bug); `link` (`src/commands/link.rs:50-58`) parses all-digit prefixes as numeric `fingerprints.id` first (misresolve); no duplicate-link guard and no `UNIQUE` in `schema.rs:81-88` whose `CHECK` is `OR` not `XOR` despite the doc in `links.rs:89-91`; `Links::create` (`src/repo/links.rs:95-127`) has no existence check beyond FK; `correlate.rs:27-33,50-59` empty-input early return plus survivors-only `replace_for_fingerprint` never clears stale rows; `candidates.rs:42-63` `MAX_PAIRS` truncates oldest-`id` alerts (newest drift dropped) with a no-op second cap branch.

## What Changes
- Unambiguous-or-error prefix resolution (error lists candidates, suggests longer prefix); numeric-id vs hash-prefix disambiguation (explicit `id:`/`#` handling, digit-only strings prefer hash unless `id:` prefixed or exact id match policy documented + tested).
- `UNIQUE(fingerprint_id, alert_id)` + `XOR` check constraint (migration), duplicate-link idempotent error, FK existence diagnostics.
- Correlation clearing: per-fingerprint replace deletes stale rows even when below threshold / on empty alerts; candidate cap keeps newest alerts and enforces a true global cap.

## Capabilities
### New Capabilities
- `identity-resolution`: unambiguous bug identity, safe manual links, fresh correlations.
### Modified Capabilities
- `fingerprinting-and-retention`: `show`/`top` identity semantics.
- `correlation-and-ai-context`: correlation freshness + candidate policy.

## Impact
Affects: `src/repo/bugs.rs`, `src/commands/show.rs`, `src/commands/link.rs`, `src/repo/links.rs`, `src/storage/schema.rs`, `src/storage/migrations.rs`, `src/correlate.rs`, `src/repo/correlations.rs`, `src/similarity/candidates.rs`, `tests/show.rs`, `tests/correlation.rs`.
