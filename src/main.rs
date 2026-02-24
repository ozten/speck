//! Binary entrypoint for the `speck` CLI.

use std::process::ExitCode;

fn main() -> ExitCode {
    // Load .env file if present (missing file is fine).
    dotenvy::dotenv().ok();
    // Recording is handled in commands::dispatch via SPECK_REC=true.
    match speck::run(std::env::args()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("{err}");
            ExitCode::FAILURE
        }
    }
}
