//! Replaying adapter for the `GitRepo` port.

use std::path::Path;
use std::sync::{Arc, Mutex};

use super::{next_output, replay_result};
use crate::cassette::replayer::CassetteReplayer;
use crate::ports::GitRepo;

/// Serves recorded git results from a cassette.
pub struct ReplayingGitRepo {
    replayer: Option<Arc<Mutex<CassetteReplayer>>>,
}

impl ReplayingGitRepo {
    /// Create a replaying git repo backed by the given replayer.
    #[must_use]
    pub fn new(replayer: Arc<Mutex<CassetteReplayer>>) -> Self {
        Self { replayer: Some(replayer) }
    }

    /// Create a replaying git repo with no cassette. Panics when called.
    #[must_use]
    pub fn unconfigured() -> Self {
        Self { replayer: None }
    }
}

impl GitRepo for ReplayingGitRepo {
    fn current_commit(&self) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let output = next_output(self.replayer.as_ref(), "git", "current_commit");
        replay_result(output)
    }

    fn diff(&self) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let output = next_output(self.replayer.as_ref(), "git", "diff");
        replay_result(output)
    }

    fn list_files(
        &self,
        _path: &Path,
    ) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
        let output = next_output(self.replayer.as_ref(), "git", "list_files");
        replay_result(output)
    }
}
