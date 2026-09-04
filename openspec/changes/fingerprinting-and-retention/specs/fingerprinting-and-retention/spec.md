## ADDED Requirements

### Requirement: Normalize arbitrary failure text
The normalizer MUST remove or replace configured unstable tokens including ANSI color, absolute paths, line/column, UUIDs, timestamps, PIDs, ports, random numbers, durations, and temporary directories, while preserving meaningful text and relative paths.

#### Scenario: Moving timeout failure
- **WHEN** two failures differ only by source line and timeout duration
- **THEN** they produce the same canonical error and fingerprint

#### Scenario: Language-specific stack formats
- **WHEN** input is a Rust, .NET, Python, Node, Go, or unknown-toolchain error
- **THEN** the generic normalizer accepts it without language-specific assumptions and returns deterministic output

### Requirement: Aggregate bug occurrences
Each normalized failure MUST map to a fingerprint with hash, canonical message, summary, first seen, last seen, and occurrence count; each occurrence MUST retain run, timestamp, commit, and a bounded raw excerpt.

#### Scenario: Repeated logical bug
- **WHEN** the same canonical failure is recorded by three runs
- **THEN** one fingerprint has occurrence count 3 and each run remains individually queryable

### Requirement: Report bug history
`driftwatch show <bug-id>` and `driftwatch report` MUST present occurrence counts, first/last seen, recent commits, recent failures, and a bug trend section in Markdown-compatible output.

#### Scenario: Report recurring bug
- **WHEN** a user requests a report containing a recurring fingerprint
- **THEN** the report includes its stable ID, summary, count, dates, recent commits, and evidence excerpt

### Requirement: Garbage-collect bulky logs
`driftwatch gc [--days N]` MUST remove full stdout/stderr older than the cutoff while retaining fingerprint, occurrence metadata, timestamps, commits, summaries, and aggregate statistics.

#### Scenario: Retain historical memory
- **WHEN** GC runs with a 90-day cutoff
- **THEN** old streams are deleted or marked unavailable, but the historical bug and its occurrence statistics remain queryable
