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

use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::Instant;

use crate::error::Error;
use crate::repo::runs::RunStatus;

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
}

/// Output captured from one of the child's streams. The text is decoded as
/// UTF-8 (lossy) and the `truncated` flag indicates whether the configured
/// limit was hit.
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
            });
        }
        Err(e) => return Err(Error::io(&spec.program, e)),
    };
    let stdout_handle = child.stdout.take();
    let stderr_handle = child.stderr.take();

    let (out_tx, out_rx) = mpsc::channel();
    let (err_tx, err_rx) = mpsc::channel();

    if let Some(stdout) = stdout_handle {
        let limit = spec.capture_limits.stdout_bytes;
        let tx = out_tx;
        thread::spawn(move || {
            let captured = drain_stream(stdout, &mut io::stdout(), limit);
            let _ = tx.send(captured);
        });
    } else {
        let _ = out_tx.send(CapturedStream::default());
    }

    if let Some(stderr) = stderr_handle {
        let limit = spec.capture_limits.stderr_bytes;
        let tx = err_tx;
        thread::spawn(move || {
            let captured = drain_stream(stderr, &mut io::stderr(), limit);
            let _ = tx.send(captured);
        });
    } else {
        let _ = err_tx.send(CapturedStream::default());
    }

    let exit_status = child.wait().map_err(|e| Error::io(&spec.program, e))?;
    let duration_ms = started.elapsed().as_millis() as i64;

    // The threads exit naturally when the pipes close. If they panic, the
    // channel will be dropped and the recv() will return an error; we treat
    // that as an empty capture rather than failing the whole run.
    let stdout = out_rx.recv().unwrap_or_default();
    let stderr = err_rx.recv().unwrap_or_default();

    let exit_code = exit_status.code();
    let status = match exit_code {
        Some(0) => RunStatus::Success,
        Some(_) => RunStatus::Failed,
        None => RunStatus::StartFailed,
    };

    Ok(RunOutcome {
        exit_code,
        status,
        stdout,
        stderr,
        duration_ms,
    })
}

/// Read the entire pipe from `reader`, mirror each chunk to `sink` (typically
/// the parent's stdout/stderr), and append to a bounded `CapturedStream`.
///
/// `limit` semantics:
/// - `None`: capture all bytes.
/// - `Some(0)`: capture nothing; set `truncated = true` (we still read to
///   keep the child from blocking).
/// - `Some(n)`: capture up to `n` bytes; mark truncated if the child wrote
///   more.
fn drain_stream<R: Read, W: Write>(
    mut reader: R,
    sink: &mut W,
    limit: Option<u64>,
) -> CapturedStream {
    let mut text = String::new();
    let mut truncated = false;
    let mut cap_remaining: u64 = match limit {
        Some(0) => 0,
        Some(n) => n,
        None => u64::MAX,
    };
    let mut buf = [0u8; 8192];
    loop {
        match reader.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                let chunk = &buf[..n];
                let _ = sink.write_all(chunk);
                let _ = sink.flush();
                if !truncated && cap_remaining > 0 {
                    let take = (n as u64).min(cap_remaining);
                    // Lossy UTF-8 decode per chunk; safe for downstream
                    // `String` operations.
                    text.push_str(&String::from_utf8_lossy(&chunk[..take as usize]));
                    cap_remaining -= take;
                    if take < n as u64 {
                        truncated = true;
                    }
                } else {
                    // Either we already know we are past the limit, or the
                    // limit is zero: do not append, but mark truncated if any
                    // bytes have been seen.
                    if n > 0 {
                        truncated = true;
                    }
                }
            }
            Err(_) => break,
        }
    }
    CapturedStream { text, truncated }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn spec(program: &str, args: &[&str], cwd: PathBuf) -> CommandSpec {
        CommandSpec {
            program: program.into(),
            args: args.iter().map(|s| s.to_string()).collect(),
            cwd,
            tags: vec![],
            capture_limits: CaptureLimits::default(),
        }
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
