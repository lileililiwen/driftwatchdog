//! Cross-platform child-process runner.
//!
//! Spawns a child with `program + argv` (no shell), streams its stdout and
//! stderr to the parent's terminal, and captures bounded excerpts that the
//! `commands::run` orchestration persists to the database. A missing
//! executable is reported as `StartFailed` rather than a panic; any other
//! spawn error is surfaced as `Error::Io`.
//!
//! The capture limit is `Some(0)` to disable capture, `Some(n)` for `n` bytes,
//! and `None` for unlimited. The drain thread keeps reading past the limit so
//! the child does not block on a full pipe.
//!
//! Crash-hardening guarantees:
//! - Stream bytes are accumulated as `Vec<u8>` and decoded once, so a
//!   multi-byte UTF-8 sequence split across pipe reads is preserved.
//! - `exit code == None` after a successful spawn means killed-by-signal
//!   (`RunStatus::Signalled`), never `StartFailed`.
//! - An opt-in `CommandSpec::timeout_ms` bounds execution; on timeout the
//!   whole child process group is killed (Unix) so grandchildren holding
//!   the pipe cannot wedge the run.
//! - Sink write failures, read errors, and capture-thread panics surface as
//!   `RunOutcome::diagnostic` instead of silent empty capture.

use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use crate::error::Error;
use crate::repo::runs::RunStatus;

#[cfg(unix)]
use std::os::unix::process::CommandExt;

/// Bounded capture limits for a child process's streams. `None` means
/// unlimited; `Some(0)` disables capture; `Some(n)` captures up to `n` bytes.
#[derive(Debug, Clone, Copy, Default)]
pub struct CaptureLimits {
    pub stdout_bytes: Option<u64>,
    pub stderr_bytes: Option<u64>,
}

/// Description of a child process to run.
#[derive(Debug, Clone)]
pub struct CommandSpec {
    pub program: String,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    #[allow(dead_code)]
    pub tags: Vec<String>,
    pub capture_limits: CaptureLimits,
    /// Opt-in wall-clock timeout. `None` preserves the historical unbounded
    /// wait (drain collection is still bounded so a grandchild holding the
    /// pipe cannot hang the parent forever).
    pub timeout_ms: Option<u64>,
}

/// Output captured from one of the child's streams. The text is decoded as
/// UTF-8 (lossy, once from the accumulated bytes) and the `truncated` flag
/// indicates whether the configured limit was hit.
#[derive(Debug, Clone, Default)]
pub struct CapturedStream {
    pub text: String,
    pub truncated: bool,
}

/// Result of running a child process. Even when the child could not be
/// started, this is returned (with `status = StartFailed` and `exit_code =
/// None`) so the caller can persist a uniform row.
#[derive(Debug, Clone)]
pub struct RunOutcome {
    pub exit_code: Option<i32>,
    pub status: RunStatus,
    pub stdout: CapturedStream,
    pub stderr: CapturedStream,
    pub duration_ms: i64,
    /// True when `timeout_ms` fired and the child group was killed.
    pub timed_out: bool,
    /// Capture/thread diagnostics (sink failures, read errors, panics,
    /// drain timeouts). `None` on the clean path.
    pub diagnostic: Option<String>,
}

/// Result of draining one pipe: the bounded capture plus an optional
/// diagnostic describing a sink write failure or read error.
#[derive(Debug, Default)]
struct DrainResult {
    captured: CapturedStream,
    diagnostic: Option<String>,
}

