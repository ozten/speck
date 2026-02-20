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
    /// Validate behavior and quality checks.
    Validate {
        /// The spec ID to validate (e.g., "IMPACT-42").
        spec_id: Option<String>,
        /// Validate all task specs in the store.
        #[arg(long)]
        all: bool,
    },
    /// Map dependencies between tasks.
    Map,
    /// Show details of a specific item.
    Show {
        /// The identifier to show.
        id: Option<String>,
    },
    /// Display current project status.
    Status,
    /// List dependency relationships.
    Deps,
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
    fn parses_validate_subcommand() {
        let cli = Cli::parse_from(["speck", "validate", "--all"]);
        assert!(matches!(cli.command, Command::Validate { all: true, .. }));
    }

    #[test]
    fn parses_validate_with_spec_id() {
        let cli = Cli::parse_from(["speck", "validate", "IMPACT-42"]);
        assert!(matches!(cli.command, Command::Validate { spec_id: Some(_), all: false }));
    }

    #[test]
    fn parses_map_subcommand() {
        let cli = Cli::parse_from(["speck", "map"]);
        assert!(matches!(cli.command, Command::Map));
    }

    #[test]
    fn parses_show_subcommand() {
        let cli = Cli::parse_from(["speck", "show"]);
        assert!(matches!(cli.command, Command::Show { id: None }));
    }

    #[test]
    fn parses_show_with_id() {
        let cli = Cli::parse_from(["speck", "show", "task-1"]);
        assert!(matches!(cli.command, Command::Show { id: Some(_) }));
    }

    #[test]
    fn parses_status_subcommand() {
        let cli = Cli::parse_from(["speck", "status"]);
        assert!(matches!(cli.command, Command::Status));
    }

    #[test]
    fn parses_deps_subcommand() {
        let cli = Cli::parse_from(["speck", "deps"]);
        assert!(matches!(cli.command, Command::Deps));
    }
}
