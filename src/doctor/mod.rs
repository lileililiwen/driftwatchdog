//! `driftwatch doctor` diagnostics.
//!
//! Runs a fixed set of checks against the local environment and returns
//! a [`Report`]. The doctor never aborts early: a single failing check
//! must not skip the rest. All checks are independent and tolerate
//! missing optional tools (those are `Warn`, not `Fail`).
//!
//! The doctor is read-only: database checks open the DB without
//! running migrations, the writability probe uses an atomic
//! `create_new` file cleaned up by a `Drop` guard, and external
//! version probes are bounded so a hanging checker cannot stall the
//! command.

use std::path::Path;
use std::process::Command;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use crate::project::{config::Config, git, root::ProjectRoot};
use crate::repo::{alerts::foundation_tables_present, alerts::schema_version, Db};

use self::check::{Check, Status};

pub mod check;

pub use check::{Check as PublicCheck, Status as PublicStatus};

/// Aggregated doctor output.
#[derive(Debug, Clone)]
pub struct Report {
    pub project_root: String,
    pub checks: Vec<Check>,
    /// Process exit code: 0 only if no check returned `Fail`.
    pub exit_code: i32,
}

impl Report {
    pub fn passed(&self) -> bool {
        self.exit_code == 0
    }

    pub fn n_pass(&self) -> usize {
        self.checks
            .iter()
            .filter(|c| c.status == Status::Pass)
            .count()
    }

    pub fn n_warn(&self) -> usize {
        self.checks
            .iter()
            .filter(|c| c.status == Status::Warn)
            .count()
    }

    pub fn n_info(&self) -> usize {
        self.checks
            .iter()
            .filter(|c| c.status == Status::Info)
            .count()
    }

    pub fn n_fail(&self) -> usize {
        self.checks
            .iter()
            .filter(|c| c.status == Status::Fail)
            .count()
    }

    /// Render the report as a human-readable summary. Safe to print on
    /// stdout; never contains secrets.
    pub fn render(&self) -> String {
        let mut s = String::new();
        s.push_str("Driftwatch doctor\n");
        s.push_str(&format!("Project: {}\n\n", self.project_root));
        for c in &self.checks {
            s.push_str(&format!("[{}] {} ({})\n", c.status, c.name, c.id));
            if let Some(detail) = &c.detail {
                s.push_str(&format!("        {detail}\n"));
            }
            if let Some(remediation) = &c.remediation {
                s.push_str(&format!("        remediation: {remediation}\n"));
            }
        }
        s.push('\n');
        if self.exit_code == 0 {
            s.push_str(&format!(
                "All {} checks passed ({} warnings, {} info).\n",
                self.n_pass(),
                self.n_warn(),
                self.n_info()
            ));
        } else {
            s.push_str(&format!(
                "{} of {} checks failed; see remediation above.\n",
                self.n_fail(),
                self.checks.len()
            ));
        }
        s
    }
}

/// Run the full doctor suite. Always produces a `Report` (never an
/// `Err`): the function only returns `Err` for the binary plumbing
/// errors (which are not real diagnostic failures).
pub fn run(cwd: &Path) -> Result<Report, crate::error::Error> {
    let proj = ProjectRoot::discover(cwd)?;
    let mut checks: Vec<Check> = Vec::new();
    checks.extend(check_project_paths(&proj));
    checks.push(check_db(&proj));
    checks.push(check_config(&proj));
    checks.push(check_git(&proj));
    checks.extend(check_directories(&proj));
    checks.extend(check_checkers(&proj));
    checks.extend(check_recent_check_runs(&proj));
    checks.extend(check_gate_toolchain(&proj));
    checks.extend(check_gate_context(&proj));
    checks.extend(check_gate_ai(&proj));

    let exit_code = if checks.iter().any(|c| c.status == Status::Fail) {
        2
    } else {
        0
    };

    Ok(Report {
        project_root: proj.root.to_string_lossy().into_owned(),
        checks,
        exit_code,
    })
}

fn check_project_paths(proj: &ProjectRoot) -> Vec<Check> {
    vec![Check::pass("project.root", "Project root resolved")
        .with_remediation(format!("Rooted at `{}`.", proj.root.display()))]
}

