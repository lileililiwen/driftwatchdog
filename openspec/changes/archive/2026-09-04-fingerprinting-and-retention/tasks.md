## 1. Normalization and fingerprints

- [x] 1.1 Define normalization configuration, ordered rules, placeholders, and canonicalization output types.
- [x] 1.2 Write table-driven tests for ANSI, paths, line/column, UUID, timestamp, PID, port, numbers, durations, temp paths, and unchanged semantic text.
- [x] 1.3 Implement generic normalization and SHA-256 fingerprint generation; make optional language profiles additive and non-blocking.

## 2. Aggregation and queries

- [x] 2.1 Add upsert logic for fingerprints and occurrence insertion with bounded excerpts and stable IDs.
- [x] 2.2 Attach failed runtime records to fingerprint aggregation and update first/last/count fields transactionally.
- [x] 2.3 Implement `show` and Markdown report rendering with recent commits, evidence, and trend data.

## 3. Retention

- [x] 3.1 Implement `gc --days` with an explicit cutoff and a dry-run-friendly deletion summary.
- [x] 3.2 Test that old streams are removed while bug identity, occurrence metadata, and statistics remain.
- [x] 3.3 Verify repeated GC is idempotent and never deletes current records.
