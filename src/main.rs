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
        };
    Ok(ExitCode::from(code as u8))
}