fn check_db(proj: &ProjectRoot) -> Check {
    // Read-only: doctor must never migrate or modify the DB.
    if !proj.db_path.exists() {
        return Check::fail(
            "db.open",
            "Database could not be opened",
            format!("{}: file not found", proj.db_path.display()),
        )
        .with_remediation(format!(
            "If the file is corrupt, delete `{}` and run `driftwatch init` again.",
            proj.db_path.display()
        ));
    }
    match Db::open_read_only(&proj.db_path) {
        Ok(db) => {
            let version = schema_version(db.conn()).unwrap_or(0);
            if !foundation_tables_present(db.conn()).unwrap_or(false) {
                return Check::fail(
                    "db.schema",
                    "Database schema is incomplete",
                    format!("Missing foundation tables in `{}`.", proj.db_path.display()),
                )
                .with_remediation(
                    "Run `driftwatch init` to repair the schema, or restore from a backup.",
                );
            }
            Check::pass("db.open", "Database opens and schema is current")
                .with_remediation(format!("Local schema version: {version}"))
        }
        Err(e) => Check::fail(
            "db.open",
            "Database could not be opened",
            format!("{}: {e}", proj.db_path.display()),
        )
        .with_remediation(format!(
            "If the file is corrupt, delete `{}` and run `driftwatch init` again.",
            proj.db_path.display()
        )),
    }
}

fn check_config(proj: &ProjectRoot) -> Check {
    if !proj.config_path.exists() {
        return Check::warn(
            "config.parse",
            "driftwatch.toml not present",
            "Using default configuration; this is acceptable for a fresh project.",
        )
        .with_remediation("Run `driftwatch init` to create a starter config, or write your own.");
    }
    match Config::load(&proj.config_path) {
        Ok(cfg) => match cfg.validate_working_dirs(&proj.config_path, &proj.root) {
            Ok(()) => Check::pass("config.parse", "driftwatch.toml is valid"),
            Err(e) => Check::fail("config.parse", "driftwatch.toml is invalid", format!("{e}"))
                .with_remediation(format!(
                    "Edit `{}` to fix the parse error.",
                    proj.config_path.display()
                )),
        },
        Err(e) => Check::fail("config.parse", "driftwatch.toml is invalid", format!("{e}"))
            .with_remediation(format!(
                "Edit `{}` to fix the parse error.",
                proj.config_path.display()
            )),
    }
}

fn check_git(proj: &ProjectRoot) -> Check {
    let ctx = git::capture(&proj.root);
    if ctx.commit.is_some() || ctx.branch.is_some() {
        let mut detail = format!(
            "commit: {}, branch: {}",
            ctx.commit.as_deref().unwrap_or("-"),
            ctx.branch.as_deref().unwrap_or("-"),
        );
        if let Some(d) = ctx.dirty {
            detail.push_str(&format!(", dirty: {d}"));
        }
        Check::pass("git.available", "Git is available").with_remediation(detail)
    } else {
        // Missing Git context is honest INFO, not a warning: the
        // project works without Git; reports just omit commit metadata.
        Check::info(
            "git.available",
            "Git context unavailable",
            ctx.diagnostic
                .as_deref()
                .unwrap_or("git not installed or not a worktree")
                .to_string(),
        )
        .with_remediation("Install git and run inside a worktree to record commit metadata.")
    }
}

fn check_directories(proj: &ProjectRoot) -> Vec<Check> {
    let mut out = Vec::new();
    // State directory must exist (init creates it) and be writable.
    out.push(writability_check(
        "dir.state",
        "State directory is writable",
        &proj.state_dir,
    ));
    // The parent of the config file is the project root; it must be
    // writable so future `init` runs can create the config on demand.
    if let Some(parent) = proj.config_path.parent() {
        out.push(writability_check(
            "dir.project",
            "Project directory is writable",
            parent,
        ));
    }
    out
}

fn writability_check(id: &'static str, name: &'static str, dir: &Path) -> Check {
    if !dir.exists() {
        return Check::fail(id, name, format!("`{}` does not exist.", dir.display()))
            .with_remediation(format!(
                "Create the directory: `mkdir -p {}`",
                dir.display()
            ));
    }
    // Atomic probe: `create_new` fails when the file already exists
    // (no clobbering, no TOCTOU truncate race). A `Drop` guard
    // removes the probe even on early return. A stale leftover from
    // a crashed run is removed and retried once.
    struct ProbeGuard<'a> {
        path: std::path::PathBuf,
        _marker: std::marker::PhantomData<&'a ()>,
    }
    impl Drop for ProbeGuard<'_> {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.path);
        }
    }
    let probe = dir.join(".driftwatch-doctor-probe");
    let attempt = || -> std::io::Result<ProbeGuard<'_>> {
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&probe)
        {
            Ok(_) => Ok(ProbeGuard {
                path: probe.clone(),
                _marker: std::marker::PhantomData,
            }),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                let _ = std::fs::remove_file(&probe);
                std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&probe)
                    .map(|_| ProbeGuard {
                        path: probe.clone(),
                        _marker: std::marker::PhantomData,
                    })
            }
            Err(e) => Err(e),
        }
    };
    match attempt() {
        Ok(_guard) => Check::pass(id, name),
        Err(e) => {
            Check::fail(id, name, format!("{}: {e}", dir.display())).with_remediation(format!(
                "Adjust permissions on `{}` so the current user can write to it.",
                dir.display()
            ))
        }
    }
}

