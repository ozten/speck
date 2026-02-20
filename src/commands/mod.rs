//! Command dispatch and handlers.

pub mod deps;
pub mod map;
pub mod plan;
pub mod show;
pub mod status;
pub mod validate;

use crate::cli::Command;

/// Dispatch a parsed command to its handler.
///
/// # Errors
///
/// Returns an error string if the selected command handler fails.
pub fn dispatch(command: &Command) -> Result<(), String> {
    match command {
        Command::Plan => plan::run(),
        Command::Validate => validate::run(),
        Command::Map => map::run(),
        Command::Show { id } => show::run(id.as_deref()),
        Command::Status => status::run(),
        Command::Deps => deps::run(),
    }
}
