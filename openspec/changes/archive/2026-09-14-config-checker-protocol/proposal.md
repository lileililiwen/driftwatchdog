# Proposal: Config validation and checker protocol strictness

## Why
Audit found lenient config + brittle protocol: `src/project/config.rs:57-77` accepts unknown fields, duplicate checker names, empty commands, and `timeout_ms==0` (immediate timeout); `working_dir` is copied verbatim and resolved against process cwd, allowing `/etc` / `../../` escapes; checker failures are isolated per run but `doctor check_one_checker` splits on whitespace; `protocol.rs:77-83` rejects documents with unknown top-level fields (breaks forward-compat despite `protocol.rs:1-10` doc) while missing `alerts` defaults to `[]` so `{}` parses as success/Empty; `Status::Unknown` is never constructed in `src/commands/check.rs:119-290` (covered in crash-hardening for the kill path — here for dry-run/parse-failure mapping).

## What Changes
- Strict config: `deny_unknown_fields`, duplicate-name / empty-command / `timeout_ms==0` rejection with actionable errors, `working_dir` jailed to the project root (canonicalize + must-start-with check).
- Forward-compatible protocol: ignore unknown top-level fields, require explicit `alerts` (missing = protocol error, not success), cap alert count/size with a clear error.
- `check --dry-run` and parse-failure paths map to explicit statuses; every outcome carries a diagnostic.

## Capabilities
### New Capabilities
- `config-checker-protocol`: validated checker config + forward-compatible alerts protocol.
### Modified Capabilities
- `checker-and-drift-alerts`: config/protocol/check semantics.

## Impact
Affects: `src/project/config.rs`, `src/checker/protocol.rs`, `src/checker/runner.rs`, `src/commands/check.rs`, `src/doctor/mod.rs`, `driftwatch.toml`, `driftwatch.toml.example` (new), `tests/check.rs`, `README.md` checker section.
