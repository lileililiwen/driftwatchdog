use anyhow::Context;
use clap::Parser;
use driftwatch::cli::{Cli, Command};
use std::process::ExitCode;

fn main() -> anyhow::Result<ExitCode> {
    let cli = Cli::parse();
    let cwd = std::env::current_dir().map_err(|e| anyhow::anyhow!("cwd: {e}"))?;
    let code =
        match cli.command {
            Command::Init(args) => {
                driftwatch::project::init(args)
                    .with_context(|| "failed to initialize driftwatch project")?;
                0
            }
            Command::Run(args) => driftwatch::commands::run_cmd(args, &cwd)
                .with_context(|| "driftwatch run failed")?,
            Command::List(args) => driftwatch::commands::list_cmd(args, &cwd)
                .with_context(|| "driftwatch list failed")?,
            Command::Top(args) => driftwatch::commands::top_cmd(args, &cwd)
                .with_context(|| "driftwatch top failed")?,
            Command::Show(args) => driftwatch::commands::show_cmd(args, &cwd)
                .with_context(|| "driftwatch show failed")?,
            Command::Report(args) => driftwatch::commands::report_cmd(args, &cwd)
                .with_context(|| "driftwatch report failed")?,
            Command::Gc(args) => {
                driftwatch::commands::gc_cmd(args, &cwd).with_context(|| "driftwatch gc failed")?
            }
            Command::Export(args) => driftwatch::commands::export_cmd(args, &cwd)
                .with_context(|| "driftwatch export failed")?,
            Command::Doctor(args) => driftwatch::commands::doctor_cmd(args, &cwd)
                .with_context(|| "driftwatch doctor failed")?,
            Command::Check(args) => driftwatch::commands::check_cmd(args, &cwd)
                .with_context(|| "driftwatch check failed")?,
            Command::Link(args) => driftwatch::commands::link_cmd(args, &cwd)
                .with_context(|| "driftwatch link failed")?,
            Command::Unlink(args) => driftwatch::commands::unlink_cmd(args, &cwd)
                .with_context(|| "driftwatch unlink failed")?,
        };
    Ok(ExitCode::from(code as u8))
}
