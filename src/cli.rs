//! CLI argument definitions and dispatch surface.
//!
//! `clap` derive produces `--help` and validation; the binary calls into
//! the library for behavior.

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
    /// Run a project command and record the outcome.
    Run(RunArgs),
    /// List recorded runs (newest first).
    List(ListArgs),
    /// Show grouped recurring failures.
    Top(TopArgs),
}

#[derive(Debug, Args, Default, Clone)]
pub struct InitArgs {
    /// Skip creating `driftwatch.toml` even if it is absent.
    #[arg(long)]
    pub no_config: bool,
}

/// `driftwatch run [--tag <tag>]... <program> [args...]`
#[derive(Debug, Args, Default, Clone)]
pub struct RunArgs {
    /// Optional tag, repeatable. Stored alongside the run row.
    #[arg(long, value_name = "TAG")]
    pub tag: Vec<String>,

    /// The program and its arguments. Everything after the optional `--tag`
    /// flags is captured here as a vector, with `command[0]` treated as the
    /// program name. `allow_hyphen_values` keeps clap from trying to parse
    /// flags inside the wrapped command (e.g. `driftwatch run cargo test
    /// --release`).
    #[arg(
        trailing_var_arg = true,
        allow_hyphen_values = true,
        required = true,
        num_args = 1..
    )]
    pub command: Vec<String>,
}

/// `driftwatch list [--limit N] [--failed] [--tag TAG]`
#[derive(Debug, Args, Default, Clone)]
pub struct ListArgs {
    /// Maximum number of rows to show.
    #[arg(long, default_value_t = 20)]
    pub limit: usize,
    /// Show only failed or start-failed runs.
    #[arg(long)]
    pub failed: bool,
    /// Restrict to runs that carry this tag.
    #[arg(long, value_name = "TAG")]
    pub tag: Option<String>,
}

/// `driftwatch top [--limit N] [--days N] [--tag TAG]`
#[derive(Debug, Args, Default, Clone)]
pub struct TopArgs {
    /// Maximum number of groups to show.
    #[arg(long, default_value_t = 20)]
    pub limit: usize,
    /// Restrict to fingerprints last seen within this many days.
    #[arg(long, default_value_t = 30)]
    pub days: i64,
    /// Restrict to fingerprints whose occurrences include a run with this tag.
    #[arg(long, value_name = "TAG")]
    pub tag: Option<String>,
}
