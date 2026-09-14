//! `driftwatch completions` / `driftwatch man`: shell completions and
//! man-page generation from the `clap` command definition.

use std::io::Write;

use clap::CommandFactory;
use clap_complete::{generate, Shell};

use crate::cli::{Cli, CompletionShell, CompletionsArgs};
use crate::error::Error;

/// Print completions for `shell` to stdout. Always exits 0 on success.
pub fn completions(args: CompletionsArgs) -> Result<i32, Error> {
    let shell = match args.shell {
        CompletionShell::Bash => Shell::Bash,
        CompletionShell::Zsh => Shell::Zsh,
        CompletionShell::Fish => Shell::Fish,
        CompletionShell::Powershell => Shell::PowerShell,
        CompletionShell::Elvish => Shell::Elvish,
    };
    let mut cmd = Cli::command();
    let name = cmd.get_name().to_string();
    generate(shell, &mut cmd, name, &mut std::io::stdout());
    Ok(0)
}

/// Render a man page for the CLI to stdout. Always exits 0 on success.
pub fn man() -> Result<i32, Error> {
    let cmd = Cli::command();
    let man = clap_mangen::Man::new(cmd);
    let mut out = Vec::new();
    man.render(&mut out)
        .map_err(|e| Error::IoBare(std::io::Error::other(e.to_string())))?;
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    handle.write_all(&out)?;
    handle.flush()?;
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn completions_generate_for_all_shells() {
        for shell in [
            Shell::Bash,
            Shell::Zsh,
            Shell::Fish,
            Shell::PowerShell,
            Shell::Elvish,
        ] {
            let mut cmd = Cli::command();
            let mut buf = Vec::new();
            generate(shell, &mut cmd, "driftwatch", &mut buf);
            assert!(!buf.is_empty(), "empty completions for {shell:?}");
        }
    }

    #[test]
    fn man_page_renders() {
        let cmd = Cli::command();
        let man = clap_mangen::Man::new(cmd);
        let mut out = Vec::new();
        man.render(&mut out).unwrap();
        let text = String::from_utf8_lossy(&out);
        assert!(text.contains("driftwatch"));
    }

    #[test]
    fn help_mentions_examples() {
        let mut cmd = Cli::command();
        let mut buf = Vec::new();
        cmd.write_long_help(&mut buf).unwrap();
        let text = String::from_utf8_lossy(&buf);
        assert!(text.contains("Examples"), "top-level help lacks examples");
    }
}
