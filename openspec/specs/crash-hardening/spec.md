# crash-hardening Specification

## Purpose
TBD - created by archiving change crash-hardening. Update Purpose after archive.
## Requirements
### Requirement: Char-boundary-safe truncation
The CLI MUST never panic when truncating untrusted text for display. All display truncation MUST cut at a `char` boundary and append an ellipsis only when truncated.

#### Scenario: CJK diagnostic at the cut point
- **WHEN** a checker diagnostic is 300 CJK characters and the limit is 200 bytes
- **THEN** `driftwatch check` prints a truncated line without panicking and the output is valid UTF-8

#### Scenario: Emoji summary in list/top
- **WHEN** a command summary contains emoji straddling byte 79 (list) / 59 (top)
- **THEN** `driftwatch list` and `driftwatch top` render without panicking

#### Scenario: Property — no panic on arbitrary input
- **WHEN** the truncation helper is fed arbitrary byte-valid strings at every limit 0..300
- **THEN** it never panics and the result `len <= limit + ellipsis_len`

### Requirement: UTF-8-safe bounded capture
Stream capture MUST accumulate bytes and decode once, so multi-byte sequences split across read chunks are preserved.

#### Scenario: Emoji split across chunks
- **WHEN** a child writes a 4-byte emoji split across two 8KiB pipe reads
- **THEN** the persisted excerpt contains the emoji, not `U+FFFD` replacements

### Requirement: Signal-aware exit status
A child killed by a signal (`exit code == None`) MUST NOT be reported as `StartFailed`. It MUST be reported as killed/signalled (`Status::Unknown` for checkers, signalled status for runs).

#### Scenario: SIGKILLed child
- **WHEN** the wrapped command is killed by SIGKILL
- **THEN** `driftwatch run` records a killed status (not start-failure) and exits nonzero, and `driftwatch list` shows it distinctly from spawn errors

#### Scenario: Checker killed by signal
- **WHEN** a checker process is signal-killed
- **THEN** `driftwatch check` reports `Status::Unknown` with a signal diagnostic, and remaining checkers still run

### Requirement: Bounded execution with group kill
`driftwatch run` MUST support a timeout and MUST kill the whole child process group on timeout; checker timeouts MUST also kill the group, not just the direct child.

#### Scenario: Hung child with timeout
- **WHEN** `driftwatch run --timeout-ms 500 sh -c 'sleep 30'` executes
- **THEN** the command returns within a bounded time with a timeout diagnostic and no orphaned sleep remains

#### Scenario: Grandchild holding the pipe
- **WHEN** a checker spawns a background grandchild inheriting stdout
- **THEN** timeout kills the group and `check` records a timeout (not silent partial output)

### Requirement: No silent I/O capture loss
Write/read failures and capture-thread panics MUST surface as explicit diagnostics, never as silent empty output.

#### Scenario: Sink write failure
- **WHEN** the mirror sink fails
- **THEN** the run outcome carries a diagnostic instead of an empty excerpt

