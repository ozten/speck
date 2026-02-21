//! Command dispatch and handlers.

pub mod deps;
pub mod map;
pub mod plan;
pub mod show;
pub mod status;
pub mod sync;
pub mod validate;

use std::env;

use crate::cassette::session::RecordingSession;
use crate::cli::Command;
use crate::context::ServiceContext;

/// Dispatch a parsed command to its handler.
///
/// When `SPECK_REC=true` is set, all port interactions are recorded to
/// per-port cassette files in `.speck/cassettes/<timestamp>/`.
///
/// # Errors
///
/// Returns an error string if the selected command handler fails.
pub fn dispatch(command: &Command) -> Result<(), String> {
    let recording_enabled = env::var("SPECK_REC")
        .map(|v| v == "true")
        .unwrap_or(false);

    let (ctx, session) = if recording_enabled {
        let (ctx, session) = ServiceContext::recording()?;
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
        Command::Validate { spec_id, all } => validate::run_with_context(ctx, spec_id.as_deref(), *all),
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
