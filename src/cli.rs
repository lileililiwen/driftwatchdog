//! CLI argument definitions and dispatch surface.
//!
//! `clap` derive produces `--help` and validation; the binary calls into
//! `driftwatch::project` for behavior. Only the `init` command is implemented
//! in this change package.

use clap::{Args, Parser, Subcommand};

/// Driftwatch: local-first runtime failure memory.
#[derive(Debug, Parser)]
#[command(name = "driftwatch", version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Initialize the current directory as a Driftwatch project.
    Init(InitArgs),
}

#[derive(Debug, Args, Default, Clone)]
pub struct InitArgs {
    /// Skip creating `driftwatch.toml` even if it is absent.
    #[arg(long)]
    pub no_config: bool,
}
