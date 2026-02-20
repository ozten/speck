//! `speck status` command.

/// Execute the `status` command.
///
/// # Errors
///
/// Returns an error string if status logic fails (stub currently always succeeds).
pub fn run() -> Result<(), String> {
    println!("not yet implemented");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::run;

    #[test]
    fn status_command_runs() {
        assert!(run().is_ok());
    }
}
