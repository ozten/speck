//! CLI argument definitions.

use clap::{Parser, Subcommand};

/// Top-level CLI parser for `speck`.
#[derive(Debug, Parser)]
#[command(name = "speck", version, about = "Plan and verify product work")]
pub struct Cli {
    /// The command to execute.
    #[command(subcommand)]
    pub command: Command,
}

/// Supported top-level subcommands.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Produce a lightweight implementation plan.
    Plan,
    /// Verify behavior and quality checks.
    Verify,
}

#[cfg(test)]
mod tests {
    use super::{Cli, Command};
    use clap::Parser;

    #[test]
    fn parses_plan_subcommand() {
        let cli = Cli::parse_from(["speck", "plan"]);
        assert!(matches!(cli.command, Command::Plan));
    }

    #[test]
    fn parses_verify_subcommand() {
        let cli = Cli::parse_from(["speck", "verify"]);
        assert!(matches!(cli.command, Command::Verify));
    }
}
