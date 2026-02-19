//! `speck plan` command.

/// Execute the `plan` command.
///
/// # Errors
///
/// Returns an error string if planning logic fails (stub currently always succeeds).
pub fn run() -> Result<(), String> {
    println!("[stub] speck plan: gather requirements, identify risks, draft milestones");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::run;

    #[test]
    fn plan_command_runs() {
        assert!(run().is_ok());
    }
}
