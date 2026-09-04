## 1. Export

- [ ] 1.1 Define a versioned export DTO layer separate from SQLite row types.
- [ ] 1.2 Write tests for JSON document shape, JSONL one-record-per-line validity, stable IDs, timestamps, and empty datasets.
- [ ] 1.3 Implement JSON, JSONL, and Markdown serializers with stdout output and explicit serialization errors.

## 2. Doctor

- [ ] 2.1 Define diagnostic checks and pass/warn/fail result format without exposing secrets.
- [ ] 2.2 Implement SQLite/schema, config, Git, checker discovery/version, and directory permission checks.
- [ ] 2.3 Implement `doctor` exit semantics and actionable remediation text.

## 3. Verification

- [ ] 3.1 Run export round-trip fixture tests and compare output against documented schema examples.
- [ ] 3.2 Test doctor with healthy, non-Git, invalid-config, unwritable, and missing-checker fixtures.
- [ ] 3.3 Verify commands remain offline and do not mutate project files beyond explicit output redirection.
