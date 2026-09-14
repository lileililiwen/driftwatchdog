//! Cross-platform checker runner.
//!
//! A checker is a user-configured child process. This module launches it,
//! captures bounded output, and applies a per-checker timeout. It does
//! **not** propagate I/O errors as `Result::Err`: a missing executable,
//! nonzero exit, timeout, or other failure all map to a [`CheckerRun`]
//! with the appropriate flags, so the caller can persist a failed
//! snapshot and continue to the next checker.
//!
//! Crash-hardening: bytes accumulate raw and decode once (UTF-8 split
//! safety), signal kills map to `signalled` (never `spawn_error`), timeouts
//! kill the whole process group, and sink/read/thread failures surface as
//! `capture_error` instead of silent empty output.

use std::collections::BTreeMap;
use std::io::Read;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

#[cfg(unix)]
use std::os::unix::process::CommandExt;

/// Default per-checker timeout when the user does not specify one.
pub const DEFAULT_TIMEOUT_MS: u64 = 30_000;
/// Default per-stream output limit when the user does not specify one.
pub const DEFAULT_MAX_OUTPUT_BYTES: u64 = 1024 * 1024;

/// Description of a checker to run.
#[derive(Debug, Clone)]
pub struct CheckerSpec {
    pub name: String,
    pub program: String,
    pub args: Vec<String>,
    pub working_dir: PathBuf,
    pub env: BTreeMap<String, String>,
    pub timeout: Duration,
    pub max_output_bytes: u64,
}

impl CheckerSpec {
    /// Build a [`CheckerSpec`] from a config entry + project root. Falls
    /// back to the documented defaults when the optional fields are absent.
    pub fn from_entry(
        entry: &crate::project::config::CheckerEntry,
        project_root: &std::path::Path,
    ) -> Self {
        let working_dir = entry
            .working_dir
            .clone()
            .unwrap_or_else(|| project_root.to_path_buf());
        let timeout_ms = entry.timeout_ms.unwrap_or(DEFAULT_TIMEOUT_MS);
        let max_output_bytes = entry.max_output_bytes.unwrap_or(DEFAULT_MAX_OUTPUT_BYTES);
        Self {
            name: entry.name.clone(),
            program: entry.command.clone(),
            args: entry.args.clone(),
            working_dir,
            env: entry.env.clone(),
            timeout: Duration::from_millis(timeout_ms),
            max_output_bytes,
        }
    }
}

/// Result of running a checker. Captured output is bounded; the `truncated`
/// flags indicate whether the configured limit was reached.
#[derive(Debug, Clone, Default)]
pub struct CheckerRun {
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub stdout_truncated: bool,
    pub stderr_truncated: bool,
    pub timed_out: bool,
    /// True when the child exited without a code and was not a timeout:
    /// killed by a signal. The caller maps this to `Status::Unknown`.
    pub signalled: bool,
    /// Set when the child could not be spawned at all (e.g. missing
    /// executable). When set, the snapshot's status should be
    /// `start_failed` and the message is the OS error.
    pub spawn_error: Option<String>,
    /// Capture-pipeline diagnostics (sink failures, read errors, panics,
    /// drain timeouts). Never silently dropped.
    pub capture_error: Option<String>,
    pub duration_ms: i64,
}

