//! Core library entry for the `speck` CLI.

pub mod adapters;
pub mod cassette;
pub mod cli;
pub mod commands;
pub mod context;
pub mod map;
pub mod plan;
pub mod ports;
pub mod spec;
pub mod store;
pub mod sync;
pub mod validate;

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
    let cli = match cli::Cli::try_parse_from(args) {
        Ok(cli) => cli,
        Err(err) if err.use_stderr() => return Err(err.to_string()),
        Err(err) => {
            // --help or --version: print to stdout and succeed.
            let _ = err.print();
            return Ok(());
        }
    };
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
