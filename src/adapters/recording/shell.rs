//! Recording adapter for the `ShellExecutor` port.

use std::sync::{Arc, Mutex};

use serde::Serialize;

use super::record_result;
use crate::cassette::recorder::CassetteRecorder;
use crate::ports::{ShellExecutor, ShellOutput};

/// Records shell interactions while delegating to an inner implementation.
pub struct RecordingShellExecutor {
    inner: Box<dyn ShellExecutor>,
    recorder: Arc<Mutex<CassetteRecorder>>,
}

impl RecordingShellExecutor {
    /// Creates a new recording shell executor wrapping the given implementation.
    pub fn new(inner: Box<dyn ShellExecutor>, recorder: Arc<Mutex<CassetteRecorder>>) -> Self {
        Self { inner, recorder }
    }
}

#[derive(Serialize)]
struct CommandInput<'a> {
    command: &'a str,
}

impl ShellExecutor for RecordingShellExecutor {
    fn run(&self, command: &str) -> Result<ShellOutput, Box<dyn std::error::Error + Send + Sync>> {
        let result = self.inner.run(command);
        let input = CommandInput { command };
        record_result(&self.recorder, "shell", "run", &input, &result);
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::live::shell::LiveShellExecutor;

    #[test]
    fn records_run_interaction() {
        let dir = std::env::temp_dir().join("speck_rec_shell_test");
        std::fs::create_dir_all(&dir).unwrap();
        let cassette_path = dir.join("shell.cassette.yaml");

        let recorder = Arc::new(Mutex::new(CassetteRecorder::new(&cassette_path, "test", "abc")));

        // Scope the adapter so it's dropped before we try to unwrap
        {
            let shell =
                RecordingShellExecutor::new(Box::new(LiveShellExecutor), Arc::clone(&recorder));
            let result = shell.run("echo hello");
            assert!(result.is_ok());
        }

        let recorder = Arc::try_unwrap(recorder).unwrap().into_inner().unwrap();
        recorder.finish().unwrap();

        let content = std::fs::read_to_string(&cassette_path).unwrap();
        assert!(content.contains("shell"));
        assert!(content.contains("run"));
        assert!(content.contains("echo hello"));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
