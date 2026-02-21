//! Live shell executor using `std::process::Command`.

use std::process::Command;

use crate::ports::shell::{ShellExecutor, ShellOutput};

/// Live shell executor that runs commands via the system shell.
pub struct LiveShellExecutor;

impl ShellExecutor for LiveShellExecutor {
    fn run(&self, command: &str) -> Result<ShellOutput, Box<dyn std::error::Error + Send + Sync>> {
        let output = Command::new("sh").arg("-c").arg(command).output()?;
        Ok(ShellOutput {
            exit_code: output.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runs_echo_command() {
        let shell = LiveShellExecutor;
        let result = shell.run("echo hello").unwrap();

        assert_eq!(result.exit_code, 0);
        assert_eq!(result.stdout.trim(), "hello");
        assert!(result.stderr.is_empty());
    }

    #[test]
    fn captures_exit_code() {
        let shell = LiveShellExecutor;
        let result = shell.run("exit 42").unwrap();

        assert_eq!(result.exit_code, 42);
    }
}
