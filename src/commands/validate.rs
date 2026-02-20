//! `speck validate` command.

/// Execute the `validate` command.
///
/// # Errors
///
/// Returns an error string if validation logic fails (stub currently always succeeds).
pub fn run() -> Result<(), String> {
    println!("not yet implemented");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::run;

    #[test]
    fn validate_command_runs() {
        assert!(run().is_ok());
    }
}
