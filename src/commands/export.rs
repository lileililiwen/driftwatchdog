//! `driftwatch export <format>`: render a portable export to stdout.

use std::io::Write;
use std::path::Path;

use crate::cli::{ExportArgs, ExportFormatArg};
use crate::error::Error;
use crate::export::{self, Format};
use crate::project::ProjectRoot;
use crate::repo::Db;

/// Run the `export` command. Always returns exit 0 on success.
pub fn export(args: ExportArgs, cwd: &Path) -> Result<i32, Error> {
    let proj = ProjectRoot::discover(cwd)?;
    let mut db = Db::open(&proj.db_path)?;
    let fmt = match args.format {
        ExportFormatArg::Json => Format::Json,
        ExportFormatArg::Jsonl => Format::Jsonl,
        ExportFormatArg::Markdown => Format::Markdown,
    };
    let rendered = export::format(&mut db, &proj, fmt)?;
    // Write to stdout in one go to keep the output deterministic and
    // avoid interleaving with other tools' stderr.
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    handle.write_all(rendered.as_bytes())?;
    handle.flush()?;
    Ok(0)
}
