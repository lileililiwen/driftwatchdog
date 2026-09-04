## 1. CLI and configuration foundation

- [x] 1.1 Create the Rust binary crate and command dispatcher with an `init` subcommand and structured application errors.
- [x] 1.2 Define `driftwatch.toml` loading with defaults for project, storage, fingerprint, and checker sections, preserving absent optional sections.
- [x] 1.3 Implement idempotent project-root discovery and initialization of `.driftwatch/` plus a non-overwriting default config.

## 2. Storage and Git contracts

- [x] 2.1 Add SQLite connection setup with foreign keys enabled and versioned migrations for all foundation entities.
- [x] 2.2 Add repository interfaces and basic CRUD primitives for future run, bug, alert, correlation, and link records.
- [x] 2.3 Implement best-effort Git metadata collection for commit, branch, and dirty status.

## 3. Verification

- [x] 3.1 Test initialization in a temporary directory, including idempotent re-run and preservation of a user-edited config.
- [x] 3.2 Test schema creation, migration version, foreign keys, and non-Git metadata behavior.
- [x] 3.3 Verify `driftwatch init --help` and a clean build on supported host platforms.
