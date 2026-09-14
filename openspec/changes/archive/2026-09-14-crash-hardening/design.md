## Approach
- Add `src/util/truncate.rs` (or extend an existing util): `truncate_char_boundary(s, limit) -> String` using `char_indices`/`get(..)` + ellipsis; unit-test with CJK/emoji at the cut point; replace all `&s[..N]` sites.
- Capture path: accumulate raw `Vec<u8>` per stream under existing byte caps, then single `String::from_utf8_lossy` at the end; keep a test feeding a 4-byte emoji split across two 8KiB chunks.
- Status model: introduce `RunStatus::Signalled` (or reuse `Unknown`) with `signal: Option<i32>` on Unix; `check` maps `exit_code: None` to `Status::Unknown`, never `StartFailed`.
- Runtime timeout: `CommandSpec::timeout_ms: Option<u64>`; wait loop with polling + group kill (`pre_exec setsid` / `CREATE_NEW_PROCESS_GROUP`, behind `#[cfg(unix)]` with Windows fallback to direct kill); checker reuses the same helper.
- I/O: propagate `sink.write_all` errors into `RunOutcome.diagnostic`; replace `unwrap_or_default` with explicit `Err` outcome.

## Non-goals
- No new output formats; no change to fingerprinting rules or correlation scores.
- No default timeout value change for existing users (opt-in first, default later).

## Open questions
- Exact Unix group-kill crate vs `std::os::unix::process::CommandExt::process_group` — decide at implementation.