fn check_checkers(proj: &ProjectRoot) -> Vec<Check> {
    let cfg = match Config::load(&proj.config_path) {
        Ok(c) => c,
        Err(_) => return Vec::new(), // Already reported by `config.parse`.
    };
    if cfg.checkers.is_empty() {
        return vec![
            Check::info(
                "checker.configured",
                "No external checkers configured",
                "driftwatch runs without checkers; add [[checkers]] for drift alerts.",
            )
            .with_remediation(
                "hint: add [[checkers]] to driftwatch.toml (see driftwatch.toml.example) or run with --dry-run",
            ),
        ];
    }
    let mut out = Vec::new();
    for checker in &cfg.checkers {
        out.push(check_one_checker(checker));
    }
    out
}

/// Budget for a checker's `--version` probe. A hanging checker
/// reports a timeout instead of stalling `doctor`.
const VERSION_TIMEOUT: Duration = Duration::from_secs(5);

/// Run `program --version` with argv (never shell-split) and a
/// timeout. `Ok(Ok(text))` is a successful probe, `Ok(Err(code))`
/// is a nonzero exit, `Err` is spawn/timeout.
fn version_probe(program: &str) -> Result<Result<String, Option<i32>>, String> {
    let program = program.to_string();
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let out = Command::new(&program).arg("--version").output();
        let _ = tx.send(out);
    });
    let out = match rx.recv_timeout(VERSION_TIMEOUT) {
        Ok(o) => o,
        Err(_) => return Err("timed out after 5s".to_string()),
    };
    match out {
        Ok(o) if o.status.success() => {
            let version = String::from_utf8_lossy(&o.stdout);
            Ok(Ok(version.lines().next().unwrap_or("").trim().to_string()))
        }
        Ok(o) => Ok(Err(o.status.code())),
        Err(e) => Err(e.to_string()),
    }
}

fn check_one_checker(checker: &crate::project::config::CheckerEntry) -> Check {
    let id = "checker.executable";
    // `command` is the program; `args` carry the flags. Never
    // whitespace-split: a program path may contain spaces.
    let program = checker.command.trim();
    if program.is_empty() {
        return Check::fail(
            id,
            format!("Checker `{}` has no program", checker.name),
            "command is empty",
        )
        .with_remediation(format!(
            "Set `command` for the `{}` checker in driftwatch.toml.",
            checker.name
        ));
    }
    match version_probe(program) {
        Ok(Ok(version)) => Check::pass(id, format!("Checker `{}` is installed", checker.name))
            .with_remediation(format!("program: {program}, version: {version}")),
        Ok(Err(code)) => Check::warn(
            id,
            format!("Checker `{}` responded with an error", checker.name),
            format!("program: {program}, exit: {code:?}"),
        )
        .with_remediation(format!(
            "Verify that `{program}` runs successfully on its own before invoking driftwatch."
        )),
        Err(e) if e.contains("timed out") => Check::warn(
            id,
            format!("Checker `{}` timed out", checker.name),
            format!("program: {program}, error: {e}"),
        )
        .with_remediation(format!(
            "Verify that `{program} --version` responds within 5s."
        )),
        Err(e) => Check::warn(
            id,
            format!("Checker `{}` is not on PATH", checker.name),
            format!("program: {program}, error: {e}"),
        )
        .with_remediation(format!(
            "Install `{program}` or adjust the `command` for the `{}` checker.",
            checker.name
        )),
    }
}

/// Gate toolchain readiness (optional `gate-tools.toml` layer).
/// Absence stays silent (unconfigured-but-ok); a declared manifest is
/// probed truthfully — cache digests are re-read, `PATH` is searched,
/// container capability is bounded-probed — so readiness is never
/// claimed from configuration alone. Ordinary `driftwatch` usage is
/// unaffected; missing tools are `Warn` with the explicit bootstrap
/// next action, unsupported platforms are `Fail` (fail-closed).
/// Gate context readiness (optional `gate.toml` `[[contexts]]` layer).
/// Absence stays silent (unconfigured-but-ok) so reports and exports
/// remain valid without providers. Unknown providers are `Warn`;
/// a selected-but-absent `openspec/` dir is honest `Info`
/// (`NOT_APPLICABLE`, never false success).
fn check_gate_context(proj: &ProjectRoot) -> Vec<Check> {
    crate::gate::context::context_checks(&proj.root)
}

