//! Binary entrypoint for the `speck` CLI.

use std::process::ExitCode;

fn main() -> ExitCode {
    // If SPECK_RECORD is set, create a recording context that writes a
    // cassette file on drop. This is an internal developer mechanism and
    // is not exposed in --help.
    let _recording_ctx = std::env::var("SPECK_RECORD")
        .ok()
        .map(|path| speck::context::ServiceContext::recording(std::path::Path::new(&path)));

    match speck::run(std::env::args()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("{err}");
            ExitCode::FAILURE
        }
    }
}
