use anyhow::Context;
use clap::Parser;
use driftwatchdog::cli::{Cli, Command};
use std::process::ExitCode;

fn main() -> anyhow::Result<ExitCode> {
    match run() {
        Ok(code) => Ok(code),
        Err(e) => {
            // Single-wrap remediation: surface the library hint (if
            // any) alongside the anyhow chain instead of burying it.
            if let Some(lib) = e
                .chain()
                .find_map(|cause| cause.downcast_ref::<driftwatchdog::error::Error>())
            {
                if let Some(hint) = lib.hint() {
                    eprintln!("hint: {hint}");
                }
            }
            Err(e)
        }
    }
}

fn run() -> anyhow::Result<ExitCode> {
    let cli = Cli::parse();
    let cwd = std::env::current_dir().map_err(|e| anyhow::anyhow!("cwd: {e}"))?;
    let code = match cli.command {
        Command::Init(args) => {
            driftwatchdog::project::init(args)
                .with_context(|| "failed to initialize driftwatchdog project")?;
            0
        }
        Command::Run(args) => driftwatchdog::commands::run_cmd(args, &cwd)
            .with_context(|| "driftwatchdog run failed")?,
        Command::List(args) => driftwatchdog::commands::list_cmd(args, &cwd)
            .with_context(|| "driftwatchdog list failed")?,
        Command::Top(args) => driftwatchdog::commands::top_cmd(args, &cwd)
            .with_context(|| "driftwatchdog top failed")?,
        Command::Show(args) => driftwatchdog::commands::show_cmd(args, &cwd)
            .with_context(|| "driftwatchdog show failed")?,
        Command::Report(args) => driftwatchdog::commands::report_cmd(args, &cwd)
            .with_context(|| "driftwatchdog report failed")?,
        Command::Gc(args) => driftwatchdog::commands::gc_cmd(args, &cwd)
            .with_context(|| "driftwatchdog gc failed")?,
        Command::Export(args) => driftwatchdog::commands::export_cmd(args, &cwd)
            .with_context(|| "driftwatchdog export failed")?,
        Command::Doctor(args) => driftwatchdog::commands::doctor_cmd(args, &cwd)
            .with_context(|| "driftwatchdog doctor failed")?,
        Command::Check(args) => driftwatchdog::commands::check_cmd(args, &cwd)
            .with_context(|| "driftwatchdog check failed")?,
        Command::Link(args) => driftwatchdog::commands::link_cmd(args, &cwd)
            .with_context(|| "driftwatchdog link failed")?,
        Command::Unlink(args) => driftwatchdog::commands::unlink_cmd(args, &cwd)
            .with_context(|| "driftwatchdog unlink failed")?,
        Command::Completions(args) => driftwatchdog::commands::completions_cmd(args)
            .with_context(|| "driftwatchdog completions failed")?,
        Command::Man(_) => {
            driftwatchdog::commands::man_cmd().with_context(|| "driftwatchdog man failed")?
        }
        Command::Mcp(_) => {
            driftwatchdog::mcp::server::run(&cwd).with_context(|| "driftwatchdog mcp failed")?
        }
    };
    Ok(ExitCode::from(code as u8))
}
