## Why

Driftwatch needs a stable local project boundary before it can record runs or alerts. The foundation must make initialization repeatable and keep implementation language separate from the languages of monitored projects.

## What Changes

Create the Rust single-binary CLI foundation, project configuration, `.driftwatch/` state directory, SQLite schema, migration mechanism, and Git metadata abstraction.

## Capabilities

### New Capabilities
- `project-foundation`: initialize and manage the local Driftwatch project state and persistence foundation.

### Modified Capabilities
- None.

## Impact

Adds CLI parsing, TOML configuration, SQLite persistence/migrations, and read-only Git inspection. No network service or external checker is required.
