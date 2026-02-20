//! `speck show` command.

/// Execute the `show` command.
///
/// # Errors
///
/// Returns an error string if show logic fails (stub currently always succeeds).
pub fn run(_id: Option<&str>) -> Result<(), String> {
    println!("not yet implemented");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::run;

    #[test]
    fn show_command_runs() {
        assert!(run(None).is_ok());
    }

    #[test]
    fn show_command_runs_with_id() {
        assert!(run(Some("task-1")).is_ok());
    }
}
