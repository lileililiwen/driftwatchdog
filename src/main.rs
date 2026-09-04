use anyhow::Context;
use clap::Parser;
use driftwatch::cli::{Cli, Command};

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Init(args) => {
            let _ = driftwatch::project::init(args)
                .with_context(|| "failed to initialize driftwatch project")?;
        }
    }
    Ok(())
}
