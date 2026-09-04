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
    /// Show a single recurring bug by hash prefix or numeric id.
    Show(ShowArgs),
    /// Render a Markdown report of recurring failures and trends.
    Report(ReportArgs),
    /// Garbage-collect bulky stdout/stderr older than `--days`.
    Gc(GcArgs),
    /// Export persisted data as JSON, JSONL, or Markdown to stdout.
    Export(ExportArgs),
    /// Diagnose the local installation and print pass/warn/fail checks.
    Doctor(DoctorArgs),
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

/// `driftwatch show <bug-id>`
#[derive(Debug, Args, Clone)]
pub struct ShowArgs {
    /// Hash prefix (hex chars) or numeric `fingerprints.id`.
    pub bug_id: String,
}

/// `driftwatch report [--limit N] [--days N] [--tag TAG]`
#[derive(Debug, Args, Default, Clone)]
pub struct ReportArgs {
    /// Maximum fingerprints to include.
    #[arg(long, default_value_t = 20)]
    pub limit: usize,
    /// Restrict to fingerprints last seen within this many days.
    #[arg(long, default_value_t = 30)]
    pub days: i64,
    /// Restrict to fingerprints whose occurrences include a run with this tag.
    #[arg(long, value_name = "TAG")]
    pub tag: Option<String>,
}

/// `driftwatch gc [--days N] [--dry-run]`
#[derive(Debug, Args, Clone)]
pub struct GcArgs {
    /// Remove bulky streams for runs older than this many days. Default 90.
    #[arg(long, default_value_t = 90)]
    pub days: i64,
    /// Print what would change without modifying the database.
    #[arg(long)]
    pub dry_run: bool,
}

/// `driftwatch export <format>` where `<format>` is one of `json`,
/// `jsonl`, or `markdown`.
#[derive(Debug, Args, Clone)]
pub struct ExportArgs {
    /// Output format. `json` is a single document; `jsonl` is one
    /// record per line; `markdown` is a human-readable projection.
    #[arg(value_enum, value_name = "FORMAT")]
    pub format: ExportFormatArg,
}

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum ExportFormatArg {
    Json,
    Jsonl,
    Markdown,
}

/// `driftwatch doctor` — no flags in v1.
#[derive(Debug, Args, Default, Clone)]
pub struct DoctorArgs {}
