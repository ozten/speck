//! `speck plan` command.

use std::path::PathBuf;

/// Execute the `plan` command.
///
/// # Errors
///
/// Returns an error string if planning logic fails (stub currently always succeeds).
pub fn run(requirement: Option<&str>, from: Option<&PathBuf>) -> Result<(), String> {
    println!("not yet implemented");
    let _ = requirement;
    let _ = from;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::run;

    #[test]
    fn plan_command_runs() {
        assert!(run(None, None).is_ok());
    }
}
