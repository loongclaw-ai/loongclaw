use clap::CommandFactory;
use clap_complete::{Shell, generate};

use loongclaw_spec::CliResult;

pub struct CompletionsCommandOptions {
    pub shell: Shell,
}

/// Generate completions into an arbitrary writer — enables unit testing without stdout capture.
pub fn generate_completions(shell: Shell, writer: &mut dyn std::io::Write) {
    let mut cmd = crate::Cli::command();
    generate(shell, &mut cmd, crate::CLI_COMMAND_NAME, writer);
}

pub fn run_completions_cli(options: CompletionsCommandOptions) -> CliResult<()> {
    generate_completions(options.shell, &mut std::io::stdout());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn completions_bash_non_empty() {
        let mut buf = Vec::new();
        generate_completions(Shell::Bash, &mut buf);
        assert!(!buf.is_empty());
    }

    #[test]
    fn completions_zsh_contains_binary_name() {
        let mut buf = Vec::new();
        generate_completions(Shell::Zsh, &mut buf);
        let out = String::from_utf8(buf).unwrap();
        assert!(out.contains("loongclaw"));
    }

    #[test]
    fn completions_fish_non_empty() {
        let mut buf = Vec::new();
        generate_completions(Shell::Fish, &mut buf);
        assert!(!buf.is_empty());
    }

    #[test]
    fn completions_powershell_non_empty() {
        let mut buf = Vec::new();
        generate_completions(Shell::PowerShell, &mut buf);
        assert!(!buf.is_empty());
    }

    #[test]
    fn completions_elvish_non_empty() {
        let mut buf = Vec::new();
        generate_completions(Shell::Elvish, &mut buf);
        assert!(!buf.is_empty());
    }

    #[test]
    fn run_completions_cli_returns_ok() {
        let result = run_completions_cli(CompletionsCommandOptions { shell: Shell::Fish });
        assert!(result.is_ok());
    }
}
