## 1. Normalization and fingerprints

- [ ] 1.1 Define normalization configuration, ordered rules, placeholders, and canonicalization output types.
- [ ] 1.2 Write table-driven tests for ANSI, paths, line/column, UUID, timestamp, PID, port, numbers, durations, temp paths, and unchanged semantic text.
- [ ] 1.3 Implement generic normalization and SHA-256 fingerprint generation; make optional language profiles additive and non-blocking.

## 2. Aggregation and queries

- [ ] 2.1 Add upsert logic for fingerprints and occurrence insertion with bounded excerpts and stable IDs.
- [ ] 2.2 Attach failed runtime records to fingerprint aggregation and update first/last/count fields transactionally.
- [ ] 2.3 Implement `show` and Markdown report rendering with recent commits, evidence, and trend data.

## 3. Retention

- [ ] 3.1 Implement `gc --days` with an explicit cutoff and a dry-run-friendly deletion summary.
- [ ] 3.2 Test that old streams are removed while bug identity, occurrence metadata, and statistics remain.
- [ ] 3.3 Verify repeated GC is idempotent and never deletes current records.
