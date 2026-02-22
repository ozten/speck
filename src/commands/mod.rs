//! Command dispatch and handlers.

pub mod deps;
pub mod map;
pub mod plan;
pub mod show;
pub mod status;
pub mod sync;
pub mod validate;

use std::env;
use std::path::PathBuf;

use crate::cassette::session::RecordingSession;
use crate::cli::Command;
use crate::context::ServiceContext;

/// Dispatch a parsed command to its handler.
///
/// When `SPECK_RECORD` is set to a directory path, all port interactions are
/// recorded to per-port cassette files in that directory.
///
/// # Errors
///
/// Returns an error string if the selected command handler fails.
pub fn dispatch(command: &Command) -> Result<(), String> {
    let (ctx, session) = if let Ok(path) = env::var("SPECK_RECORD") {
        let (ctx, session) = ServiceContext::recording_at(PathBuf::from(path))?;
        (ctx, Some(session))
    } else {
        (ServiceContext::live(), None)
    };

    let result = dispatch_with_context(command, &ctx);

    // Finish recording after command completes (even on error)
    if let Some(session) = session {
        // Drop context first to release Arc references
        drop(ctx);
        finish_recording(session)?;
    }

    result
}

/// Dispatch a command with the given service context.
fn dispatch_with_context(command: &Command, ctx: &ServiceContext) -> Result<(), String> {
    match command {
        Command::Plan => plan::run(),
        Command::Validate { spec_id, all } => {
            validate::run_with_context(ctx, spec_id.as_deref(), *all, None)
        }
        Command::Map { diff } => map::run(*diff),
        Command::Show { id } => show::run(id.as_deref()),
        Command::Status => status::run(),
        Command::Deps => deps::run(),
        Command::Sync { target, dry_run } => sync::run(target, *dry_run),
    }
}

/// Finish a recording session and print the output directory.
fn finish_recording(session: RecordingSession) -> Result<(), String> {
    let output_dir = session.finish()?;
    eprintln!("Recording saved to: {}", output_dir.display());
    Ok(())
}
