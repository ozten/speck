//! `speck map` command.

/// Execute the `map` command.
///
/// # Errors
///
/// Returns an error string if mapping logic fails (stub currently always succeeds).
pub fn run() -> Result<(), String> {
    println!("not yet implemented");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::run;

    #[test]
    fn map_command_runs() {
        assert!(run().is_ok());
    }
}
