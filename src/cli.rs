//! CLI argument definitions and dispatch surface.
//!
//! `clap` derive produces `--help` and validation; the binary calls into
//! the library for behavior.
//!
//! Binary naming rule: installs from Cargo, the shell installer, and
//! direct downloads expose the binary as `driftwatchdog`; the npm
//! launcher exposes the same binary as `driftwatch`. Examples below
//! use `driftwatch`; replace with `driftwatchdog` when that is the
//! binary on your `PATH`.

use clap::{Args, Parser, Subcommand};

/// Driftwatch: local-first runtime failure memory.
///
/// Records failed commands, recognizes recurring bugs, consumes
/// external spec-checker results, and produces context that humans
/// and coding agents can act on. State lives under `.driftwatch/`;
/// nothing is uploaded and no LLM is called.
#[derive(Debug, Parser)]
#[command(
    name = "driftwatch",
    version,
    about,
    long_about = "Local-first runtime failure memory for AI-assisted coding workflows.\n\nExamples:\n  driftwatch init\n  driftwatch run cargo test\n  driftwatch top\n  driftwatch show abcdef12\n  driftwatch report --ai > drift.md\n  driftwatch check --dry-run"
)]
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
    /// Run configured external checkers and record drift alerts.
    Check(CheckArgs),
    /// Resolve the project Gate plan, execute applicable checks
    /// locally, persist the result, and exit nonzero on blocking
    /// failures. Run locally before archive; CI repeats this command.
    Gate(GateArgs),
    /// Create an explicit link between a recurring bug and a drift alert.
    Link(LinkArgs),
    /// Remove a manual link by id.
    Unlink(UnlinkArgs),
    /// Print shell completions for the given shell to stdout.
    Completions(CompletionsArgs),
    /// Print a man page for the CLI to stdout.
    Man(ManArgs),
    /// Serve the Model Context Protocol (read-only tools) on stdio.
    Mcp(McpArgs),
}

#[derive(Debug, Args, Default, Clone)]
pub struct InitArgs {
    /// Skip creating `driftwatch.toml` even if it is absent.
    #[arg(long)]
    pub no_config: bool,
}

/// `driftwatch run [--tag <tag>]... <program> [args...]`
#[derive(Debug, Args, Default, Clone)]
#[command(
    after_help = "Examples:\n  driftwatch run cargo test\n  driftwatch run --tag db pytest -x\n  driftwatch run --timeout-ms 60000 npm test"
)]
pub struct RunArgs {
    /// Optional tag, repeatable. Stored alongside the run row.
    #[arg(long, value_name = "TAG")]
    pub tag: Vec<String>,

    /// Wall-clock timeout in milliseconds. On expiry the whole child
    /// process group is killed and the run is recorded as `timeout`.
    #[arg(long, value_name = "MS")]
    pub timeout_ms: Option<u64>,

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
    /// Hash prefix (hex chars), numeric `fingerprints.id`, or
    /// `id:<n>` for an explicit numeric lookup. Bare all-digit input
    /// tries the hash prefix first and only falls back to the numeric
    /// id when no hash matches.
    pub bug_id: String,
}

/// `driftwatch report [--limit N] [--days N] [--tag TAG]`
#[derive(Debug, Args, Default, Clone)]
#[command(
    after_help = "Examples:\n  driftwatch report\n  driftwatch report --ai > drift.md\n  driftwatch report --days 7 --tag db"
)]
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
    /// Render the AI-oriented context report instead of the standard Markdown.
    /// Includes project context, recurring failures, current spec violations,
    /// possible relationships, and an investigation task section.
    #[arg(long)]
    pub ai: bool,
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

/// `driftwatch check [--only NAMES] [--dry-run]`
#[derive(Debug, Args, Default, Clone)]
#[command(
    after_help = "Examples:\n  driftwatch check\n  driftwatch check --dry-run\n  driftwatch check --only architecture"
)]
pub struct CheckArgs {
    /// Restrict to the named checkers. Repeat to include more than one,
    /// or supply a single comma-separated list. When omitted, every
    /// configured checker in declaration order runs.
    #[arg(long, value_name = "NAME", value_delimiter = ',')]
    pub only: Vec<String>,
    /// Do not persist any rows. Useful for smoke-testing a checker
    /// command without polluting the database.
    #[arg(long)]
    pub dry_run: bool,
}

/// `driftwatch gate [--dry-run] [--format human|json]`
#[derive(Debug, Args, Default, Clone)]
#[command(
    after_help = "Examples:\n  driftwatch gate\n  driftwatch gate --dry-run\n  driftwatch gate --format json"
)]
pub struct GateArgs {
    /// Show the resolved plan without child execution or persistence.
    #[arg(long)]
    pub dry_run: bool,
    /// Output format. `human` prints a table; `json` prints a
    /// machine-readable status document to stdout.
    #[arg(long, value_enum, default_value_t = GateFormatArg::Human)]
    pub format: GateFormatArg,
}

#[derive(Debug, Clone, Copy, Default, clap::ValueEnum)]
pub enum GateFormatArg {
    #[default]
    Human,
    Json,
}

/// `driftwatch link bug:<id> spec:<id> [--note "..."]`
///
/// Both sides accept the documented `kind:<id>` form. The bug side
/// also accepts a bare hash prefix (8+ hex chars preferred) or
/// `id:<n>` for an explicit numeric `fingerprints.id`; bare
/// all-digit input tries the hash prefix first and only falls back
/// to the numeric id when no hash matches. The spec side accepts
/// the numeric `drift_alerts.id`.
#[derive(Debug, Args, Clone)]
#[command(
    after_help = "Examples:\n  driftwatch link bug:abcdef12 spec:42 --note \"see issue #108\"\n  driftwatch link id:7 spec:3"
)]
pub struct LinkArgs {
    /// Bug reference. Forms: `bug:<id>`, `id:<n>`, or bare hash prefix.
    pub bug: String,
    /// Spec/alert reference. Forms: `spec:<id>` or numeric `drift_alerts.id`.
    pub spec: String,
    /// Optional human note persisted alongside the link.
    #[arg(long, value_name = "NOTE")]
    pub note: Option<String>,
}

/// `driftwatch unlink <link-id>`
#[derive(Debug, Args, Clone)]
pub struct UnlinkArgs {
    /// Numeric `manual_links.id` to remove.
    pub link_id: i64,
}

/// `driftwatch completions <shell>` where `<shell>` is one of
/// `bash`, `zsh`, `fish`, `powershell`, or `elvish`.
#[derive(Debug, Args, Clone)]
pub struct CompletionsArgs {
    /// Shell to generate completions for.
    #[arg(value_enum, value_name = "SHELL")]
    pub shell: CompletionShell,
}

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum CompletionShell {
    Bash,
    Zsh,
    Fish,
    Powershell,
    Elvish,
}

/// `driftwatch man` — no flags.
#[derive(Debug, Args, Default, Clone)]
pub struct ManArgs {}

/// `driftwatch mcp` — no flags. Serves JSON-RPC 2.0 on stdio; the
/// process exits 0 when stdin closes.
#[derive(Debug, Args, Default, Clone)]
#[command(
    after_help = "Examples:\n  driftwatch mcp     # talk JSON-RPC on stdin/stdout\n  # register with an MCP-aware client by pointing its stdio command at `driftwatch mcp`."
)]
pub struct McpArgs {}
