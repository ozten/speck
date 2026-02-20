//! `speck deps` command.

/// Execute the `deps` command.
///
/// # Errors
///
/// Returns an error string if deps logic fails (stub currently always succeeds).
pub fn run() -> Result<(), String> {
    println!("not yet implemented");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::run;

    #[test]
    fn deps_command_runs() {
        assert!(run().is_ok());
    }
}
