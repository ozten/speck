//! Replaying adapter for the `ShellExecutor` port.

use std::sync::{Arc, Mutex};

use super::{next_output, replay_result};
use crate::cassette::replayer::CassetteReplayer;
use crate::ports::{ShellExecutor, ShellOutput};

/// Serves recorded shell execution results from a cassette.
pub struct ReplayingShellExecutor {
    replayer: Option<Arc<Mutex<CassetteReplayer>>>,
}

impl ReplayingShellExecutor {
    /// Create a replaying shell executor backed by the given replayer.
    #[must_use]
    pub fn new(replayer: Arc<Mutex<CassetteReplayer>>) -> Self {
        Self { replayer: Some(replayer) }
    }

    /// Create a replaying shell executor with no cassette. Panics when called.
    #[must_use]
    pub fn unconfigured() -> Self {
        Self { replayer: None }
    }
}

impl ShellExecutor for ReplayingShellExecutor {
    fn run(&self, _command: &str) -> Result<ShellOutput, Box<dyn std::error::Error + Send + Sync>> {
        let output = next_output(self.replayer.as_ref(), "shell", "run");
        replay_result(output)
    }
}
