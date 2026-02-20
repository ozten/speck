//! Shell executor port for running external commands.

/// The output of a shell command execution.
#[derive(Debug, Clone)]
pub struct ShellOutput {
    /// The exit code of the process.
    pub exit_code: i32,
    /// The captured standard output.
    pub stdout: String,
    /// The captured standard error.
    pub stderr: String,
}

/// Executes shell commands.
///
/// Abstracting shell execution allows deterministic replay by recording
/// and replaying command outputs during cassette playback.
pub trait ShellExecutor: Send + Sync {
    /// Runs a command string in the system shell and returns its output.
    ///
    /// # Errors
    ///
    /// Returns an error if the command cannot be spawned or fails to execute.
    fn run(&self, command: &str) -> Result<ShellOutput, Box<dyn std::error::Error + Send + Sync>>;
}
