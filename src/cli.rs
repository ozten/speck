//! CLI argument definitions.

use std::path::PathBuf;

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
    #[command(group = clap::ArgGroup::new("input").multiple(false))]
    Plan {
        /// Requirement text to plan against.
        #[arg(group = "input")]
        requirement: Option<String>,
        /// Read requirement from a file.
        #[arg(long, group = "input")]
        from: Option<PathBuf>,
    },
    /// Validate behavior and quality checks.
    Validate {
        /// The spec ID to validate.
        spec_id: Option<String>,
        /// Validate all specs in the store.
        #[arg(long)]
        all: bool,
    },
    /// Map dependencies between tasks.
    Map {
        /// Show what changed since the last map.
        #[arg(long)]
        diff: bool,
    },
    /// Show details of a specific item.
    Show {
        /// The identifier to show.
        id: Option<String>,
    },
    /// Display current project status.
    Status,
    /// List dependency relationships.
    Deps,
    /// Sync specs to an external tracker.
    Sync {
        /// The sync target (e.g., "beads").
        target: String,
        /// Show what would happen without making changes.
        #[arg(long)]
        dry_run: bool,
    },
}

#[cfg(test)]
mod tests {
    use super::{Cli, Command};
    use clap::Parser;

    #[test]
    fn parses_plan_subcommand() {
        let cli = Cli::parse_from(["speck", "plan"]);
        assert!(matches!(cli.command, Command::Plan { requirement: None, from: None }));
    }

    #[test]
    fn parses_plan_with_requirement() {
        let cli = Cli::parse_from(["speck", "plan", "add login page"]);
        assert!(matches!(cli.command, Command::Plan { requirement: Some(_), from: None }));
        if let Command::Plan { requirement, .. } = cli.command {
            assert_eq!(requirement.unwrap(), "add login page");
        }
    }

    #[test]
    fn parses_plan_with_from() {
        let cli = Cli::parse_from(["speck", "plan", "--from", "requirements.md"]);
        assert!(matches!(cli.command, Command::Plan { requirement: None, from: Some(_) }));
        if let Command::Plan { from, .. } = cli.command {
            assert_eq!(from.unwrap().to_str().unwrap(), "requirements.md");
        }
    }

    #[test]
    fn plan_rejects_both_requirement_and_from() {
        let result = Cli::try_parse_from(["speck", "plan", "some text", "--from", "file.md"]);
        assert!(result.is_err());
    }

    #[test]
    fn parses_validate_subcommand() {
        let cli = Cli::parse_from(["speck", "validate"]);
        assert!(matches!(cli.command, Command::Validate { spec_id: None, all: false }));
    }

    #[test]
    fn parses_validate_with_spec_id() {
        let cli = Cli::parse_from(["speck", "validate", "TASK-1"]);
        assert!(matches!(cli.command, Command::Validate { spec_id: Some(_), all: false }));
    }

    #[test]
    fn parses_validate_all() {
        let cli = Cli::parse_from(["speck", "validate", "--all"]);
        assert!(matches!(cli.command, Command::Validate { spec_id: None, all: true }));
    }

    #[test]
    fn parses_map_subcommand() {
        let cli = Cli::parse_from(["speck", "map"]);
        assert!(matches!(cli.command, Command::Map { diff: false }));
    }

    #[test]
    fn parses_map_diff() {
        let cli = Cli::parse_from(["speck", "map", "--diff"]);
        assert!(matches!(cli.command, Command::Map { diff: true }));
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

    #[test]
    fn parses_sync_subcommand() {
        let cli = Cli::parse_from(["speck", "sync", "beads"]);
        assert!(matches!(
            cli.command,
            Command::Sync { ref target, dry_run: false } if target == "beads"
        ));
    }

    #[test]
    fn parses_sync_dry_run() {
        let cli = Cli::parse_from(["speck", "sync", "beads", "--dry-run"]);
        assert!(matches!(
            cli.command,
            Command::Sync { ref target, dry_run: true } if target == "beads"
        ));
    }
}
