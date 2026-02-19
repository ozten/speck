//! Core library entry for the `speck` CLI.

pub mod cli;
pub mod commands;

use clap::Parser;

/// Run the CLI with the provided arguments.
///
/// # Errors
///
/// Returns an error string when argument parsing fails or command execution fails.
pub fn run<I, T>(args: I) -> Result<(), String>
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString> + Clone,
{
    let cli = cli::Cli::try_parse_from(args).map_err(|err| err.to_string())?;
    commands::dispatch(&cli.command)
}

#[cfg(test)]
mod tests {
    use super::run;

    #[test]
    fn run_executes_plan() {
        let result = run(["speck", "plan"]);
        assert!(result.is_ok());
    }

    #[test]
    fn run_errors_on_unknown_subcommand() {
        let result = run(["speck", "unknown"]);
        assert!(result.is_err());
    }
}
