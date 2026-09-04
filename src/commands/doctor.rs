//! `driftwatch doctor`: run the diagnostic suite and print the report.

use std::io::Write;
use std::path::Path;

use crate::cli::DoctorArgs;
use crate::doctor;
use crate::error::Error;

/// Run the `doctor` command. The process exit code reflects the report:
/// 0 on a clean (or warn-only) diagnostic, 2 if any required check
/// failed.
pub fn doctor(args: DoctorArgs, cwd: &Path) -> Result<i32, Error> {
    let _ = args; // no flags in v1
    let report = doctor::run(cwd)?;
    let rendered = report.render();
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    handle.write_all(rendered.as_bytes())?;
    handle.flush()?;
    Ok(report.exit_code)
}