/// Run a child process. Streams its output to the parent's stdout/stderr and
/// captures bounded excerpts as configured. On a spawn failure, returns a
/// `RunOutcome` with `StartFailed` and a diagnostic in `stderr.text`.
pub fn run(spec: &CommandSpec) -> Result<RunOutcome, Error> {
    let started = Instant::now();
    let mut cmd = Command::new(&spec.program);
    cmd.args(&spec.args)
        .current_dir(&spec.cwd)
        .stdin(Stdio::inherit())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(unix)]
    {
        // New process group so a timeout can kill grandchildren too.
        cmd.process_group(0);
    }

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            // POSIX "command not found" semantics: surface a StartFailed
            // outcome so the database gets a uniform row.
            return Ok(RunOutcome {
                exit_code: None,
                status: RunStatus::StartFailed,
                stdout: CapturedStream::default(),
                stderr: CapturedStream {
                    text: e.to_string(),
                    truncated: false,
                },
                duration_ms: started.elapsed().as_millis() as i64,
                timed_out: false,
                diagnostic: None,
            });
        }
        Err(e) => return Err(Error::io(&spec.program, e)),
    };
    let child_id = child.id();
    let stdout_handle = child.stdout.take();
    let stderr_handle = child.stderr.take();

    let (out_tx, out_rx) = mpsc::channel();
    let (err_tx, err_rx) = mpsc::channel();

    if let Some(stdout) = stdout_handle {
        let limit = spec.capture_limits.stdout_bytes;
        let tx = out_tx;
        thread::spawn(move || {
            let drained = drain_stream(stdout, &mut io::stdout(), limit);
            let _ = tx.send(drained);
        });
    } else {
        let _ = out_tx.send(DrainResult::default());
    }

    if let Some(stderr) = stderr_handle {
        let limit = spec.capture_limits.stderr_bytes;
        let tx = err_tx;
        thread::spawn(move || {
            let drained = drain_stream(stderr, &mut io::stderr(), limit);
            let _ = tx.send(drained);
        });
    } else {
        let _ = err_tx.send(DrainResult::default());
    }

    // Wait for the child: bounded polling when a timeout is configured,
    // blocking wait otherwise.
    let mut timed_out = false;
    if let Some(ms) = spec.timeout_ms {
        let deadline = started + Duration::from_millis(ms);
        let poll = Duration::from_millis(10);
        loop {
            match child.try_wait().map_err(|e| Error::io(&spec.program, e))? {
                Some(_status) => break,
                None => {
                    if Instant::now() >= deadline {
                        timed_out = true;
                        kill_child_group(&mut child, child_id);
                        // Reap so we do not leave a zombie.
                        let _ = child.wait();
                        break;
                    }
                    thread::sleep(poll);
                }
            }
        }
    } else {
        child.wait().map_err(|e| Error::io(&spec.program, e))?;
    }
    // Re-query the exit status after the wait above. `try_wait` returns the
    // status when the child has exited; a timeout kill leaves no status.
    let exit_status = if timed_out {
        None
    } else {
        child.try_wait().map_err(|e| Error::io(&spec.program, e))?
    };
    let duration_ms = started.elapsed().as_millis() as i64;

    // Collect drain results with a bound so a grandchild holding the pipe
    // cannot wedge the parent forever. A missing message means the capture
    // thread panicked: surface it instead of silently returning empty text.
    let drain_timeout = Duration::from_secs(5);
    let mut diagnostics: Vec<String> = Vec::new();
    let stdout_drain = match out_rx.recv_timeout(drain_timeout) {
        Ok(d) => d,
        Err(_) => DrainResult {
            captured: CapturedStream::default(),
            diagnostic: Some("stdout capture thread panicked or hung".into()),
        },
    };
    let stderr_drain = match err_rx.recv_timeout(drain_timeout) {
        Ok(d) => d,
        Err(_) => DrainResult {
            captured: CapturedStream::default(),
            diagnostic: Some("stderr capture thread panicked or hung".into()),
        },
    };
    for d in [&stdout_drain, &stderr_drain] {
        if let Some(diag) = &d.diagnostic {
            diagnostics.push(diag.clone());
        }
    }

    let exit_code = exit_status.and_then(|s| s.code());
    let status = match exit_code {
        Some(0) => RunStatus::Success,
        Some(_) => RunStatus::Failed,
        None if timed_out => RunStatus::Timeout,
        // Spawn succeeded but no code => killed by a signal. This must
        // never be StartFailed (reserved for spawn failures above).
        None => RunStatus::Signalled,
    };

    let mut diagnostic = if diagnostics.is_empty() {
        None
    } else {
        Some(diagnostics.join("; "))
    };
    if timed_out {
        let timeout_msg = format!(
            "command exceeded timeout of {} ms; child process group killed",
            spec.timeout_ms.unwrap_or(0)
        );
        diagnostic = Some(match diagnostic {
            Some(d) => format!("{timeout_msg}; {d}"),
            None => timeout_msg,
        });
    }
    if status == RunStatus::Signalled {
        let sig_msg = "child killed by signal (exit code unavailable)".to_string();
        diagnostic = Some(match diagnostic {
            Some(d) => format!("{sig_msg}; {d}"),
            None => sig_msg,
        });
    }

    Ok(RunOutcome {
        exit_code,
        status,
        stdout: stdout_drain.captured,
        stderr: stderr_drain.captured,
        duration_ms,
        timed_out,
        diagnostic,
    })
}

/// Kill the child and its process group. On Unix the child was spawned with
/// `process_group(0)` so it leads its own group; `kill(-pid)` reaches
/// grandchildren. Falls back to a direct kill when the group kill fails
/// (or on non-Unix platforms).
fn kill_child_group(child: &mut std::process::Child, pid: u32) {
    #[cfg(unix)]
    {
        // Negative pid targets the whole process group.
        let group_killed = unsafe { libc::kill(-(pid as i32), libc::SIGKILL) == 0 };
        if !group_killed {
            let _ = child.kill();
        }
    }
    #[cfg(not(unix))]
    {
        let _ = child.kill();
    }
}

