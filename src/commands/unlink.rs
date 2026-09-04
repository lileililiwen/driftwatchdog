//! `driftwatch unlink <link-id>`
//!
//! Removes exactly one `manual_links` row by id. Returns
//! `Error::ManualLinkNotFound` when the id is not present so the
//! caller gets a clear error rather than a silent no-op.

use std::path::Path;

use crate::cli::UnlinkArgs;
use crate::error::Error;
use crate::project::ProjectRoot;
use crate::repo::{links::Links, Db};

const EXIT_OK: i32 = 0;

/// Execute `driftwatch unlink`. Returns 0 on success.
pub fn unlink(args: UnlinkArgs, cwd: &Path) -> Result<i32, Error> {
    let proj = ProjectRoot::discover(cwd)?;
    let mut db = Db::open(&proj.db_path)?;
    if Links::delete(&mut db, args.link_id)? {
        println!("driftwatch: removed manual link #{}", args.link_id);
        Ok(EXIT_OK)
    } else {
        Err(Error::ManualLinkNotFound { id: args.link_id })
    }
}
