## ADDED Requirements
### Requirement: Precise normalization
Normalization MUST distinguish ports from line:col, durations from bare counts, and MUST preserve indentation; invalid patterns MUST error, not panic.

#### Scenario: DB host and port
- **WHEN** output contains `example.com:5432` and `src/main.rs:10:5`
- **THEN** the first becomes `<host>:<port>`, the second `<path>:<line>:<col>`, and the two never share a fingerprint for that reason alone

#### Scenario: Bare count vs duration
- **WHEN** output contains `5 s` (five seconds plural?) vs `retry 5 s`
- **THEN** only true durations normalize; `5 tests failed` keeps its count distinct

#### Scenario: Quoted tmp path
- **WHEN** output contains `("/tmp/foo-bar")`
- **THEN** it normalizes identically to ` /tmp/foo-bar`

#### Scenario: Python traceback
- **WHEN** two tracebacks differ only in indentation-nested blocks
- **THEN** they produce different fingerprints (indentation preserved)

#### Scenario: Bad regex pattern
- **WHEN** a custom rule has an invalid pattern
- **THEN** compilation returns an error naming the rule instead of panicking

### Requirement: Calibrated similarity
File/symbol/tag scoring MUST handle placeholders, multi-token symbols, and short domain tokens; the weights/threshold MUST be consistent (no dead weight).

#### Scenario: Absolute-path bug vs alert
- **WHEN** a bug mentions `/home/u/proj/src/db.rs` and an alert references `src/db.rs` / symbol `DbPool`
- **THEN** `score_file` gives partial/full credit (not 0) and the pair can pass threshold

#### Scenario: Generic error symbol
- **WHEN** bug text starts with `error` and alert symbol is `error`
- **THEN** the pair does NOT pass on symbol alone

#### Scenario: Short tokens
- **WHEN** texts share `s3` / `db` / `io`
- **THEN** those tokens contribute (not dropped), while `error/failed` are down-weighted

#### Scenario: Tag weight honesty
- **WHEN** scoring runs
- **THEN** either tags contribute (implemented `score_tag`) or the weight is 0 and documented — never a dead 0.1 weight with threshold tuned for 1.0