/// Run a checker. Never returns an `Err` for the failure modes the spec
/// calls out: missing executable, nonzero exit, timeout, or malformed
/// output are all encoded in the returned [`CheckerRun`].
pub fn run_checker(spec: &CheckerSpec) -> CheckerRun {
    let started = Instant::now();
    let mut cmd = Command::new(&spec.program);
    cmd.args(&spec.args)
        .current_dir(&spec.working_dir)
        // Re-inherit the parent's environment, then layer the checker's
        // configured env on top. We deliberately do NOT call `env_clear`
        // first so a checker that does not configure an env still sees
        // PATH and friends.
        .envs(spec.env.iter())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(unix)]
    {
        cmd.process_group(0);
    }

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            return CheckerRun {
                spawn_error: Some(e.to_string()),
                duration_ms: started.elapsed().as_millis() as i64,
                ..CheckerRun::default()
            };
        }
    };
    let child_id = child.id();

    let stdout_handle = child.stdout.take();
    let stderr_handle = child.stderr.take();

    let (out_tx, out_rx) = mpsc::channel();
    let (err_tx, err_rx) = mpsc::channel();
    let limit = spec.max_output_bytes;

    if let Some(stdout) = stdout_handle {
        let tx = out_tx;
        thread::spawn(move || {
            let captured = drain_bounded(stdout, limit);
            let _ = tx.send(captured);
        });
    } else {
        let _ = out_tx.send(BoundedCapture::default());
    }
    if let Some(stderr) = stderr_handle {
        let tx = err_tx;
        thread::spawn(move || {
            let captured = drain_bounded(stderr, limit);
            let _ = tx.send(captured);
        });
    } else {
        let _ = err_tx.send(BoundedCapture::default());
    }

    // Polling wait so we can apply a deadline. The polling interval is
    // small (10ms) so the timed-out case is detected quickly while
    // keeping the loop cheap for the success case.
    let poll = Duration::from_millis(10);
    let deadline = started + spec.timeout;
    let mut timed_out = false;
    let exit_status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) => {
                if Instant::now() >= deadline {
                    timed_out = true;
                    kill_child_group(&mut child, child_id);
                    // Reap the child so we do not leave a zombie.
                    let _ = child.wait();
                    break None;
                }
                thread::sleep(poll);
            }
            Err(_) => break None,
        }
    };

    // Collect the captured output. The drain threads normally finish
    // very shortly after the child exits because the pipes close. Use
    // a bounded recv so a grandchild holding the pipe cannot block the
    // command forever; a missing message means the thread panicked.
    let mut capture_errors: Vec<String> = Vec::new();
    let stdout = match out_rx.recv_timeout(Duration::from_secs(5)) {
        Ok(c) => {
            if let Some(e) = c.diagnostic {
                capture_errors.push(format!("stdout: {e}"));
            }
            c.capture
        }
        Err(_) => {
            capture_errors.push("stdout capture thread panicked or hung".into());
            InnerCapture::default()
        }
    };
    let stderr = match err_rx.recv_timeout(Duration::from_secs(5)) {
        Ok(c) => {
            if let Some(e) = c.diagnostic {
                capture_errors.push(format!("stderr: {e}"));
            }
            c.capture
        }
        Err(_) => {
            capture_errors.push("stderr capture thread panicked or hung".into());
            InnerCapture::default()
        }
    };

    let exit_code = exit_status.and_then(|s| s.code());
    // No code + no timeout + no spawn error => killed by a signal.
    let signalled =
        exit_code.is_none() && !timed_out && matches!(exit_status, Some(s) if s.code().is_none());

    CheckerRun {
        exit_code,
        stdout: stdout.text,
        stderr: stderr.text,
        stdout_truncated: stdout.truncated,
        stderr_truncated: stderr.truncated,
        timed_out,
        signalled,
        spawn_error: None,
        capture_error: if capture_errors.is_empty() {
            None
        } else {
            Some(capture_errors.join("; "))
        },
        duration_ms: started.elapsed().as_millis() as i64,
    }
}

fn kill_child_group(child: &mut std::process::Child, pid: u32) {
    #[cfg(unix)]
    {
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

#[derive(Debug, Default)]
struct InnerCapture {
    text: String,
    truncated: bool,
}

#[derive(Debug, Default)]
struct BoundedCapture {
    capture: InnerCapture,
    diagnostic: Option<String>,
}

/// Accumulate raw bytes up to `limit`, keep reading past it so the child
/// never blocks, then decode once. Read errors surface as a diagnostic.
fn drain_bounded<R: Read>(mut reader: R, limit: u64) -> BoundedCapture {
    let mut stored: Vec<u8> = Vec::with_capacity(limit.min(8192) as usize);
    let mut total_seen: u64 = 0;
    let mut diagnostic: Option<String> = None;
    let mut buf = [0u8; 8192];
    loop {
        match reader.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                total_seen += n as u64;
                if (stored.len() as u64) < limit {
                    let remaining = limit - stored.len() as u64;
                    let take = (n as u64).min(remaining) as usize;
                    stored.extend_from_slice(&buf[..take]);
                }
            }
            Err(e) => {
                diagnostic = Some(format!("stream read failed: {e}"));
                break;
            }
        }
    }
    let truncated = total_seen > limit;
    let text = String::from_utf8_lossy(&stored).into_owned();
    BoundedCapture {
        capture: InnerCapture { text, truncated },
        diagnostic,
    }
}