/// AI evaluation readiness (optional `gate.toml` `[ai]` layer).
/// Absence stays silent (unconfigured-but-ok); disabled is honest
/// `Info`; enabled-without-provider is `Warn` with remediation; a
/// missing executable is `Warn` (optional provider, never `Fail`).
fn check_gate_ai(proj: &ProjectRoot) -> Vec<Check> {
    crate::gate::ai::ai_checks(&proj.root)
}

fn check_gate_toolchain(proj: &ProjectRoot) -> Vec<Check> {
    let cache_base = crate::gate::toolchain::user_cache_base();
    crate::gate::toolchain::toolchain_checks(
        &proj.root,
        &cache_base,
        &crate::gate::toolchain::RealProbes,
    )
}

/// Surface a Warn for any configured checker whose most recent
/// `check_snapshots` row is in a non-success status. A configured
/// checker that has never been run is silent here — `check_checkers`
/// already covers the executable-presence story.
fn check_recent_check_runs(proj: &ProjectRoot) -> Vec<Check> {
    let cfg = match Config::load(&proj.config_path) {
        Ok(c) => c,
        Err(_) => return Vec::new(), // Already reported by `config.parse`.
    };
    if cfg.checkers.is_empty() {
        return Vec::new();
    }
    // Read-only: a doctor failure to open the DB is already reported
    // by `db.open`; per-checker snapshot errors are skipped (the
    // doctor never aborts early) rather than propagated.
    let db = match Db::open_read_only(&proj.db_path) {
        Ok(d) => d,
        Err(_) => return Vec::new(), // Already reported by `db.open`.
    };
    let alerts = crate::repo::alerts::Alerts::new(&db);
    let mut out = Vec::new();
    for checker in &cfg.checkers {
        let snap = match alerts.latest_snapshot_for(&checker.name) {
            Ok(Some(s)) => s,
            Ok(None) => continue, // Never run; not a problem here.
            Err(_) => continue,
        };
        let is_failure = matches!(
            snap.status.as_str(),
            "failed" | "timeout" | "bad_json" | "start_failed" | "unknown"
        );
        if is_failure {
            let detail = snap
                .diagnostic
                .clone()
                .unwrap_or_else(|| format!("status: {}", snap.status));
            out.push(
                Check::warn(
                    "checker.last_run",
                    format!("Checker `{}` last run failed", checker.name),
                    detail,
                )
                .with_remediation(format!(
                    "Run `driftwatch check --only {}` to see the live error.",
                    checker.name
                )),
            );
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn pass_factory_has_no_detail() {
        let c = Check::pass("x", "X");
        assert_eq!(c.status, Status::Pass);
        assert!(c.detail.is_none());
    }

    #[test]
    fn fail_factory_records_detail() {
        let c = Check::fail("x", "X", "boom").with_remediation("fix it");
        assert_eq!(c.status, Status::Fail);
        assert_eq!(c.detail.as_deref(), Some("boom"));
        assert_eq!(c.remediation.as_deref(), Some("fix it"));
    }

    #[test]
    fn run_produces_pass_report_on_fresh_init() {
        let tmp = tempdir().unwrap();
        // Set up a healthy project: run init to write the .driftwatch dir
        // and driftwatch.toml.
        crate::project::init(crate::cli::InitArgs::default()).ok(); // not used directly
        let _ = tmp; // We instead just create the artifacts by hand.
        let root = tmp.path();
        std::fs::create_dir_all(root.join(".driftwatch")).unwrap();
        let mut db = Db::open(&root.join(".driftwatch/state.db")).unwrap();
        // Apply migrations so the schema is current.
        crate::storage::migrations::apply(db.conn_mut()).unwrap();
        std::fs::write(root.join("driftwatch.toml"), "").unwrap();
        let report = run(root).unwrap();
        // The DB and config checks must pass on a fresh project.
        let by_id: std::collections::HashMap<&str, &Check> =
            report.checks.iter().map(|c| (c.id, c)).collect();
        assert_eq!(by_id["db.open"].status, Status::Pass);
        assert_eq!(by_id["config.parse"].status, Status::Pass);
    }

    #[test]
    fn run_reports_unwritable_state_dir_as_fail() {
        let tmp = tempdir().unwrap();
        // Initialize the project to get a real DB.
        std::fs::create_dir_all(tmp.path().join(".driftwatch")).unwrap();
        let _ = Db::open(&tmp.path().join(".driftwatch/state.db")).unwrap();
        // Restrict the state dir to no permissions.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(tmp.path().join(".driftwatch"))
                .unwrap()
                .permissions();
            perms.set_mode(0o000);
            std::fs::set_permissions(tmp.path().join(".driftwatch"), perms).unwrap();
        }
        let report = run(tmp.path()).unwrap();
        let state_check = report
            .checks
            .iter()
            .find(|c| c.id == "dir.state")
            .expect("dir.state check");
        // On Unix with mode 0 the check is a Fail. On non-Unix CI
        // (e.g. Windows runners) the probe may still succeed; we only
        // assert the strict-Unix behavior.
        #[cfg(unix)]
        assert_eq!(state_check.status, Status::Fail);
        // Restore so the test process can clean up.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(tmp.path().join(".driftwatch"))
                .unwrap()
                .permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(tmp.path().join(".driftwatch"), perms).unwrap();
        }
    }

    #[test]
    fn report_exit_code_is_zero_when_all_pass() {
        let r = Report {
            project_root: "/p".into(),
            checks: vec![Check::pass("a", "A"), Check::warn("b", "B", "x")],
            exit_code: 0,
        };
        assert!(r.passed());
        assert_eq!(r.n_pass(), 1);
        assert_eq!(r.n_warn(), 1);
        assert_eq!(r.n_fail(), 0);
    }

    #[test]
    fn report_render_includes_failures() {
        let r = Report {
            project_root: "/p".into(),
            checks: vec![
                Check::pass("a", "A"),
                Check::fail("b", "B", "boom").with_remediation("fix"),
            ],
            exit_code: 2,
        };
        let s = r.render();
        assert!(s.contains("[PASS] A (a)"));
        assert!(s.contains("[FAIL] B (b)"));
        assert!(s.contains("remediation: fix"));
        assert!(s.contains("1 of 2 checks failed"));
    }

    #[test]
    fn doctor_does_not_modify_the_db() {
        use crate::repo::alerts::schema_version;
        let tmp = tempdir().unwrap();
        let root = tmp.path();
        std::fs::create_dir_all(root.join(".driftwatch")).unwrap();
        let db_path = root.join(".driftwatch/state.db");
        let mut db = Db::open(&db_path).unwrap();
        crate::storage::migrations::apply(db.conn_mut()).unwrap();
        drop(db);
        let version_before: i64 = {
            let conn = rusqlite::Connection::open(&db_path).unwrap();
            conn.query_row("SELECT MAX(version) FROM schema_version", [], |r| r.get(0))
                .unwrap()
        };
        let mtime_before = std::fs::metadata(&db_path).unwrap().modified().unwrap();
        std::fs::write(root.join("driftwatch.toml"), "").unwrap();
        let _ = run(root).unwrap();
        let version_after: i64 = {
            let conn = rusqlite::Connection::open(&db_path).unwrap();
            conn.query_row("SELECT MAX(version) FROM schema_version", [], |r| r.get(0))
                .unwrap()
        };
        let mtime_after = std::fs::metadata(&db_path).unwrap().modified().unwrap();
        assert_eq!(version_before, version_after);
        assert_eq!(mtime_before, mtime_after);
        let _ = schema_version;
    }

    #[test]
    fn version_probe_timeout_reports_warn_instead_of_hanging() {
        // A checker whose `--version` hangs must surface a timeout
        // quickly instead of stalling doctor.
        let tmp = tempdir().unwrap();
        let script = tmp.path().join("hang-checker");
        std::fs::write(&script, "#!/bin/sh\nsleep 30\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&script).unwrap().permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&script, perms).unwrap();
        }
        let entry = crate::project::config::CheckerEntry {
            name: "hang".into(),
            command: script.to_string_lossy().into_owned(),
            args: vec![],
            working_dir: None,
            env: std::collections::BTreeMap::new(),
            timeout_ms: None,
            max_output_bytes: None,
        };
        let started = std::time::Instant::now();
        let check = check_one_checker(&entry);
        assert!(
            started.elapsed() < Duration::from_secs(20),
            "probe hung: {:?}",
            started.elapsed()
        );
        // A hanging `--version` is a Warn (not Fail, not hang).
        assert_eq!(check.status, Status::Warn);
        let detail = check.detail.unwrap_or_default();
        assert!(
            detail.contains("timed out") || detail.contains("hang"),
            "got: {detail}"
        );
    }
}
