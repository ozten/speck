//! Command dispatch and handlers.

pub mod plan;
pub mod verify;

use crate::cli::Command;

/// Dispatch a parsed command to its handler.
///
/// # Errors
///
/// Returns an error string if the selected command handler fails.
pub fn dispatch(command: &Command) -> Result<(), String> {
    match command {
        Command::Plan => plan::run(),
        Command::Verify => verify::run(),
    }
}