#[cfg(unix)]
#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use std::path::Path;
    use tempfile::tempdir;

    fn sh_spec(tmp: &tempfile::TempDir, script: &str, timeout_ms: u64) -> CheckerSpec {
        CheckerSpec {
            name: "t".into(),
            program: "sh".into(),
            args: vec!["-c".into(), script.into()],
            working_dir: tmp.path().to_path_buf(),
            env: BTreeMap::new(),
            timeout: Duration::from_millis(timeout_ms),
            max_output_bytes: DEFAULT_MAX_OUTPUT_BYTES,
        }
    }

    #[test]
    fn missing_executable_returns_spawn_error() {
        let tmp = tempdir().unwrap();
        let spec = CheckerSpec {
            name: "t".into(),
            program: "definitely-not-a-real-binary-xyz-12345".into(),
            args: vec![],
            working_dir: tmp.path().to_path_buf(),
            env: BTreeMap::new(),
            timeout: Duration::from_millis(1000),
            max_output_bytes: 1024,
        };
        let run = run_checker(&spec);
        assert!(run.spawn_error.is_some());
        assert!(run.exit_code.is_none());
    }

    #[test]
    fn successful_checker_records_zero_exit() {
        let tmp = tempdir().unwrap();
        let spec = sh_spec(&tmp, "exit 0", 1000);
        let run = run_checker(&spec);
        assert_eq!(run.exit_code, Some(0));
        assert!(!run.timed_out);
        assert!(!run.signalled);
        assert!(run.spawn_error.is_none());
    }

    #[test]
    fn nonzero_exit_propagates_without_timing_out() {
        let tmp = tempdir().unwrap();
        let spec = sh_spec(&tmp, "exit 7", 1000);
        let run = run_checker(&spec);
        assert_eq!(run.exit_code, Some(7));
        assert!(!run.timed_out);
        assert!(!run.signalled);
    }

    #[test]
    fn signal_killed_checker_is_signalled() {
        let tmp = tempdir().unwrap();
        let spec = sh_spec(&tmp, "kill -KILL $$", 2000);
        let run = run_checker(&spec);
        assert!(run.exit_code.is_none());
        assert!(!run.timed_out);
        assert!(run.spawn_error.is_none());
        assert!(run.signalled, "expected signalled, got {run:?}");
    }

    #[test]
    fn emoji_split_across_reads_is_preserved() {
        // Feed the drain an emoji split byte-by-byte.
        struct TinyChunks {
            data: Vec<u8>,
            pos: usize,
        }
        impl Read for TinyChunks {
            fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
                if self.pos >= self.data.len() {
                    return Ok(0);
                }
                buf[0] = self.data[self.pos];
                self.pos += 1;
                Ok(1)
            }
        }
        let mut data = b"hi ".to_vec();
        data.extend_from_slice("🎉".as_bytes());
        let out = drain_bounded(TinyChunks { data, pos: 0 }, 1024);
        assert!(out.diagnostic.is_none());
        assert!(out.capture.text.contains('🎉'));
        assert!(!out.capture.text.contains('�'));
    }

    #[test]
    fn read_error_surfaces_diagnostic() {
        struct Boom;
        impl Read for Boom {
            fn read(&mut self, _buf: &mut [u8]) -> std::io::Result<usize> {
                Err(std::io::Error::other("boom"))
            }
        }
        let out = drain_bounded(Boom, 1024);
        assert!(out.diagnostic.is_some());
    }

    #[test]
    fn drain_bounded_reads_past_limit_without_blocking_semantics() {
        let data = vec![b'A'; 5000];
        let out = drain_bounded(Cursor::new(data), 1024);
        assert!(out.capture.truncated);
        assert!(out.capture.text.len() <= 1024);
    }

    #[test]
    fn timeout_marks_run_as_timed_out() {
        let tmp = tempdir().unwrap();
        // Sleep 2s but only wait 100ms.
        let spec = sh_spec(&tmp, "sleep 2", 100);
        let run = run_checker(&spec);
        assert!(run.timed_out);
        assert!(run.exit_code.is_none());
    }

    #[test]
    fn timeout_kills_grandchild_holding_pipe() {
        let tmp = tempdir().unwrap();
        // Background sleep inherits stdout; only a group kill closes it.
        let spec = sh_spec(&tmp, "sleep 30 & exec sleep 30", 200);
        let started = Instant::now();
        let run = run_checker(&spec);
        assert!(run.timed_out, "got {run:?}");
        assert!(started.elapsed() < Duration::from_secs(10));
    }

    #[test]
    fn bounded_capture_marks_truncation() {
        let tmp = tempdir().unwrap();
        let mut spec = sh_spec(&tmp, "yes A | head -c 200000", 5000);
        spec.max_output_bytes = 1024;
        let run = run_checker(&spec);
        assert!(run.stdout_truncated, "expected truncation");
        assert!(run.stdout.len() <= 1024);
    }

    #[test]
    fn from_entry_uses_defaults_when_optional_fields_absent() {
        let entry = crate::project::config::CheckerEntry {
            name: "x".into(),
            command: "prog".into(),
            args: vec![],
            working_dir: None,
            env: BTreeMap::new(),
            timeout_ms: None,
            max_output_bytes: None,
        };
        let proj = Path::new("/tmp");
        let spec = CheckerSpec::from_entry(&entry, proj);
        assert_eq!(spec.working_dir, PathBuf::from("/tmp"));
        assert_eq!(spec.timeout, Duration::from_millis(DEFAULT_TIMEOUT_MS));
        assert_eq!(spec.max_output_bytes, DEFAULT_MAX_OUTPUT_BYTES);
    }

    #[test]
    fn from_entry_respects_overrides() {
        let entry = crate::project::config::CheckerEntry {
            name: "x".into(),
            command: "prog".into(),
            args: vec!["--json".into()],
            working_dir: Some(PathBuf::from("./sub")),
            env: BTreeMap::from([("FOO".into(), "bar".into())]),
            timeout_ms: Some(5000),
            max_output_bytes: Some(2048),
        };
        let proj = Path::new("/tmp");
        let spec = CheckerSpec::from_entry(&entry, proj);
        assert_eq!(spec.working_dir, PathBuf::from("./sub"));
        assert_eq!(spec.timeout, Duration::from_millis(5000));
        assert_eq!(spec.max_output_bytes, 2048);
        assert_eq!(spec.env.get("FOO").map(String::as_str), Some("bar"));
    }
}
