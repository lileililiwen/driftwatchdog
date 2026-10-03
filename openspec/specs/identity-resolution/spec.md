# identity-resolution Specification

## Purpose

Audit found identity bugs: `find_by_hash_prefix` (`src/repo/bugs.rs:184-187`) returns `rows[0]` for `len>=8` even when ambiguous (silent wrong-bug); `link` (`src/commands/link.rs:50-58`) parses all-digit prefixes as numeric `fingerprints.id` first (misresolve); no duplicate-link guard and no `UNIQUE` in `schema.rs:81-88` whose `CHECK` is `OR` not `XOR` despite the doc in `links.rs:89-91`; `Links::create` (`src/repo/links.rs:95-127`) has no existence check beyond FK; `correlate.rs:27-33,50-59` empty-input early return plus survivors-only `replace_for_fingerprint` never clears stale rows; `candidates.rs:42-63` `MAX_PAIRS` truncates oldest-`id` alerts (newest drift dropped) with a no-op second cap branch.
## Requirements
### Requirement: Unambiguous prefix resolution
Hash-prefix lookup MUST resolve only when exactly one fingerprint matches; otherwise it MUST error explicitly.

#### Scenario: Ambiguous 8-char prefix
- **WHEN** two fingerprints share prefix `abcdef12` and the user runs `driftwatch show abcdef12`
- **THEN** the command fails with an `ambiguous` error listing both candidates and suggesting a longer prefix, and never silently picks `rows[0]`

#### Scenario: Unknown prefix
- **WHEN** no fingerprint matches
- **THEN** the error suggests `driftwatch list`/`top` and the required prefix length

### Requirement: Safe link targeting
Bare all-digit link targets MUST NOT silently resolve to the wrong identity; duplicates MUST be rejected idempotently.

#### Scenario: Digit-only bug ref
- **WHEN** the user runs `driftwatch link 12345 spec:9`
- **THEN** the CLI treats it per the documented rule (hash-prefix unless `id:` prefixed), or errors asking for `bug:<…>` / `id:<…>`, and never links the wrong bug

#### Scenario: Duplicate link
- **WHEN** the same bug+alert pair is linked twice
- **THEN** the second call reports `already linked (id N)` instead of inserting a duplicate row

#### Scenario: XOR link endpoints
- **WHEN** a manual link row is inserted
- **THEN** exactly one endpoint form is set (XOR CHECK enforced by the DB, not just docs)

### Requirement: Fresh correlations
Correlations MUST clear stale rows when pairs stop passing; empty alert sets MUST clear heuristic rows while preserving manual links.

#### Scenario: Score drops below threshold
- **WHEN** a fingerprint previously correlated now scores below threshold
- **THEN** its old correlation rows disappear from `report --ai` after the next `check`

#### Scenario: Newest alert survives cap
- **WHEN** 6000 candidate pairs exist with `MAX_PAIRS=5000`
- **THEN** the newest alerts are retained (not the oldest-`id` slice) and the total never exceeds the cap

