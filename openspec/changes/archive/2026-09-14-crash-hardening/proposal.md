# Proposal: Crash hardening (panics, UTF-8, signals, hangs)

## Why
Audit found deterministic crashes and hangs: byte-slice truncation panics on non-char-boundary checker/diagnostic output (`src/commands/check.rs:296-303`, `src/commands/list.rs:90`, `src/commands/top.rs:33-34`), per-chunk `String::from_utf8_lossy` splitting multi-byte UTF-8 (`src/runtime/runner.rs:179`, `src/checker/runner.rs:194`), signal-killed children misclassified as `StartFailed` (`src/runtime/runner.rs:130-135`, `src/checker/runner.rs:166`, `Status::Unknown` in `src/checker/report.rs:29` never constructed), unbounded `child.wait()` with no runtime timeout, `child.kill()` killing only the direct child (grandchildren leak, 500ms `recv_timeout` returns partial output), and silently swallowed I/O/thread failures (`let _ = sink.write_all`, `Err(_)=>break`, `recv().unwrap_or_default()`, `drain_bounded` swallowing).

## What Changes
- Char-boundary-safe truncation helper shared by `check`/`list`/`top` (and any future `&s[..N]` site); no `&s[..N]` on untrusted text.
- UTF-8-safe bounded capture: decode once from the byte buffer (or incremental decoder holding overhang), never per-chunk lossy on arbitrary split points.
- Signal exit-code model: `code()==None` maps to killed-by-signal (distinct from start-failure), surfaced in `list`/`show`/`check` outcomes; construct `Status::Unknown` where appropriate.
- Runtime timeout + process-group kill for `driftwatch run` (opt-in `--timeout-ms`, default preserves current behavior except hung-pipe safety); checker keeps per-checker timeout and kills the group.
- I/O errors surfaced as diagnostics (not silent empty capture); thread-panic yields an explicit error, not `unwrap_or_default`.

## Capabilities
### New Capabilities
- `crash-hardening`: no-panic truncation, UTF-8-safe capture, signal-aware status, bounded run/checker execution.
### Modified Capabilities
- `runtime-memory`: `run` completion/status semantics.
- `checker-and-drift-alerts`: `check` outcome/diagnostic semantics.

## Impact
Affects: `src/commands/check.rs`, `src/commands/list.rs`, `src/commands/top.rs`, `src/runtime/runner.rs`, `src/checker/runner.rs`, `src/checker/report.rs`, `src/commands/run.rs`, `tests/run.rs`, `tests/check.rs`.
