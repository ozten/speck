//! Binary entrypoint for the `speck` CLI.

use std::process::ExitCode;

fn main() -> ExitCode {
    // Recording is handled in commands::dispatch via SPECK_RECORD=<dir>.
    match speck::run(std::env::args()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("{err}");
            ExitCode::FAILURE
        }
    }
}