/// Read the entire pipe from `reader`, mirror each chunk to `sink`
/// (typically the parent's stdout/stderr), and append to a bounded
/// `CapturedStream`.
///
/// Bytes are accumulated raw and decoded once at the end so a multi-byte
/// UTF-8 sequence split across 8KiB reads is preserved. Sink write failures
/// and read errors are recorded as a diagnostic instead of being dropped.
///
/// `limit` semantics:
/// - `None`: capture all bytes.
/// - `Some(0)`: capture nothing; set `truncated = true` (we still read to
///   keep the child from blocking).
/// - `Some(n)`: capture up to `n` bytes; mark truncated if the child wrote
///   more.
fn drain_stream<R: Read, W: Write>(mut reader: R, sink: &mut W, limit: Option<u64>) -> DrainResult {
    let cap: u64 = match limit {
        Some(0) => 0,
        Some(n) => n,
        None => u64::MAX,
    };
    let mut stored: Vec<u8> = Vec::new();
    if cap != u64::MAX {
        stored.reserve(cap.min(8192) as usize);
    }
    let mut total_seen: u64 = 0;
    let mut diagnostic: Option<String> = None;
    let mut buf = [0u8; 8192];
    loop {
        match reader.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                let chunk = &buf[..n];
                if let Err(e) = sink.write_all(chunk).and_then(|()| sink.flush()) {
                    if diagnostic.is_none() {
                        diagnostic = Some(format!("mirror sink write failed: {e}"));
                    }
                }
                total_seen += n as u64;
                if (stored.len() as u64) < cap {
                    let remaining = cap - stored.len() as u64;
                    let take = (n as u64).min(remaining) as usize;
                    stored.extend_from_slice(&chunk[..take]);
                }
            }
            Err(e) => {
                diagnostic = Some(match diagnostic {
                    Some(d) => format!("{d}; stream read failed: {e}"),
                    None => format!("stream read failed: {e}"),
                });
                break;
            }
        }
    }
    let truncated = total_seen > cap || (cap == 0 && total_seen > 0);
    let text = String::from_utf8_lossy(&stored).into_owned();
    DrainResult {
        captured: CapturedStream { text, truncated },
        diagnostic,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use tempfile::tempdir;

    fn spec(program: &str, args: &[&str], cwd: PathBuf) -> CommandSpec {
        CommandSpec {
            program: program.into(),
            args: args.iter().map(|s| s.to_string()).collect(),
            cwd,
            tags: vec![],
            capture_limits: CaptureLimits::default(),
            timeout_ms: None,
        }
    }

    /// A reader that yields its bytes in tiny fixed-size chunks, forcing
    /// multi-byte UTF-8 sequences to straddle read boundaries.
    struct ChunkedReader {
        data: Vec<u8>,
        pos: usize,
        chunk: usize,
    }

    impl Read for ChunkedReader {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            if self.pos >= self.data.len() {
                return Ok(0);
            }
            let n = self.chunk.min(self.data.len() - self.pos).min(buf.len());
            buf[..n].copy_from_slice(&self.data[self.pos..self.pos + n]);
            self.pos += n;
            Ok(n)
        }
    }

    struct FailingSink;

    impl Write for FailingSink {
        fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
            Err(io::Error::other("sink boom"))
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn emoji_split_across_chunks_is_preserved() {
        let emoji = "🎉".as_bytes();
        let mut data = vec![b'a'; 8191];
        data.extend_from_slice(emoji);
        data.extend_from_slice(b"tail");
        let reader = ChunkedReader {
            data,
            pos: 0,
            chunk: 1,
        };
        let mut sink = Vec::new();
        let out = drain_stream(reader, &mut sink, None);
        assert!(out.diagnostic.is_none());
        assert!(
            out.captured.text.contains('🎉'),
            "got {:?}",
            out.captured.text
        );
        assert!(!out.captured.text.contains('�'));
    }

    #[test]
    fn failing_sink_surfaces_diagnostic_not_empty_silence() {
        let reader = Cursor::new(b"hello".to_vec());
        let out = drain_stream(reader, &mut FailingSink, None);
        assert!(out.diagnostic.is_some(), "expected a diagnostic");
        // Bytes are still captured even though mirroring failed.
        assert_eq!(out.captured.text, "hello");
    }

    #[cfg(unix)]
    #[test]
    fn argv_passes_through_verbatim() {
        let tmp = tempdir().unwrap();
        let s = spec(
            "echo",
            &["--release", "with spaces"],
            tmp.path().to_path_buf(),
        );
        let out = run(&s).unwrap();
        assert_eq!(out.status, RunStatus::Success);
        assert!(out.stdout.text.contains("--release"));
        assert!(out.stdout.text.contains("with spaces"));
    }

    #[cfg(unix)]
    #[test]
    fn success_exit_code_is_zero() {
        let tmp = tempdir().unwrap();
        let s = spec("sh", &["-c", "exit 0"], tmp.path().to_path_buf());
        let out = run(&s).unwrap();
        assert_eq!(out.status, RunStatus::Success);
        assert_eq!(out.exit_code, Some(0));
    }

    #[cfg(unix)]
    #[test]
    fn nonzero_exit_code_propagates() {
        let tmp = tempdir().unwrap();
        let s = spec("sh", &["-c", "exit 42"], tmp.path().to_path_buf());
        let out = run(&s).unwrap();
        assert_eq!(out.status, RunStatus::Failed);
        assert_eq!(out.exit_code, Some(42));
    }

    #[test]
    fn missing_executable_returns_start_failed() {
        let tmp = tempdir().unwrap();
        let s = spec(
            "definitely-not-a-real-binary-xyz-12345",
            &[],
            tmp.path().to_path_buf(),
        );
        let out = run(&s).unwrap();
        assert_eq!(out.status, RunStatus::StartFailed);
        assert!(out.exit_code.is_none());
        // The diagnostic is the OS error string; we don't pin its exact
        // wording because it differs across platforms ("No such file or
        // directory" vs. "The system cannot find the file specified").
        assert!(!out.stderr.text.is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn signal_killed_child_is_signalled_not_start_failed() {
        let tmp = tempdir().unwrap();
        // Kill ourselves with SIGKILL: no exit code, must not be StartFailed.
        let s = spec("sh", &["-c", "kill -KILL $$"], tmp.path().to_path_buf());
        let out = run(&s).unwrap();
        assert_eq!(out.status, RunStatus::Signalled);
        assert!(out.exit_code.is_none());
        assert!(out.diagnostic.is_some());
    }

    #[cfg(unix)]
    #[test]
    fn timeout_kills_hung_child_with_group() {
        let tmp = tempdir().unwrap();
        let mut s = spec("sh", &["-c", "sleep 30"], tmp.path().to_path_buf());
        s.timeout_ms = Some(300);
        let started = Instant::now();
        let out = run(&s).unwrap();
        assert!(out.timed_out);
        assert_eq!(out.status, RunStatus::Timeout);
        assert!(started.elapsed() < Duration::from_secs(10));
        assert!(out.diagnostic.as_deref().unwrap_or("").contains("timeout"));
    }

    #[cfg(unix)]
    #[test]
    fn bounded_capture_marks_truncation() {
        let tmp = tempdir().unwrap();
        let s = CommandSpec {
            program: "sh".into(),
            args: vec!["-c".into(), "yes A | head -c 200000".into()],
            cwd: tmp.path().to_path_buf(),
            tags: vec![],
            capture_limits: CaptureLimits {
                stdout_bytes: Some(1024),
                stderr_bytes: Some(1024),
            },
            timeout_ms: None,
        };
        let out = run(&s).unwrap();
        assert!(
            out.stdout.truncated,
            "expected truncation, got {} bytes",
            out.stdout.text.len()
        );
        assert!(out.stdout.text.len() <= 1024);
    }

    #[cfg(unix)]
    #[test]
    fn zero_limit_means_no_capture() {
        let tmp = tempdir().unwrap();
        let s = CommandSpec {
            program: "sh".into(),
            args: vec!["-c".into(), "echo hello".into()],
            cwd: tmp.path().to_path_buf(),
            tags: vec![],
            capture_limits: CaptureLimits {
                stdout_bytes: Some(0),
                stderr_bytes: Some(0),
            },
            timeout_ms: None,
        };
        let out = run(&s).unwrap();
        assert!(out.stdout.text.is_empty());
        // The child did emit output, so truncated should be true.
        assert!(out.stdout.truncated);
        assert_eq!(out.status, RunStatus::Success);
    }

    #[cfg(unix)]
    #[test]
    fn cwd_is_honored() {
        let tmp = tempdir().unwrap();
        let s = spec("sh", &["-c", "pwd"], tmp.path().to_path_buf());
        let out = run(&s).unwrap();
        assert_eq!(out.status, RunStatus::Success);
        // macOS resolves /var -> /private/var, so canonicalize both sides.
        let expected = tmp.path().canonicalize().unwrap();
        let actual = std::path::Path::new(out.stdout.text.trim())
            .canonicalize()
            .unwrap();
        assert_eq!(actual, expected);
    }
}
